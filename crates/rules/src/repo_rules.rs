//! Detectors that need every file: call counts, duplicates and co-change.
//!
//! Calls are matched to definitions by name only. A name defined more than once
//! in the codebase is ambiguous, so detectors that count callers skip it.

use std::collections::{BTreeSet, HashMap};

use serde_json::json;
use syntax::{Body, Call, Function, Span};

use crate::{Codebase, File, Finding, finding, is_test};

/// Two files must change together at least this often before co-change counts as
/// a dependency rather than coincidence.
const MIN_TOGETHER: u32 = 3;

/// A function needs at least this many call sites before "every caller passes the
/// same value" is a pattern rather than a coincidence.
const MIN_CALL_SITES: usize = 3;

/// The most lines a declaration may span, signature and braces included, to count
/// as shallow.
const SHALLOW_LINES: u32 = 3;

const CONSTRUCTORS: &[&str] = &["constructor", "__init__", "__construct", "new", "default"];

/// Appends the findings of every detector that compares files: callers,
/// copies and co-change. Without history, hidden coupling is not checked.
pub(crate) fn check(codebase: &Codebase, out: &mut Vec<Finding>) {
    let mut definitions: HashMap<&str, Vec<(&File, &Function)>> = HashMap::new();
    let mut calls: HashMap<&str, Vec<(&File, &Call)>> = HashMap::new();
    for file in &codebase.files {
        for function in &file.facts.functions {
            definitions
                .entry(function.name.as_str())
                .or_default()
                .push((file, function));
        }
        for call in &file.facts.calls {
            calls
                .entry(call.callee.as_str())
                .or_default()
                .push((file, call));
        }
    }
    let unique = |name: &str| match definitions.get(name).map(Vec::as_slice) {
        Some([only]) => Some(*only),
        _ => None,
    };
    for (name, sites) in &calls {
        let Some((file, function)) = unique(name) else {
            continue;
        };
        if is_test(&file.path) {
            continue;
        }
        // Recursive calls are not callers.
        let sites: Vec<_> = sites
            .iter()
            .filter(|(f, c)| {
                !(f.path == file.path
                    && c.caller
                        .is_some_and(|i| file.facts.functions[i].span == function.span))
            })
            .collect();
        shallow_function(file, function, &sites, out);
        constant_argument(file, function, &sites, out);
    }
    ignored_errors(codebase, out);
    repeated_code(codebase, out);
    hidden_coupling(codebase, out);
}

fn shallow_function(
    file: &File,
    function: &Function,
    sites: &[&(&File, &Call)],
    out: &mut Vec<Finding>,
) {
    let forwards = matches!(&function.body, Body::Forward(call)
        if call.args.iter().map(|a| a.text.as_str()).eq(function.params.iter().map(String::as_str)));
    // A one-statement body that spans many lines (a `match`, a long iterator
    // chain) does real work behind its name; a shallow function is a line or two.
    let short = function.span.end - function.span.start < SHALLOW_LINES;
    if function.statements != 1
        || !short
        || function.overrides
        || forwards
        || CONSTRUCTORS.contains(&function.name.as_str())
    {
        return;
    }
    let [(caller_file, call)] = sites else { return };
    out.push(finding(
        "shallow-function",
        &file.path,
        function.span.start,
        &function.name,
        format!(
            "`{}` has one statement and one caller, at {}:{}.",
            function.name, caller_file.path, call.line
        ),
        json!({ "caller": format!("{}:{}", caller_file.path, call.line) }),
    ));
}

fn constant_argument(
    file: &File,
    function: &Function,
    sites: &[&(&File, &Call)],
    out: &mut Vec<Finding>,
) {
    if sites.len() < MIN_CALL_SITES {
        return;
    }
    // Positional arguments line up by position; named ones (JSX props) by name,
    // whatever order each call site writes them in.
    let named: BTreeSet<&str> = sites
        .iter()
        .flat_map(|(_, call)| call.args.iter().filter_map(|a| a.name.as_deref()))
        .collect();
    let slots = (function.params.iter().map(String::as_str).enumerate())
        .map(|(position, param)| (param, Some(position)))
        .chain(named.into_iter().map(|name| (name, None)));
    for (param, position) in slots {
        let mut values = sites.iter().map(|(_, call)| {
            let arg = match position {
                Some(position) => call.args.iter().filter(|a| a.name.is_none()).nth(position),
                None => call.args.iter().find(|a| a.name.as_deref() == Some(param)),
            };
            arg.filter(|a| a.literal).map(|a| a.text.as_str())
        });
        let Some(Some(first)) = values.next() else {
            continue;
        };
        if values.all(|v| v == Some(first)) {
            out.push(finding(
                "constant-argument",
                &file.path,
                function.span.start,
                &function.name,
                format!(
                    "All {} calls to `{}` pass `{first}` as `{param}`.",
                    sites.len(),
                    function.name
                ),
                json!({ "param": param, "value": first, "call_sites": sites.len() }),
            ));
        }
    }
}

fn ignored_errors(codebase: &Codebase, out: &mut Vec<Finding>) {
    let mut per_callee: HashMap<&str, u32> = HashMap::new();
    for file in &codebase.files {
        for ignored in &file.facts.ignored_errors {
            if let Some(callee) = &ignored.callee {
                *per_callee.entry(callee.as_str()).or_default() += 1;
            }
        }
    }
    for file in codebase.files.iter().filter(|f| !is_test(&f.path)) {
        for ignored in &file.facts.ignored_errors {
            let callee = ignored.callee.as_deref().unwrap_or("");
            let discards = per_callee.get(callee).copied().unwrap_or(1);
            let message = if callee.is_empty() {
                "This handler discards the error.".to_owned()
            } else {
                format!(
                    "This handler discards the error from `{callee}` ({discards} such discard(s) in the codebase)."
                )
            };
            out.push(finding(
                "ignored-error",
                &file.path,
                ignored.line,
                callee,
                message,
                json!({ "callee": callee, "discards": discards }),
            ));
        }
    }
}

fn repeated_code(codebase: &Codebase, out: &mut Vec<Finding>) {
    let mut groups: HashMap<u64, Vec<(&str, &syntax::Fragment)>> = HashMap::new();
    for file in codebase.files.iter().filter(|f| !is_test(&f.path)) {
        for fragment in &file.facts.fragments {
            groups
                .entry(fragment.hash)
                .or_default()
                .push((file.path.as_str(), fragment));
        }
    }
    let mut groups: Vec<_> = groups.into_values().filter(|g| g.len() > 1).collect();
    // Largest first, so a copy inside an already reported copy is not reported again.
    groups.sort_by(|a, b| {
        b[0].1
            .lines
            .cmp(&a[0].1.lines)
            .then_with(|| a[0].0.cmp(b[0].0))
            .then_with(|| a[0].1.span.start.cmp(&b[0].1.span.start))
    });
    let mut reported: Vec<(&str, Span)> = Vec::new();
    for group in groups {
        let inside = |(path, fragment): &(&str, &syntax::Fragment)| {
            reported.iter().any(|(p, s)| {
                p == path && s.start <= fragment.span.start && fragment.span.end <= s.end
            })
        };
        if group.iter().all(inside) {
            continue;
        }
        let (path, first) = group[0];
        let copies: Vec<String> = group
            .iter()
            .map(|(p, f)| format!("{p}:{}", f.span.start))
            .collect();
        out.push(finding(
            "repeated-code",
            path,
            first.span.start,
            "",
            format!(
                "{} lines of code appear {} times with the same structure.",
                first.lines,
                group.len()
            ),
            json!({ "copies": copies, "lines": first.lines, "identifiers": first.identifiers }),
        ));
        reported.extend(group.iter().map(|(p, f)| (*p, f.span)));
    }
}

fn hidden_coupling(codebase: &Codebase, out: &mut Vec<Finding>) {
    let Some(history) = &codebase.history else {
        return;
    };
    let files = codebase.by_path();
    let mut pairs: Vec<_> = history.together.iter().collect();
    pairs.sort();
    for ((a, b), &together) in pairs {
        let (Some(file_a), Some(file_b)) = (files.get(a.as_str()), files.get(b.as_str())) else {
            continue;
        };
        let least = history
            .churn
            .get(a)
            .copied()
            .unwrap_or(0)
            .min(history.churn.get(b).copied().unwrap_or(0));
        if together < MIN_TOGETHER || together * 2 < least {
            continue;
        }
        let (stem_a, stem_b) = (module_stem(a), module_stem(b));
        let imports = |file: &File, stem: &str| file.facts.imports.iter().any(|i| i.contains(stem));
        let test_pair =
            is_test(a) || is_test(b) || stem_a.contains(stem_b) || stem_b.contains(stem_a);
        // A call to a function the other file defines is a visible dependency even
        // without an import statement (auto-imports, star imports, globals).
        let calls_into = |caller: &File, callee: &File| {
            caller.facts.calls.iter().any(|c| {
                callee
                    .facts
                    .functions
                    .iter()
                    .any(|f| f.public && f.name == c.callee)
            })
        };
        if test_pair
            || imports(file_a, stem_b)
            || imports(file_b, stem_a)
            || calls_into(file_a, file_b)
            || calls_into(file_b, file_a)
        {
            continue;
        }
        out.push(finding(
            "hidden-coupling",
            a,
            1,
            b,
            format!("`{a}` and `{b}` changed together in {together} commits, but neither imports or calls the other."),
            json!({ "other": b, "together": together }),
        ));
    }
}

/// The name other files use to import a path: the file stem, or the directory
/// name for `index.ts`, `mod.rs` and `__init__.py`.
fn module_stem(path: &str) -> &str {
    let mut parts = path.rsplit('/');
    let name = parts.next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    if matches!(stem, "index" | "mod" | "__init__" | "lib" | "main") {
        parts.next().unwrap_or(stem)
    } else {
        stem
    }
}
