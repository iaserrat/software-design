//! Comment text: stripping markers and finding the descriptive first paragraph.

/// Comment text without `//`, `#`, `/* */`, `*`, `///`, `//!` or triple quotes,
/// one cleaned line per source line.
pub(crate) fn clean(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            let line = line.trim();
            let line = line.strip_suffix("*/").unwrap_or(line);
            let line = line
                .strip_prefix("/**")
                .or_else(|| line.strip_prefix("/*!"))
                .or_else(|| line.strip_prefix("/*"))
                .or_else(|| line.strip_prefix("///"))
                .or_else(|| line.strip_prefix("//!"))
                .or_else(|| line.strip_prefix("//"))
                .or_else(|| line.strip_prefix('#'))
                .or_else(|| line.strip_prefix("\"\"\""))
                .or_else(|| line.strip_prefix("'''"))
                .or_else(|| line.strip_prefix('*'))
                .unwrap_or(line);
            let line = line
                .strip_suffix("\"\"\"")
                .or_else(|| line.strip_suffix("'''"))
                .unwrap_or(line);
            line.trim()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// The text up to the first blank line or tag section.
pub(crate) fn summary(text: &str) -> &str {
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || is_section(trimmed) {
            break;
        }
        end += line.len();
    }
    text[..end].trim_end()
}

fn is_section(line: &str) -> bool {
    const SECTIONS: [&str; 9] = [
        "Args:",
        "Arguments:",
        "Returns:",
        "Raises:",
        "Parameters",
        "Examples",
        ":param",
        ":return",
        ":raises",
    ];
    line.starts_with('@') || line.starts_with("# ") || SECTIONS.iter().any(|s| line.starts_with(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markers_of_every_style() {
        assert_eq!(clean("/**\n * Doc for Bar.\n */"), "Doc for Bar.");
        assert_eq!(clean("/// Get the items."), "Get the items.");
        assert_eq!(clean("# add one"), "add one");
        assert_eq!(clean("\"\"\"Doc for Bar.\"\"\""), "Doc for Bar.");
    }

    #[test]
    fn summary_stops_at_tags_and_blank_lines() {
        assert_eq!(summary("Copy bytes.\n@param n count"), "Copy bytes.");
        assert_eq!(
            summary("Copy bytes.\nMore.\n\nDetails"),
            "Copy bytes.\nMore."
        );
        assert_eq!(summary("Copy bytes.\n# Errors\nNone"), "Copy bytes.");
    }
}
