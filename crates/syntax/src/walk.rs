//! One pass over a syntax tree that fills a [`FileFacts`].

mod body;
mod calls;
mod comments;
mod declarations;
mod errors;
mod fields;
mod locals;

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use crate::fragment::line;
use crate::lang::{Kinds, Lang};
use crate::{FileFacts, Local, Span};

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
    traverse(tree.root_node(), |node, entering| {
        if entering {
            walker.enter(node);
        } else {
            walker.exit(node);
        }
        true
    });
    walker.facts
}

/// Visits `root` and its descendants depth first without recursing, so deep trees
/// cannot overflow the stack. `visit(node, true)` runs on entry and returns whether
/// to descend; `visit(node, false)` runs once the node's children are done.
fn traverse<'t>(root: Node<'t>, mut visit: impl FnMut(Node<'t>, bool) -> bool) {
    let mut cursor = root.walk();
    loop {
        if visit(cursor.node(), true) && cursor.goto_first_child() {
            continue;
        }
        loop {
            visit(cursor.node(), false);
            if cursor.node() == root {
                return;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// What a function body returns.
struct Returns {
    /// A value, not just control: `void`, `None` and `()` are no value.
    value: bool,
    /// Several values by position: a tuple, a list literal or `[A, B]` type.
    tuple: bool,
}

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
