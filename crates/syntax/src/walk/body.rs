//! Syntax facts about body.

use tree_sitter::Node;

use super::{Returns, Walker, traverse};
use crate::Body;
use crate::fragment;
use crate::lang::{Kinds, Lang};

impl Walker<'_> {
    /// Body statements, excluding comments and a leading docstring. An expression
    /// body (`x => f(x)`) counts as one statement.
    pub(super) fn statements<'t>(&self, body: Node<'t>) -> Vec<Node<'t>> {
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

    pub(super) fn body_shape(&self, statement: Node<'_>) -> Body {
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

    /// What a function returns, from its declared return type, the `return`
    /// statements of its body (not of functions nested in it), or an expression body.
    pub(super) fn returns(&self, node: Node<'_>, body: Node<'_>) -> Returns {
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
            return Returns {
                value: true,
                tuple: tuple || body.kind() == "array",
            };
        }
        let mut returns_expression = false;
        traverse(body, |current, entering| {
            if !entering {
                return false;
            }
            if Kinds::is(self.kinds.returns, current.kind()) {
                if let Some(returned) = current.named_child(0) {
                    returns_expression = true;
                    tuple |= matches!(
                        returned.kind(),
                        "expression_list" | "tuple_expression" | "tuple"
                    );
                }
                return false;
            }
            let kind = current.kind();
            current == body
                || !(Kinds::is(self.kinds.functions, kind)
                    || Kinds::is(self.kinds.lambdas, kind)
                    || Kinds::is(self.kinds.closures, kind))
        });
        Returns {
            // A Rust block's tail expression is a return with no `return`.
            value: declared_value || (self.lang != Lang::Rust && returns_expression),
            tuple,
        }
    }
    /// Statements of function bodies only: repeated declarations (types,
    /// constants, imports) are data, not code to extract into a function.
    pub(super) fn record_fragments(&mut self, block: Node<'_>) {
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
