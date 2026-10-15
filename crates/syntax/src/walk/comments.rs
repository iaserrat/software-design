//! Syntax facts about comments.

use tree_sitter::Node;

use super::{Walker, traverse};
use crate::fragment::line;
use crate::lang::{Kinds, Lang};
use crate::{Comment, Span, comment};

/// A comment on its own line describes at most this many following lines of
/// code. A longer reach would compare the comment with code it does not describe.
const DESCRIBED_LINES: usize = 3;

impl Walker<'_> {
    /// Merges comment nodes into blocks and records which line each doc comment
    /// attaches to, before the main walk needs it.
    pub(super) fn collect_comments(&mut self, root: Node<'_>) {
        let mut nodes = Vec::new();
        traverse(root, |node, entering| {
            let comment = Kinds::is(self.kinds.comments, node.kind());
            if entering && comment {
                nodes.push(node);
            }
            !comment
        });
        let mut i = 0;
        while i < nodes.len() {
            let first = nodes[i];
            let doc = self.is_doc_comment(first);
            let mut last = first;
            while let Some(&next) = nodes.get(i + 1) {
                let joins = next.start_position().row == end_line(last) as usize;
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
    pub(super) fn wrapped<'t>(&self, node: Node<'t>) -> Node<'t> {
        let mut outer = node;
        while let Some(parent) = outer
            .parent()
            .filter(|p| Kinds::is(self.kinds.wrappers, p.kind()))
        {
            outer = parent;
        }
        outer
    }

    /// Associates a declaration's comment with its signature for comment rules.
    pub(super) fn attach_doc(
        &mut self,
        declaration: Node<'_>,
        body: Option<Node<'_>>,
        header: &str,
    ) -> Option<usize> {
        let doc = match (self.lang, body) {
            (Lang::Python, Some(body)) => self.docstring(body),
            _ => self.doc_for(declaration),
        };
        if let Some(index) = doc {
            header.clone_into(&mut self.facts.comments[index].code);
        }
        doc
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
    fn docstring(&mut self, body: Node<'_>) -> Option<usize> {
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
            code: String::new(),
        });
        Some(self.facts.comments.len() - 1)
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
