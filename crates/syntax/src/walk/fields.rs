//! Syntax facts about fields.

use tree_sitter::Node;

use super::{Walker, has_child_kind};
use crate::Field;
use crate::fragment::line;
use crate::lang::Lang;

impl Walker<'_> {
    pub(super) fn record_field(&mut self, node: Node<'_>) {
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
}

/// True for type text naming a mutable collection: `string[]`, `Map<K, V>`,
/// `list[int]`, `array`, `Vec<T>`, `HashMap<K, V>` and similar.
fn is_collection_type(text: &str) -> bool {
    const COLLECTIONS: [&str; 21] = [
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
