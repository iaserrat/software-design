//! What each grammar calls things. This is the only file that knows node kind
//! names; adding a language means adding a variant and filling in these tables.

use tree_sitter::Language;

/// A supported source language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    /// `.ts`, `.mts`, `.cts`, and `<script>` blocks of `.vue` files.
    TypeScript,
    /// `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`: the TSX grammar also parses plain JavaScript.
    Tsx,
    Python,
    Php,
    Rust,
}

impl Lang {
    /// The language of a path, by extension. Declaration files (`.d.ts`) and
    /// minified bundles (`.min.js`) are not source and return `None`.
    #[must_use]
    pub fn for_path(path: &str) -> Option<Lang> {
        if path.ends_with(".d.ts") || path.ends_with(".min.js") {
            return None;
        }
        let ext = path.rsplit_once('.')?.1;
        Some(match ext {
            "ts" | "mts" | "cts" | "vue" => Lang::TypeScript,
            "tsx" | "js" | "jsx" | "mjs" | "cjs" => Lang::Tsx,
            "py" => Lang::Python,
            "php" => Lang::Php,
            "rs" => Lang::Rust,
            _ => return None,
        })
    }

    /// The tree-sitter grammar that parses this language.
    pub(crate) fn grammar(self) -> Language {
        match self {
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// The node kind names this language's grammar uses.
    pub(crate) fn kinds(self) -> &'static Kinds {
        match self {
            Lang::TypeScript | Lang::Tsx => &TYPESCRIPT,
            Lang::Python => &PYTHON,
            Lang::Php => &PHP,
            Lang::Rust => &RUST,
        }
    }
}

/// Node kind names, grouped by the role they play in the fact model.
#[derive(Debug)]
pub(crate) struct Kinds {
    /// Declarations with `name`, `parameters` and `body` fields.
    pub functions: &'static [&'static str],
    /// Anonymous functions that take their name from an enclosing declarator.
    pub lambdas: &'static [&'static str],
    /// Anonymous functions that never have a name: their parameters are their own.
    pub closures: &'static [&'static str],
    /// Declarations that open a method scope: classes, impls, traits.
    pub scopes: &'static [&'static str],
    /// Declarations reported as types (a subset of `scopes`, plus structs and enums).
    pub types: &'static [&'static str],
    pub calls: &'static [&'static str],
    pub comments: &'static [&'static str],
    /// Statement containers: a child of one of these is a statement.
    pub blocks: &'static [&'static str],
    pub literals: &'static [&'static str],
    pub imports: &'static [&'static str],
    pub identifiers: &'static [&'static str],
    pub returns: &'static [&'static str],
    /// Wrappers whose start line is where a preceding doc comment must end.
    pub wrappers: &'static [&'static str],
    /// Nodes allowed between a doc comment and its declaration.
    pub attributes: &'static [&'static str],
}

impl Kinds {
    /// True when `kind` is one of `list`.
    pub(crate) fn is(list: &[&str], kind: &str) -> bool {
        list.contains(&kind)
    }
}

static TYPESCRIPT: Kinds = Kinds {
    functions: &[
        "function_declaration",
        "generator_function_declaration",
        "method_definition",
    ],
    lambdas: &["arrow_function", "function_expression"],
    closures: &[],
    scopes: &[
        "class_declaration",
        "abstract_class_declaration",
        "class",
        "interface_declaration",
    ],
    types: &[
        "class_declaration",
        "abstract_class_declaration",
        "interface_declaration",
        "enum_declaration",
        "type_alias_declaration",
    ],
    // JSX elements are calls of their component; see `Walker::jsx_call`.
    calls: &["call_expression", "jsx_element", "jsx_self_closing_element"],
    comments: &["comment"],
    blocks: &["program", "statement_block", "class_body"],
    literals: &[
        "string",
        "number",
        "true",
        "false",
        "null",
        "undefined",
        "template_string",
    ],
    imports: &["import_statement"],
    identifiers: &["identifier", "shorthand_property_identifier"],
    returns: &["return_statement"],
    wrappers: &["export_statement"],
    attributes: &["decorator"],
};

static PYTHON: Kinds = Kinds {
    functions: &["function_definition"],
    lambdas: &[],
    closures: &["lambda"],
    scopes: &["class_definition"],
    types: &["class_definition"],
    calls: &["call"],
    comments: &["comment"],
    blocks: &["module", "block"],
    literals: &[
        "string",
        "integer",
        "float",
        "true",
        "false",
        "none",
        "concatenated_string",
    ],
    imports: &["import_statement", "import_from_statement"],
    identifiers: &["identifier"],
    returns: &["return_statement"],
    wrappers: &["decorated_definition"],
    attributes: &["decorator"],
};

static PHP: Kinds = Kinds {
    functions: &["function_definition", "method_declaration"],
    lambdas: &[],
    closures: &["anonymous_function", "arrow_function"],
    scopes: &[
        "class_declaration",
        "trait_declaration",
        "interface_declaration",
        "enum_declaration",
    ],
    types: &[
        "class_declaration",
        "trait_declaration",
        "interface_declaration",
        "enum_declaration",
    ],
    calls: &[
        "function_call_expression",
        "member_call_expression",
        "nullsafe_member_call_expression",
        "scoped_call_expression",
    ],
    comments: &["comment"],
    blocks: &["program", "compound_statement", "declaration_list"],
    literals: &[
        "string",
        "encapsed_string",
        "integer",
        "float",
        "boolean",
        "null",
    ],
    imports: &["namespace_use_declaration"],
    identifiers: &["variable_name"],
    returns: &["return_statement"],
    wrappers: &[],
    attributes: &["attribute_list"],
};

static RUST: Kinds = Kinds {
    functions: &["function_item"],
    lambdas: &[],
    closures: &["closure_expression"],
    scopes: &["impl_item", "trait_item"],
    types: &["struct_item", "enum_item", "trait_item", "union_item"],
    calls: &["call_expression"],
    comments: &["line_comment", "block_comment"],
    blocks: &["source_file", "block", "declaration_list"],
    literals: &[
        "string_literal",
        "raw_string_literal",
        "integer_literal",
        "float_literal",
        "boolean_literal",
        "char_literal",
    ],
    imports: &["use_declaration"],
    identifiers: &["identifier"],
    returns: &["return_expression"],
    wrappers: &[],
    attributes: &["attribute_item"],
};

/// The `<script>` content of a Vue single-file component, with every line before
/// it blanked so line numbers still match the `.vue` file. `None` for other paths.
pub(crate) fn vue_script(path: &str, source: &str) -> Option<String> {
    let vue = std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("vue"));
    if !vue {
        return None;
    }
    let script = source.find("<script").and_then(|open| {
        let start = open + source[open..].find('>')? + 1;
        let end = start + source[start..].find("</script>")?;
        Some((start, end))
    });
    let Some((start, end)) = script else {
        return Some(String::new());
    };
    let blank_lines = source[..start].matches('\n').count();
    let leading = &source[source[..start].rfind('\n').map_or(0, |i| i + 1)..start];
    Some(format!(
        "{}{}{}",
        "\n".repeat(blank_lines),
        " ".repeat(leading.len()),
        &source[start..end]
    ))
}
