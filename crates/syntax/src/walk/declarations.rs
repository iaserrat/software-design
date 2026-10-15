//! Syntax facts about declarations.

use std::collections::HashMap;

use tree_sitter::Node;

use super::{Frame, Scope, Walker, children, has_child_kind, span_of};
use crate::fragment::{self, line};
use crate::lang::{Kinds, Lang};
use crate::{Body, Function, Span, TypeDecl};

impl Walker<'_> {
    pub(super) fn enter_scope(&mut self, node: Node<'_>) {
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

    pub(super) fn record_type(&mut self, node: Node<'_>) {
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
        let doc = self.attach_doc(node, node.child_by_field_name("body"), &header);
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
    pub(super) fn lambda_declarator<'t>(&self, node: Node<'t>) -> Option<Node<'t>> {
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

    pub(super) fn enter_function(&mut self, node: Node<'_>) {
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
        let doc = self.attach_doc(declaration, Some(body), &header);
        let statements = self.statements(body);
        let body_shape = if statements.len() == 1 {
            self.body_shape(statements[0])
        } else {
            Body::Other
        };
        let returns = self.returns(node, body);
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
            returns_value: returns.value,
            returns_tuple: returns.tuple,
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

    pub(super) fn is_public(&self, node: Node<'_>, name: &str) -> bool {
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

    pub(super) fn params(&self, node: Node<'_>) -> Vec<String> {
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
}
