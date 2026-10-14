//! Structural hashes of statements, for finding repeated code.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};

use tree_sitter::Node;

use crate::lang::Kinds;
use crate::{Fragment, Span};

/// Statements spanning fewer code lines than this are not worth extracting
/// into a function: the call would be about as long as the code it replaces.
const MIN_LINES: u32 = 3;

/// A fragment for `node` when it has at least [`MIN_LINES`] lines of code.
pub(crate) fn of(node: Node<'_>, source: &[u8], kinds: &Kinds) -> Option<Fragment> {
    let mut hasher = DefaultHasher::new();
    let mut code_lines = HashSet::new();
    let mut identifiers = HashSet::new();
    let mut cursor = node.walk();
    'outer: loop {
        let current = cursor.node();
        let kind = current.kind();
        if Kinds::is(kinds.comments, kind) {
            // Comments are not code: skip the subtree.
        } else if current.child_count() == 0 || Kinds::is(kinds.literals, kind) {
            let text = current.utf8_text(source).unwrap_or_default();
            if text.chars().any(char::is_alphanumeric) {
                code_lines.insert(current.start_position().row);
            }
            // Tokens and the names code depends on (called functions, members)
            // hash by text; local names and literals hash by kind, so copies that
            // only rename variables or change constants still match.
            if !current.is_named() || is_dependency(&cursor) {
                text.hash(&mut hasher);
            } else {
                kind.hash(&mut hasher);
            }
            if current.is_named()
                && (Kinds::is(kinds.identifiers, kind) || kind.ends_with("identifier"))
            {
                identifiers.insert(text);
            }
        } else {
            kind.hash(&mut hasher);
            if cursor.goto_first_child() {
                continue;
            }
        }
        loop {
            if cursor.node() == node {
                break 'outer;
            }
            if cursor.goto_next_sibling() {
                continue 'outer;
            }
            // Mark the end of a child list so `f(a)(b)` and `f(a(b))` differ.
            0u8.hash(&mut hasher);
            if !cursor.goto_parent() {
                break 'outer;
            }
        }
    }
    let lines = u32::try_from(code_lines.len()).unwrap_or(u32::MAX);
    (lines >= MIN_LINES).then(|| Fragment {
        hash: hasher.finish(),
        span: Span {
            start: line(node.start_position().row),
            end: line(node.end_position().row),
        },
        lines,
        identifiers: u32::try_from(identifiers.len()).unwrap_or(u32::MAX),
    })
}

/// True for a name the code depends on rather than one it chooses: a member,
/// field or property name, or the name of a called function.
fn is_dependency(cursor: &tree_sitter::TreeCursor<'_>) -> bool {
    let node = cursor.node();
    let parent = node.parent().map(|p| p.kind()).unwrap_or_default();
    matches!(
        node.kind(),
        "property_identifier"
            | "field_identifier"
            | "private_property_identifier"
            | "shorthand_property_identifier"
    ) || matches!(
        cursor.field_name(),
        Some("function" | "attribute" | "property" | "field")
    ) || (cursor.field_name() == Some("name")
        && matches!(
            parent,
            "scoped_identifier"
                | "member_call_expression"
                | "nullsafe_member_call_expression"
                | "member_access_expression"
                | "scoped_call_expression"
                | "function_call_expression"
        ))
}

/// A 0-based tree-sitter row as a 1-based line number.
pub(crate) fn line(row: usize) -> u32 {
    u32::try_from(row + 1).unwrap_or(u32::MAX)
}
