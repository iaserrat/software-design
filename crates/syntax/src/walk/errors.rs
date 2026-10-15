//! Syntax facts about errors.

use tree_sitter::Node;

use super::Walker;
use crate::fragment::line;
use crate::lang::{Kinds, Lang};
use crate::{Call, IgnoredError};

impl Walker<'_> {
    pub(super) fn record_ignored_error(&mut self, node: Node<'_>) {
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
}
