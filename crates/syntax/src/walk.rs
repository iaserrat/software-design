//! One pass over a syntax tree that fills a [`FileFacts`].

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use crate::fragment::{self, line};
use crate::lang::{Kinds, Lang};
use crate::{
    Arg, Body, Call, Comment, Field, FileFacts, Function, IgnoredError, Local, Span, TypeDecl,
    comment,
};

/// The facts of `source`. A file that fails to parse yields empty facts; a
/// file with syntax errors yields the facts of its well-formed parts.
pub(crate) fn extract(lang: Lang, source: &str) -> FileFacts {
    let mut parser = Parser::new();
    parser
        .set_language(&lang.grammar())
        .expect("bundled grammars match the tree-sitter ABI");
    let Some(tree) = parser.parse(source, None) else {
        return FileFacts::default();
    };
    let mut walker = Walker {
        lang,
        kinds: lang.kinds(),
        source: source.as_bytes(),
        facts: FileFacts::default(),
        scopes: Vec::new(),
        functions: Vec::new(),
        docs: HashMap::new(),
    };
    walker.collect_comments(tree.root_node());
    walker.walk(tree.root_node());
    walker.facts
}

/// A comment on its own line describes at most this many following lines of
/// code. A longer reach would compare the comment with code it does not describe.
const DESCRIBED_LINES: usize = 3;

/// The class, impl or trait whose methods are being visited.
struct Scope {
    node_id: usize,
    name: String,
    /// The scope extends, implements or is a trait impl: its methods may override.
    inherits: bool,
}

/// A function being visited, with the names it declares and where each is used.
/// Anonymous functions get a frame too, so their parameters do not count as
/// names of the function around them; they have no `index`.
struct Frame {
    node_id: usize,
    index: Option<usize>,
    declared: HashMap<String, u32>,
    uses: HashMap<String, u32>,
}

struct Walker<'a> {
    lang: Lang,
    kinds: &'static Kinds,
    source: &'a [u8],
    facts: FileFacts,
    scopes: Vec<Scope>,
    functions: Vec<Frame>,
    /// Doc comment index by the line its declaration (or wrapper) starts on.
    docs: HashMap<u32, usize>,
}

impl<'a> Walker<'a> {
    fn text(&self, node: Node<'_>) -> &'a str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    fn walk(&mut self, root: Node<'_>) {
        let mut cursor = root.walk();
        'outer: loop {
            self.enter(cursor.node());
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                self.exit(cursor.node());
                if cursor.goto_next_sibling() {
                    continue 'outer;
                }
                if !cursor.goto_parent() {
                    break 'outer;
                }
            }
        }
    }

    fn enter(&mut self, node: Node<'_>) {
        let kind = node.kind();
        let kinds = self.kinds;
        // Record the type before entering its scope: its visibility depends on
        // the scope around it, not on itself.
        if Kinds::is(kinds.types, kind) {
            self.record_type(node);
        }
        if Kinds::is(kinds.scopes, kind) {
            self.enter_scope(node);
        }
        if Kinds::is(kinds.functions, kind) || self.lambda_declarator(node).is_some() {
            self.enter_function(node);
        } else if Kinds::is(kinds.lambdas, kind) || Kinds::is(kinds.closures, kind) {
            self.functions.push(Frame {
                node_id: node.id(),
                index: None,
                declared: self
                    .params(node)
                    .into_iter()
                    .map(|p| (p, line(node.start_position().row)))
                    .collect(),
                uses: HashMap::new(),
            });
        }
        if Kinds::is(kinds.calls, kind)
            && let Some(mut call) = self.call(node)
        {
            call.caller = self.functions.iter().rev().find_map(|f| f.index);
            self.facts.calls.push(call);
        }
        if Kinds::is(kinds.imports, kind) {
            self.facts.imports.push(self.text(node).to_owned());
        }
        if Kinds::is(kinds.blocks, kind) {
            self.record_fragments(node);
        }
        self.record_field(node);
        self.record_ignored_error(node);
        self.record_name(node);
    }

    fn exit(&mut self, node: Node<'_>) {
        let id = node.id();
        self.scopes.pop_if(|s| s.node_id == id);
        if let Some(frame) = self.functions.pop_if(|f| f.node_id == id)
            && let Some(index) = frame.index
        {
            for (name, declared) in frame.declared {
                let last_use = frame.uses.get(&name).copied().unwrap_or(declared);
                self.facts.locals.push(Local {
                    function: index,
                    name,
                    declared,
                    last_use: last_use.max(declared),
                });
            }
        }
    }

    // --- Comments -------------------------------------------------------------

    /// Merges comment nodes into blocks and records which line each doc comment
    /// attaches to, before the main walk needs it.
    fn collect_comments(&mut self, root: Node<'_>) {
        let mut nodes = Vec::new();
        let mut cursor = root.walk();
        'outer: loop {
            let node = cursor.node();
            if Kinds::is(self.kinds.comments, node.kind()) {
                nodes.push(node);
            } else if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    continue 'outer;
                }
                if !cursor.goto_parent() {
                    break 'outer;
                }
            }
        }
        let mut i = 0;
        while i < nodes.len() {
            let first = nodes[i];
            let doc = self.is_doc_comment(first);
            let mut last = first;
            while let Some(&next) = nodes.get(i + 1) {
                let joins = next.start_position().row == last.end_position().row + 1
                    || (self.lang == Lang::Rust
                        && next.start_position().row == last.end_position().row);
                if !joins
                    || self.is_doc_comment(next) != doc
                    || self.is_block_comment(next)
                    || self.is_block_comment(last)
                    || self.trails_code(next)
                {
                    break;
                }
                last = next;
                i += 1;
            }
            let raw = &self.source[first.start_byte()..last.end_byte()];
            let raw = std::str::from_utf8(raw).unwrap_or_default();
            let index = self.facts.comments.len();
            self.facts.comments.push(Comment {
                span: Span {
                    start: line(first.start_position().row),
                    end: end_line(last),
                },
                text: comment::clean(raw),
                doc,
                code: if doc {
                    String::new()
                } else {
                    self.described_code(first, last)
                },
            });
            if doc {
                self.docs.insert(end_line(last) + 1, index);
            }
            i += 1;
        }
    }

    fn is_doc_comment(&self, node: Node<'_>) -> bool {
        let text = self.text(node);
        match self.lang {
            Lang::Rust => {
                (text.starts_with("///") && !text.starts_with("////"))
                    || text.starts_with("//!")
                    || text.starts_with("/**")
            }
            Lang::Python => false,
            _ => text.starts_with("/**"),
        }
    }

    fn is_block_comment(&self, node: Node<'_>) -> bool {
        self.text(node).starts_with("/*")
    }

    fn trails_code(&self, node: Node<'_>) -> bool {
        node.prev_sibling().is_some_and(|prev| {
            prev.end_position().row == node.start_position().row
                && !Kinds::is(self.kinds.comments, prev.kind())
        })
    }

    /// The code a non-doc comment describes: the code before it on the same line,
    /// or the statements that follow it without a blank line in between.
    fn described_code(&self, first: Node<'_>, last: Node<'_>) -> String {
        if self.trails_code(first) {
            let row_start = self.source[..first.start_byte()]
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |i| i + 1);
            return String::from_utf8_lossy(&self.source[row_start..first.start_byte()])
                .trim()
                .to_owned();
        }
        let mut code = Vec::new();
        let mut end_row = end_line(last) as usize - 1;
        let mut next = last.next_sibling();
        while let Some(node) = next {
            if Kinds::is(self.kinds.comments, node.kind())
                || node.start_position().row > end_row + 1
                || code.iter().map(|c: &&str| c.lines().count()).sum::<usize>() >= DESCRIBED_LINES
            {
                break;
            }
            if node.is_named() {
                code.push(self.text(node));
            }
            end_row = node.end_position().row;
            next = node.next_sibling();
        }
        code.join("\n")
            .lines()
            .take(DESCRIBED_LINES)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `node` with its wrappers, such as `export`: what a reader sees declared.
    fn wrapped<'t>(&self, node: Node<'t>) -> Node<'t> {
        let mut outer = node;
        while let Some(parent) = outer
            .parent()
            .filter(|p| Kinds::is(self.kinds.wrappers, p.kind()))
        {
            outer = parent;
        }
        outer
    }

    /// The doc comment ending right above `node`, its wrapper or its attributes.
    fn doc_for(&self, node: Node<'_>) -> Option<usize> {
        let anchor = self.wrapped(node);
        let mut start = anchor.start_position().row;
        let mut prev = anchor.prev_named_sibling();
        // Attributes, decorators and ordinary comments (tool directives such as
        // `eslint-disable-next-line`) may sit between a doc comment and its declaration.
        while let Some(between) = prev.filter(|p| {
            Kinds::is(self.kinds.attributes, p.kind())
                || (Kinds::is(self.kinds.comments, p.kind()) && !self.is_doc_comment(*p))
        }) {
            start = between.start_position().row;
            prev = between.prev_named_sibling();
        }
        // Decorators and attributes may also be children of the declaration itself.
        let mut child_cursor = node.walk();
        if let Some(attr) = node
            .named_children(&mut child_cursor)
            .find(|c| Kinds::is(self.kinds.attributes, c.kind()))
        {
            start = start.min(attr.start_position().row);
        }
        self.docs.get(&line(start)).copied()
    }

    /// The first statement of a Python body when it is a string literal.
    fn docstring(&mut self, body: Node<'_>, header: &str) -> Option<usize> {
        let first = body.named_child(0)?;
        let string = first
            .named_child(0)
            .filter(|s| first.kind() == "expression_statement" && s.kind() == "string")?;
        self.facts.comments.push(Comment {
            span: Span {
                start: line(string.start_position().row),
                end: line(string.end_position().row),
            },
            text: comment::clean(self.text(string)),
            doc: true,
            code: header.to_owned(),
        });
        Some(self.facts.comments.len() - 1)
    }

    // --- Declarations ---------------------------------------------------------

    fn enter_scope(&mut self, node: Node<'_>) {
        let (name, inherits) = match node.kind() {
            "impl_item" => (
                node.child_by_field_name("type")
                    .map(|t| self.text(t))
                    .unwrap_or_default(),
                node.child_by_field_name("trait").is_some(),
            ),
            "trait_item" => (
                node.child_by_field_name("name")
                    .map(|n| self.text(n))
                    .unwrap_or_default(),
                true,
            ),
            _ => {
                let inherits = node.child_by_field_name("superclasses").is_some()
                    || children(node).iter().any(|c| {
                        matches!(
                            c.kind(),
                            "class_heritage" | "base_clause" | "class_interface_clause"
                        )
                    });
                (
                    node.child_by_field_name("name")
                        .map(|n| self.text(n))
                        .unwrap_or_default(),
                    inherits,
                )
            }
        };
        self.scopes.push(Scope {
            node_id: node.id(),
            name: name.to_owned(),
            inherits,
        });
    }

    fn record_type(&mut self, node: Node<'_>) {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let header_end = node
            .child_by_field_name("body")
            .map_or(node.end_byte(), |b| b.start_byte());
        let header =
            String::from_utf8_lossy(&self.source[self.wrapped(node).start_byte()..header_end])
                .trim()
                .to_owned();
        let doc = self.doc_for(node);
        let mut doc = doc;
        if self.lang == Lang::Python
            && let Some(body) = node.child_by_field_name("body")
        {
            doc = self.docstring(body, &header);
        }
        if let Some(index) = doc {
            self.facts.comments[index].code.clone_from(&header);
        }
        self.facts.types.push(TypeDecl {
            name: self.text(name).to_owned(),
            span: span_of(node),
            public: self.is_public(node, self.text(name)),
            header,
            doc,
        });
    }

    /// The `const name = …` that names an arrow function or function expression,
    /// directly or through React's `forwardRef(…)` and `memo(…)` wrappers.
    fn lambda_declarator<'t>(&self, node: Node<'t>) -> Option<Node<'t>> {
        if !Kinds::is(self.kinds.lambdas, node.kind()) {
            return None;
        }
        let mut parent = node.parent()?;
        while parent.kind() == "arguments" {
            let call = parent.parent()?;
            let callee = self.text(call.child_by_field_name("function")?);
            if !matches!(callee.rsplit('.').next(), Some("forwardRef" | "memo")) {
                return None;
            }
            parent = call.parent()?;
        }
        (parent.kind() == "variable_declarator").then_some(parent)
    }

    fn enter_function(&mut self, node: Node<'_>) {
        let Some(body) = node.child_by_field_name("body") else {
            return; // A declaration without a body: an interface or trait signature.
        };
        let named_by = self.lambda_declarator(node).unwrap_or(node);
        let name = named_by
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .unwrap_or_default()
            .to_owned();
        let declaration = self.wrapped(if named_by == node {
            node
        } else {
            named_by.parent().unwrap_or(named_by)
        });
        let header =
            String::from_utf8_lossy(&self.source[declaration.start_byte()..body.start_byte()])
                .trim()
                .trim_end_matches("=>")
                .trim()
                .to_owned();
        let header_span = Span {
            start: line(declaration.start_position().row),
            end: line(body.start_position().row),
        };
        let params = self.params(node);
        let mut doc = self.doc_for(declaration);
        if self.lang == Lang::Python {
            doc = self.docstring(body, &header);
        }
        if let Some(index) = doc {
            self.facts.comments[index].code.clone_from(&header);
        }
        let statements = self.statements(body);
        let body_shape = if statements.len() == 1 {
            self.body_shape(statements[0])
        } else {
            Body::Other
        };
        let (returns_value, returns_tuple) = self.returns(node, body);
        let scope = self.scopes.last();
        let function = Function {
            public: self.is_public(declaration, &name)
                && !matches!(
                    node.child_by_field_name("name").map(|n| n.kind()),
                    Some("private_property_identifier")
                ),
            overrides: scope.is_some_and(|s| s.inherits)
                || has_child_kind(node, "override_modifier"),
            owner: scope.map(|s| s.name.clone()),
            name,
            span: span_of(node),
            header,
            header_span,
            params: params.clone(),
            returns_value,
            returns_tuple,
            statements: u32::try_from(statements.len()).unwrap_or(u32::MAX),
            body: body_shape,
            doc,
        };
        let index = self.facts.functions.len();
        self.facts.functions.push(function);
        // The whole body, so two identical functions report as one copy.
        if let Some(fragment) = fragment::of(body, self.source, self.kinds) {
            self.facts.fragments.push(fragment);
        }
        let line = line(node.start_position().row);
        self.functions.push(Frame {
            node_id: node.id(),
            index: Some(index),
            declared: params.into_iter().map(|p| (p, line)).collect(),
            uses: HashMap::new(),
        });
    }

    fn is_public(&self, node: Node<'_>, name: &str) -> bool {
        let modifier = children(node)
            .into_iter()
            .find(|c| matches!(c.kind(), "accessibility_modifier" | "visibility_modifier"))
            .map(|c| self.text(c));
        match self.lang {
            Lang::Rust => modifier.is_some(),
            Lang::Python => {
                !name.starts_with('_') || (name.starts_with("__") && name.ends_with("__"))
            }
            Lang::Php => !matches!(modifier, Some("private" | "protected")),
            Lang::TypeScript | Lang::Tsx => {
                if matches!(modifier, Some("private" | "protected")) {
                    return false;
                }
                let in_class = self.scopes.last().is_some();
                in_class
                    || node.kind() == "export_statement"
                    || node
                        .parent()
                        .is_some_and(|p| p.kind() == "export_statement")
            }
        }
    }

    fn params(&self, node: Node<'_>) -> Vec<String> {
        let Some(list) = node
            .child_by_field_name("parameters")
            .or_else(|| node.child_by_field_name("parameter"))
        else {
            return Vec::new();
        };
        if list.kind() == "identifier" {
            return vec![self.text(list).to_owned()];
        }
        let mut cursor = list.walk();
        list.named_children(&mut cursor)
            .filter_map(|p| {
                let name = match p.kind() {
                    "identifier" => Some(p),
                    "self_parameter" | "comment" | "line_comment" | "block_comment" => None,
                    _ => p
                        .child_by_field_name("pattern")
                        .or_else(|| p.child_by_field_name("name"))
                        .or_else(|| p.named_child(0)),
                }?;
                let text = self.text(name).trim_start_matches('$');
                (!matches!(text, "self" | "cls" | "this")).then(|| text.to_owned())
            })
            .collect()
    }

    /// Body statements, excluding comments and a leading docstring. An expression
    /// body (`x => f(x)`) counts as one statement.
    fn statements<'t>(&self, body: Node<'t>) -> Vec<Node<'t>> {
        if !Kinds::is(self.kinds.blocks, body.kind()) && body.kind() != "compound_statement" {
            return vec![body];
        }
        let mut cursor = body.walk();
        let mut statements: Vec<Node<'t>> = body
            .named_children(&mut cursor)
            .filter(|c| !Kinds::is(self.kinds.comments, c.kind()))
            .collect();
        if self.lang == Lang::Python
            && statements.first().is_some_and(|s| {
                s.kind() == "expression_statement"
                    && s.named_child(0).is_some_and(|c| c.kind() == "string")
            })
        {
            statements.remove(0);
        }
        statements
    }

    fn body_shape(&self, statement: Node<'_>) -> Body {
        let mut expr = statement;
        loop {
            let unwrapped = match expr.kind() {
                "return_statement"
                | "return_expression"
                | "expression_statement"
                | "await_expression"
                | "await"
                | "try_expression"
                | "parenthesized_expression" => expr.named_child(0),
                _ => None,
            };
            match unwrapped {
                Some(inner) => expr = inner,
                None => break,
            }
        }
        if Kinds::is(self.kinds.calls, expr.kind()) {
            return self.call(expr).map_or(Body::Other, Body::Forward);
        }
        if expr.kind() == "reference_expression" {
            expr = expr.child_by_field_name("value").unwrap_or(expr);
        }
        let (object, field) = match expr.kind() {
            "member_expression" => (
                expr.child_by_field_name("object"),
                expr.child_by_field_name("property"),
            ),
            "attribute" => (
                expr.child_by_field_name("object"),
                expr.child_by_field_name("attribute"),
            ),
            "member_access_expression" => (
                expr.child_by_field_name("object"),
                expr.child_by_field_name("name"),
            ),
            "field_expression" => (
                expr.child_by_field_name("value"),
                expr.child_by_field_name("field"),
            ),
            _ => (None, None),
        };
        match (object.map(|o| self.text(o)), field) {
            (Some("this" | "self" | "$this"), Some(field)) => {
                Body::ReturnsField(self.text(field).trim_start_matches('#').to_owned())
            }
            _ => Body::Other,
        }
    }

    /// `(returns_value, returns_tuple)` from the declared return type, the body's
    /// `return` statements, or an expression body.
    fn returns(&self, node: Node<'_>, body: Node<'_>) -> (bool, bool) {
        let declared = node
            .child_by_field_name("return_type")
            .map(|t| self.text(t).trim_start_matches(':').trim());
        let mut tuple = declared.is_some_and(|t| {
            t.starts_with('[')
                || (t.starts_with('(') && t != "()")
                || t.starts_with("tuple")
                || t.starts_with("Tuple")
        });
        let declared_value = declared.is_some_and(|t| {
            !matches!(
                t,
                "void" | "None" | "()" | "never" | "Promise<void>" | "noreturn"
            )
        });
        if !Kinds::is(self.kinds.blocks, body.kind()) && body.kind() != "compound_statement" {
            return (true, tuple || body.kind() == "array");
        }
        let mut returns_expression = false;
        let mut cursor = body.walk();
        'outer: loop {
            let current = cursor.node();
            let nested_function = current != body
                && (Kinds::is(self.kinds.functions, current.kind())
                    || Kinds::is(self.kinds.lambdas, current.kind())
                    || current.kind() == "closure_expression");
            if Kinds::is(self.kinds.returns, current.kind()) {
                if let Some(returned) = current.named_child(0) {
                    returns_expression = true;
                    tuple |= matches!(
                        returned.kind(),
                        "expression_list" | "tuple_expression" | "tuple"
                    );
                }
            } else if !nested_function && cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.node() == body {
                    break 'outer;
                }
                if cursor.goto_next_sibling() {
                    continue 'outer;
                }
                if !cursor.goto_parent() {
                    break 'outer;
                }
            }
        }
        if self.lang == Lang::Rust {
            return (declared_value, tuple);
        }
        (declared_value || returns_expression, tuple)
    }

    // --- Calls ----------------------------------------------------------------

    fn call(&self, node: Node<'_>) -> Option<Call> {
        if matches!(node.kind(), "jsx_element" | "jsx_self_closing_element") {
            return self.jsx_call(node);
        }
        let (callee, receiver) = match node.kind() {
            "member_call_expression" | "nullsafe_member_call_expression" => (
                node.child_by_field_name("name")?,
                node.child_by_field_name("object"),
            ),
            "scoped_call_expression" => (
                node.child_by_field_name("name")?,
                node.child_by_field_name("scope"),
            ),
            _ => {
                let function = node.child_by_field_name("function")?;
                match function.kind() {
                    "member_expression" => (
                        function.child_by_field_name("property")?,
                        function.child_by_field_name("object"),
                    ),
                    "attribute" => (
                        function.child_by_field_name("attribute")?,
                        function.child_by_field_name("object"),
                    ),
                    "field_expression" => (
                        function.child_by_field_name("field")?,
                        function.child_by_field_name("value"),
                    ),
                    "scoped_identifier" | "qualified_name" => (
                        function.child_by_field_name("name").or_else(|| {
                            children(function)
                                .into_iter()
                                .rfind(tree_sitter::Node::is_named)
                        })?,
                        function
                            .child_by_field_name("path")
                            .or_else(|| function.child_by_field_name("prefix")),
                    ),
                    "generic_function" => (function.child_by_field_name("function")?, None),
                    _ => (function, None),
                }
            }
        };
        let args = node
            .child_by_field_name("arguments")
            .map_or_else(Vec::new, |list| {
                let mut cursor = list.walk();
                list.named_children(&mut cursor)
                    .filter(|a| !Kinds::is(self.kinds.comments, a.kind()))
                    .map(|a| {
                        let value = if a.kind() == "argument" {
                            a.named_child(0).unwrap_or(a)
                        } else {
                            a
                        };
                        Arg {
                            name: None,
                            text: self.text(a).trim_start_matches('$').to_owned(),
                            literal: Kinds::is(self.kinds.literals, value.kind()),
                        }
                    })
                    .collect()
            });
        Some(Call {
            caller: None,
            callee: self
                .text(callee)
                .rsplit("::")
                .next()
                .unwrap_or_default()
                .to_owned(),
            receiver: receiver
                .map(|r| self.text(r).to_owned())
                .unwrap_or_default(),
            args,
            line: line(node.start_position().row),
        })
    }

    /// `<Card title="x" />` calls the `Card` component with one named argument per
    /// prop; `{...props}` is a positional one. A plain lowercase tag such as `<div>`
    /// is a DOM element, while `<Icons.spinner>` is a component.
    fn jsx_call(&self, node: Node<'_>) -> Option<Call> {
        let tag = node.child_by_field_name("open_tag").unwrap_or(node);
        let name = tag.child_by_field_name("name")?;
        let (callee, receiver) = match name.kind() {
            "member_expression" => (
                name.child_by_field_name("property")?,
                name.child_by_field_name("object"),
            ),
            _ => (name, None),
        };
        let callee = self.text(callee);
        if receiver.is_none() && !callee.starts_with(|c: char| c.is_ascii_uppercase()) {
            return None;
        }
        let mut cursor = tag.walk();
        let args = tag
            .named_children(&mut cursor)
            .filter_map(|a| match a.kind() {
                "jsx_attribute" => {
                    let prop = a.named_child(0)?;
                    // `size={2}` passes the expression; a bare `disabled` passes `true`.
                    let value = a.named_child(1).map(|v| {
                        if v.kind() == "jsx_expression" {
                            v.named_child(0).unwrap_or(v)
                        } else {
                            v
                        }
                    });
                    Some(Arg {
                        name: Some(self.text(prop).to_owned()),
                        text: value.map_or("true", |v| self.text(v)).to_owned(),
                        literal: value.is_none_or(|v| Kinds::is(self.kinds.literals, v.kind())),
                    })
                }
                "jsx_expression" => Some(Arg {
                    name: None,
                    text: self.text(a).to_owned(),
                    literal: false,
                }),
                _ => None,
            })
            .collect();
        Some(Call {
            caller: None,
            callee: callee.to_owned(),
            receiver: receiver
                .map(|r| self.text(r).to_owned())
                .unwrap_or_default(),
            args,
            line: line(node.start_position().row),
        })
    }

    // --- Fields, errors, names, fragments -------------------------------------

    fn record_field(&mut self, node: Node<'_>) {
        let (name, type_text, value, public) = match node.kind() {
            "public_field_definition" => {
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                let private = name.kind() == "private_property_identifier"
                    || has_child_kind(node, "accessibility_modifier")
                        && !self.text(node).trim_start().starts_with("public");
                (
                    self.text(name).trim_start_matches('#'),
                    node.child_by_field_name("type"),
                    node.child_by_field_name("value"),
                    !private,
                )
            }
            "property_declaration" => {
                let mut cursor = node.walk();
                let Some(element) = node
                    .named_children(&mut cursor)
                    .find(|c| c.kind() == "property_element")
                else {
                    return;
                };
                let Some(name) = element.child_by_field_name("name") else {
                    return;
                };
                let public = self.is_public(node, "");
                (
                    self.text(name).trim_start_matches('$'),
                    node.child_by_field_name("type"),
                    element.child_by_field_name("default_value"),
                    public,
                )
            }
            "field_declaration" => {
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                (
                    self.text(name),
                    node.child_by_field_name("type"),
                    None,
                    has_child_kind(node, "visibility_modifier"),
                )
            }
            "assignment" if self.lang == Lang::Python => {
                let Some(left) = node.child_by_field_name("left") else {
                    return;
                };
                let in_class_body = node.parent().and_then(|p| p.parent()).is_some_and(|b| {
                    b.kind() == "block"
                        && b.parent().is_some_and(|c| c.kind() == "class_definition")
                });
                let name = match left.kind() {
                    "identifier" if in_class_body => self.text(left),
                    "attribute"
                        if left
                            .child_by_field_name("object")
                            .is_some_and(|o| self.text(o) == "self") =>
                    {
                        left.child_by_field_name("attribute")
                            .map(|a| self.text(a))
                            .unwrap_or_default()
                    }
                    _ => return,
                };
                (
                    name,
                    node.child_by_field_name("type"),
                    node.child_by_field_name("right"),
                    !name.starts_with('_'),
                )
            }
            _ => return,
        };
        let owner = if node.kind() == "field_declaration" {
            let mut ancestor = node.parent();
            while let Some(a) = ancestor.filter(|a| a.kind() != "struct_item") {
                ancestor = a.parent();
            }
            ancestor
                .and_then(|s| s.child_by_field_name("name"))
                .map(|n| self.text(n).to_owned())
        } else {
            self.scopes.last().map(|s| s.name.clone())
        };
        let Some(owner) = owner else { return };
        let collection = type_text.is_some_and(|t| is_collection_type(self.text(t)))
            || value.is_some_and(|v| self.is_collection_value(v));
        if self
            .facts
            .fields
            .iter()
            .any(|f| f.owner == owner && f.name == name)
        {
            return; // Python assigns `self.x` in several methods; the first one declares it.
        }
        self.facts.fields.push(Field {
            owner,
            name: name.to_owned(),
            public,
            collection,
            line: line(node.start_position().row),
        });
    }

    fn is_collection_value(&self, value: Node<'_>) -> bool {
        match value.kind() {
            "array" | "list" | "dictionary" | "set" | "array_creation_expression" => true,
            "new_expression" => value
                .child_by_field_name("constructor")
                .is_some_and(|c| is_collection_type(self.text(c))),
            "call" => value
                .child_by_field_name("function")
                .is_some_and(|f| matches!(self.text(f), "list" | "dict" | "set")),
            _ => false,
        }
    }

    fn record_ignored_error(&mut self, node: Node<'_>) {
        let guarded = match node.kind() {
            "catch_clause" | "except_clause" => {
                let body = node.child_by_field_name("body").or_else(|| {
                    let mut cursor = node.walk();
                    node.named_children(&mut cursor)
                        .find(|c| c.kind() == "block")
                });
                let Some(body) = body else { return };
                let mut cursor = body.walk();
                let handles = body.named_children(&mut cursor).any(|c| {
                    !Kinds::is(self.kinds.comments, c.kind())
                        && c.kind() != "pass_statement"
                        && self.text(c) != "..."
                });
                if handles {
                    return;
                }
                node.parent()
                    .and_then(|t| t.child_by_field_name("body"))
                    .filter(|b| self.statements(*b).len() == 1)
            }
            "let_declaration" => {
                let discards = node
                    .child_by_field_name("pattern")
                    .is_none_or(|p| self.text(p) == "_");
                match node.child_by_field_name("value") {
                    Some(value) if discards && value.kind() == "call_expression" => Some(value),
                    _ => return,
                }
            }
            "expression_statement" if self.lang == Lang::Rust => {
                let Some(call) = node
                    .named_child(0)
                    .filter(|c| c.kind() == "call_expression")
                else {
                    return;
                };
                let method = call
                    .child_by_field_name("function")
                    .filter(|f| f.kind() == "field_expression");
                match method {
                    Some(m)
                        if m.child_by_field_name("field")
                            .is_some_and(|f| self.text(f) == "ok") =>
                    {
                        m.child_by_field_name("value")
                    }
                    _ => return,
                }
            }
            _ => return,
        };
        let callee = guarded.and_then(|g| self.first_call(g)).map(|c| c.callee);
        self.facts.ignored_errors.push(IgnoredError {
            line: line(node.start_position().row),
            callee,
        });
    }

    fn first_call(&self, node: Node<'_>) -> Option<Call> {
        if Kinds::is(self.kinds.calls, node.kind()) {
            return self.call(node);
        }
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .find_map(|c| self.first_call(c))
    }

    /// Tracks declarations and uses of names inside the current function.
    fn record_name(&mut self, node: Node<'_>) {
        let Some(frame) = self.functions.last_mut() else {
            return;
        };
        let kind = node.kind();
        if !Kinds::is(self.kinds.identifiers, kind) {
            return;
        }
        let name = node
            .utf8_text(self.source)
            .unwrap_or_default()
            .trim_start_matches('$');
        let row = line(node.start_position().row);
        let declares = node.parent().is_some_and(|p| {
            // The field that holds the declared name, per declaring construct.
            let fields: &[&str] = match p.kind() {
                "variable_declarator" | "let_declaration" => &["name", "pattern"],
                "assignment" | "assignment_expression" => &["left"],
                "for_statement" | "for_in_statement" | "for_expression" => &["left", "pattern"],
                _ => &[],
            };
            fields.iter().find_map(|f| p.child_by_field_name(f)) == Some(node)
        });
        if declares {
            frame.declared.entry(name.to_owned()).or_insert(row);
        }
        let last = frame.uses.entry(name.to_owned()).or_insert(row);
        *last = (*last).max(row);
    }

    /// Statements of function bodies only: repeated declarations (types,
    /// constants, imports) are data, not code to extract into a function.
    fn record_fragments(&mut self, block: Node<'_>) {
        if self.functions.is_empty() {
            return;
        }
        let mut cursor = block.walk();
        for child in block.named_children(&mut cursor) {
            if Kinds::is(self.kinds.comments, child.kind()) {
                continue;
            }
            if let Some(fragment) = fragment::of(child, self.source, self.kinds) {
                self.facts.fragments.push(fragment);
            }
        }
    }
}

/// The last line a node occupies. Rust line comments include their newline,
/// which tree-sitter reports as ending on the next row.
fn end_line(node: Node<'_>) -> u32 {
    let end = node.end_position();
    line(if end.column == 0 && end.row > node.start_position().row {
        end.row - 1
    } else {
        end.row
    })
}

/// True when a direct child of `node`, named or anonymous, is a `kind` node.
fn has_child_kind(node: Node<'_>, kind: &str) -> bool {
    children(node).iter().any(|c| c.kind() == kind)
}

/// All children, named and anonymous.
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

fn span_of(node: Node<'_>) -> Span {
    Span {
        start: line(node.start_position().row),
        end: line(node.end_position().row),
    }
}

/// True for type text naming a mutable collection: `string[]`, `Map<K, V>`,
/// `list[int]`, `array`, `Vec<T>`, `HashMap<K, V>` and similar.
fn is_collection_type(text: &str) -> bool {
    const COLLECTIONS: [&str; 22] = [
        "Array",
        "ReadonlyArray",
        "Map",
        "Set",
        "WeakMap",
        "Record",
        "list",
        "dict",
        "set",
        "List",
        "Dict",
        "Set",
        "array",
        "Collection",
        "Vec",
        "VecDeque",
        "HashMap",
        "BTreeMap",
        "HashSet",
        "BTreeSet",
        "IndexMap",
        "IndexSet",
    ];
    let text = text.trim_start_matches(':').trim().trim_start_matches('?');
    let head = text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or_default();
    text.ends_with("[]") || COLLECTIONS.contains(&head)
}
