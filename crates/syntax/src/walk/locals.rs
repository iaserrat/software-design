//! Syntax facts about locals.

use tree_sitter::Node;

use super::Walker;
use crate::fragment::line;
use crate::lang::Kinds;

impl Walker<'_> {
    /// Tracks declarations and uses of names inside the current function.
    pub(super) fn record_name(&mut self, node: Node<'_>) {
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
}
