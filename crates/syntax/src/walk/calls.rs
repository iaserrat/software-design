//! Syntax facts about calls.

use tree_sitter::Node;

use super::{Walker, children};
use crate::fragment::line;
use crate::lang::Kinds;
use crate::{Arg, Call};

impl Walker<'_> {
    pub(super) fn call(&self, node: Node<'_>) -> Option<Call> {
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
}
