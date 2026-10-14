//! Words in comments and identifiers, normalised so the two can be compared.

use std::collections::HashSet;

/// Words that carry no information about the code: articles, prepositions,
/// pronouns and auxiliary verbs.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "of", "to", "for", "in", "on", "at", "by", "with", "from", "into", "onto",
    "and", "or", "but", "is", "are", "was", "be", "been", "this", "that", "these", "those", "it",
    "its", "as", "if", "then", "so", "we", "our", "us", "you", "your", "i", "my", "should", "will",
    "can", "may", "must", "do", "does", "here", "there", "via", "per", "all", "some", "any",
    "just", "now",
];

/// Common abbreviations and synonyms, mapped to one spelling.
const CANONICAL: &[(&str, &str)] = &[
    ("ptr", "pointer"),
    ("str", "string"),
    ("num", "number"),
    ("idx", "index"),
    ("cnt", "count"),
    ("msg", "message"),
    ("req", "request"),
    ("init", "initialize"),
    ("initialise", "initialize"),
    ("obtain", "get"),
    ("retrieve", "get"),
    ("fetch", "get"),
    ("returns", "get"),
    ("return", "get"),
    ("gets", "get"),
    ("sets", "set"),
    ("assign", "set"),
    ("check", "is"),
    ("checks", "is"),
    ("whether", "is"),
];

/// The content words of comment text: lowercased, canonical, without stopwords.
pub(crate) fn content(text: &str) -> Vec<String> {
    tokens(text)
        .flat_map(split)
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Every word the identifiers of code text contain, plus the joins of adjacent
/// identifier parts, so `downCast` also yields `downcast`. String literal
/// contents are not code: a comment that matches a message string is not
/// repeating the code's logic.
pub(crate) fn code(text: &str) -> HashSet<String> {
    let mut words = HashSet::new();
    for token in tokens(&without_strings(text)) {
        let parts = split(token);
        for i in 0..parts.len() {
            let mut joined = String::new();
            for part in &parts[i..] {
                joined.push_str(part);
                words.insert(canonical(&joined));
            }
        }
    }
    words
}

/// True when `name` looks like an identifier rather than an English word:
/// it has a sigil or underscore, a digit, or an uppercase letter after the first.
pub(crate) fn is_identifier_shaped(name: &str) -> bool {
    let bare = name.trim_start_matches(['_', '$', '#']);
    bare.len() >= 4
        && (bare.len() != name.len()
            || bare.contains('_')
            || bare.chars().any(|c| c.is_ascii_digit())
            || bare.chars().skip(1).any(char::is_uppercase))
}

/// `text` with the contents of '…', "…" and `…` literals blanked out.
fn without_strings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quote = None;
    let mut escaped = false;
    for c in text.chars() {
        match quote {
            Some(_) if escaped => escaped = false,
            Some(_) if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if matches!(c, '"' | '\'' | '`') => quote = Some(c),
            None => out.push(c),
        }
        if quote.is_some() && out.ends_with(|ch: char| ch != ' ') {
            out.push(' ');
        }
    }
    out
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
}

/// Splits camelCase, `PascalCase`, `snake_case` and `SCREAMING_CASE` into lowercase words.
fn split(token: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in token.split('_').filter(|p| !p.is_empty()) {
        let chars: Vec<char> = part.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let boundary = (chars[i].is_uppercase() && chars[i - 1].is_lowercase())
                || (chars[i].is_uppercase()
                    && chars.get(i + 1).is_some_and(|n| n.is_lowercase())
                    && chars[i - 1].is_uppercase())
                || (chars[i].is_ascii_digit() != chars[i - 1].is_ascii_digit());
            if boundary {
                words.push(chars[start..i].iter().collect::<String>());
                start = i;
            }
        }
        words.push(chars[start..].iter().collect::<String>());
    }
    words
        .into_iter()
        .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
        .map(|w| canonical(&w.to_lowercase()))
        .collect()
}

/// The canonical spelling of a lowercase word: abbreviations expanded, a plural
/// `s` removed.
fn canonical(word: &str) -> String {
    if let Some((_, to)) = CANONICAL.iter().find(|(from, _)| *from == word) {
        return (*to).to_owned();
    }
    match word.strip_suffix('s') {
        Some(stem) if stem.len() >= 3 && !stem.ends_with('s') => stem.to_owned(),
        _ => word.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_every_identifier_style() {
        assert_eq!(split("hScrollBar"), ["h", "scroll", "bar"]);
        assert_eq!(split("HTTPRequest"), ["http", "request"]);
        assert_eq!(split("NUM_READ_RPC"), ["number", "read", "rpc"]);
        assert_eq!(split("get_items2"), ["get", "item"]);
    }

    #[test]
    fn code_words_include_joined_parts() {
        let words = code("downCastParameter(String parameter)");
        assert!(
            words.contains("downcast") && words.contains("parameter") && words.contains("string")
        );
    }

    #[test]
    fn string_contents_are_not_code_words() {
        let words = code(r#"fail("payment requires prescription", it's)"#);
        assert!(
            words.contains("fail") && !words.contains("payment") && !words.contains("prescription")
        );
    }

    #[test]
    fn comment_words_drop_stopwords_and_normalise() {
        assert_eq!(
            content("Obtain a normalized resource name from REQ."),
            ["get", "normalized", "resource", "name", "request"]
        );
    }

    #[test]
    fn identifier_shape_excludes_plain_words() {
        assert!(
            is_identifier_shaped("NUM_READ_RPC")
                && is_identifier_shaped("isReady")
                && is_identifier_shaped("_cache")
        );
        assert!(!is_identifier_shaped("data") && !is_identifier_shaped("Buffer"));
    }
}
