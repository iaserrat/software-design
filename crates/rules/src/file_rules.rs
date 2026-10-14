//! Detectors that need only one file's facts.

use serde_json::json;
use syntax::{Body, Function};

use crate::{File, Finding, Options, finding, is_test, words};

/// Generic names that say nothing about what they hold.
const GENERIC_NAMES: &[&str] = &[
    "data", "info", "tmp", "temp", "value", "val", "obj", "item", "thing", "stuff", "status",
    "flag", "res", "ret", "foo", "bar", "baz", "str", "arr", "elem",
];

/// A name used this many lines from its declaration can no longer be read
/// alongside it: the distance between declaration and use is what decides how
/// descriptive a name must be.
const VISIBLE_SPAN: u32 = 25;

/// Comment prefixes that are instructions to tools or markers, not descriptions.
const DIRECTIVES: &[&str] = &[
    "todo",
    "fixme",
    "hack",
    "xxx",
    "note",
    "eslint",
    "prettier",
    "noqa",
    "type:",
    "pylint",
    "@ts-",
    "phpcs",
    "phpstan",
    "psalm",
    "istanbul",
    "c8 ",
    "rustfmt",
    "clippy",
    "region",
    "endregion",
    "#region",
    "software-design-allow",
    "spdx",
    "copyright",
    "license",
    "-*-",
    "language=",
    "biome-ignore",
    "deno-",
    "nolint",
    "safety:",
    "mypy",
    "pyright",
];

/// Appends the findings of every detector that needs only `file`. The diff-only
/// detector runs when `options.added` is set.
pub(crate) fn check(file: &File, options: &Options<'_>, out: &mut Vec<Finding>) {
    let facts = &file.facts;
    let path = file.path.as_str();
    for function in &facts.functions {
        pass_through(path, function, out);
        exposed_collection(file, function, out);
        if function.public && function.returns_tuple && !function.overrides {
            out.push(finding(
                "tuple-return",
                path,
                function.span.start,
                &function.name,
                format!(
                    "`{}` returns a tuple, so callers read its parts by position.",
                    function.name
                ),
                json!({}),
            ));
        }
    }
    for local in &facts.locals {
        let function = &facts.functions[local.function];
        if local.name == "result" && !function.returns_value && !is_test(path) {
            out.push(finding(
                "result-without-return",
                path,
                local.declared,
                &local.name,
                format!(
                    "`result` in `{}` suggests a return value, but the function returns nothing.",
                    function.name
                ),
                json!({ "function": function.name }),
            ));
        }
        let generic = GENERIC_NAMES.contains(&local.name.as_str())
            || (local.name.chars().count() == 1
                && !matches!(local.name.as_str(), "i" | "j" | "k" | "_"));
        let span = local.last_use - local.declared;
        if generic && span > VISIBLE_SPAN {
            out.push(finding(
                "vague-name",
                path,
                local.declared,
                &local.name,
                format!(
                    "`{}` is used {span} lines after its declaration in `{}`.",
                    local.name, function.name
                ),
                json!({ "function": function.name, "last_use": local.last_use }),
            ));
        }
    }
    comments(file, out);
    interface_mentions_private(file, out);
    if let Some(added) = options.added.and_then(|a| a.get(path)) {
        let touched = |span: syntax::Span| {
            added
                .iter()
                .any(|r| r.start() <= &span.end && &span.start <= r.end())
        };
        for function in &facts.functions {
            let Some(doc) = function.doc.map(|i| &facts.comments[i]) else {
                continue;
            };
            if touched(function.header_span) && !touched(doc.span) {
                out.push(finding(
                    "stale-interface-comment",
                    path,
                    function.header_span.start,
                    &function.name,
                    format!(
                        "The signature of `{}` changed but its interface comment did not.",
                        function.name
                    ),
                    json!({ "comment_line": doc.span.start }),
                ));
            }
        }
    }
}

fn pass_through(path: &str, function: &Function, out: &mut Vec<Finding>) {
    let Body::Forward(call) = &function.body else {
        return;
    };
    let args: Vec<&str> = call.args.iter().map(|a| a.text.as_str()).collect();
    let forwards_params = args
        == function
            .params
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
    // A pass-through method calls a method of the same name on another object. A
    // differently named callee (`isSelected` -> `ids.includes`) names an operation
    // on a representation, which is information hiding, not delegation.
    let same_name = |name: &str| name.replace('_', "").to_lowercase();
    let same_api = same_name(&call.callee) == same_name(&function.name);
    let inheritance =
        matches!(call.receiver.as_str(), "super" | "parent" | "super()") || call.callee == "super";
    // 7.1 is about forwarding to another object. A call with no receiver, or on a
    // type (`Response::new`, `Foo::make`), is a free function or a constructor.
    let to_object = call
        .receiver
        .chars()
        .next()
        .is_some_and(|c| !c.is_uppercase());
    if forwards_params && same_api && to_object && !function.overrides && !inheritance {
        let target = if call.receiver.is_empty() {
            call.callee.clone()
        } else {
            format!("{}.{}", call.receiver, call.callee)
        };
        out.push(finding(
            "pass-through-method",
            path,
            function.span.start,
            &function.name,
            format!(
                "`{}` only forwards its arguments to `{target}`.",
                function.name
            ),
            json!({ "target": target, "params": function.params }),
        ));
    }
}

fn exposed_collection(file: &File, function: &Function, out: &mut Vec<Finding>) {
    let Body::ReturnsField(name) = &function.body else {
        return;
    };
    let field = file
        .facts
        .fields
        .iter()
        .find(|f| Some(&f.owner) == function.owner.as_ref() && &f.name == name);
    if function.public && field.is_some_and(|f| f.collection && !f.public) {
        out.push(finding(
            "exposed-collection",
            &file.path,
            function.span.start,
            &function.name,
            format!("`{}` returns the private collection `{name}` itself, so callers depend on its representation.", function.name),
            json!({ "field": name }),
        ));
    }
}

/// A one-word comment is a label; the rule needs a statement to compare.
const MIN_COMMENT_WORDS: usize = 2;

fn comments(file: &File, out: &mut Vec<Finding>) {
    // Test comments narrate steps ("Mock the clock"); the rule is about the
    // descriptions production code relies on.
    if is_test(&file.path) {
        return;
    }
    for comment in &file.facts.comments {
        let text = if comment.doc {
            comment.summary()
        } else {
            comment.text.as_str()
        };
        let lower = text.trim_start().to_lowercase();
        if comment.code.trim().is_empty() || DIRECTIVES.iter().any(|d| lower.starts_with(d)) {
            continue;
        }
        let said = words::content(text);
        if said.len() < MIN_COMMENT_WORDS {
            continue;
        }
        let code = words::code(&comment.code);
        if said.iter().all(|w| code.contains(w)) {
            out.push(finding(
                "comment-repeats-code",
                &file.path,
                comment.span.start,
                text.lines().next().unwrap_or_default(),
                "Every word of this comment already appears in the code it describes.".to_owned(),
                json!({ "comment": text, "code": comment.code.lines().next().unwrap_or_default() }),
            ));
        }
    }
}

fn interface_mentions_private(file: &File, out: &mut Vec<Finding>) {
    let facts = &file.facts;
    let public: Vec<&str> = facts
        .functions
        .iter()
        .filter(|f| f.public)
        .map(|f| f.name.as_str())
        .collect();
    let private: Vec<&str> = facts
        .functions
        .iter()
        .filter(|f| !f.public)
        .map(|f| f.name.as_str())
        .chain(
            facts
                .fields
                .iter()
                .filter(|f| !f.public)
                .map(|f| f.name.as_str()),
        )
        .filter(|name| words::is_identifier_shaped(name) && !public.contains(name))
        .collect();
    let documented = facts
        .functions
        .iter()
        .filter(|f| f.public)
        .map(|f| (f.name.as_str(), f.doc))
        .chain(
            facts
                .types
                .iter()
                .filter(|t| t.public)
                .map(|t| (t.name.as_str(), t.doc)),
        );
    for (name, doc) in documented {
        let Some(comment) = doc.map(|i| &facts.comments[i]) else {
            continue;
        };
        let mentioned: Vec<&str> = comment
            .text
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|token| {
                private
                    .iter()
                    .any(|p| p.trim_start_matches(['$', '#']) == *token)
                    && *token != name
            })
            .collect();
        if let Some(first) = mentioned.first() {
            out.push(finding(
                "interface-mentions-private",
                &file.path,
                comment.span.start,
                name,
                format!("The interface comment of `{name}` names the private `{first}`."),
                json!({ "private": mentioned }),
            ));
        }
    }
}
