//! Design rules about module depth, information hiding, naming, comments and
//! errors, applied to extracted facts.
//!
//! [`check`] runs every detector over a [`Codebase`] and returns [`Finding`]s.
//! Detector metadata (tier, red flag, exemptions, fixes) lives in the
//! repository's `rules.json`, which the plugin's skills read too, so each rule is
//! described in exactly one place.
//!
//! A finding is evidence that a red flag *may* apply. Tier A detectors are
//! precise enough to report directly; tier B findings need a reader to apply the
//! exemptions listed in `rules.json` before anyone acts on them.

mod file_rules;
mod repo_rules;
mod words;

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The rule table, shared with the plugin's skills.
pub const RULES_JSON: &str = include_str!("../../../rules.json");

/// Version of the JSON this crate emits. Bumped on any incompatible change.
pub const SCHEMA_VERSION: u32 = 2;

/// One parsed source file.
#[derive(Debug)]
pub struct File {
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
    pub facts: syntax::FileFacts,
}

/// The files to check and, for repository-wide rules, their git history.
#[derive(Debug, Default)]
pub struct Codebase {
    pub files: Vec<File>,
    pub history: Option<history::History>,
}

impl Codebase {
    /// The files keyed by path.
    pub(crate) fn by_path(&self) -> HashMap<&str, &File> {
        self.files.iter().map(|f| (f.path.as_str(), f)).collect()
    }
}

/// How much evidence a detector's signal is on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tier {
    /// Precise: report as found. Every tier A detector needs only one file.
    A,
    /// A candidate: a reader must apply the rule's exemptions first.
    B,
}

/// Parses `A` or `B`, in either case, as the CLI and MCP tools accept it.
impl FromStr for Tier {
    type Err = String;

    fn from_str(text: &str) -> Result<Tier, String> {
        match text {
            "A" | "a" => Ok(Tier::A),
            "B" | "b" => Ok(Tier::B),
            other => Err(format!("tier must be A or B, not {other}")),
        }
    }
}

/// Lines added per path, as returned by `history::Repo::added_lines`.
pub type AddedLines = HashMap<String, Vec<RangeInclusive<u32>>>;

/// What to report.
#[derive(Debug, Clone, Copy)]
pub struct Options<'a> {
    /// The least precise tier to run: `Tier::A` runs only tier A detectors.
    pub tier: Tier,
    /// When set, report only findings on these lines, and run diff-only rules.
    pub added: Option<&'a AddedLines>,
}

/// Evidence that a design rule may be broken at one place.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Detector id from `rules.json`.
    pub rule: &'static str,
    pub path: String,
    pub line: u32,
    /// The function, type or name the finding is about.
    pub symbol: String,
    pub message: String,
    pub evidence: Map<String, Value>,
    /// Commits that touched the file in the history window. Complexity costs in
    /// proportion to how often people work in the code, so these findings matter more.
    pub weight: u32,
}

/// A detector's entry in `rules.json`.
#[derive(Debug, Deserialize)]
pub struct Detector {
    pub id: String,
    /// The red flag or principle id this detector gives evidence for.
    pub flags: String,
    pub tier: Tier,
}

/// Detector entries from `rules.json`, parsed once.
///
/// # Panics
/// When the bundled `rules.json` is malformed, which the crate's tests rule out.
pub fn detectors() -> &'static [Detector] {
    #[derive(Deserialize)]
    struct Table {
        detectors: Vec<Detector>,
    }
    static DETECTORS: OnceLock<Vec<Detector>> = OnceLock::new();
    DETECTORS.get_or_init(|| {
        serde_json::from_str::<Table>(RULES_JSON)
            .expect("rules.json matches `Detector`")
            .detectors
    })
}

fn detector(id: &str) -> &'static Detector {
    detectors()
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("detector {id} is missing from rules.json"))
}

/// Runs the detectors `options` selects and returns findings ordered by weight,
/// then tier, then location. Findings on lines a `software-design-allow: <rule>
/// <reason>` comment covers are dropped.
#[must_use]
pub fn check(codebase: &Codebase, options: &Options<'_>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in &codebase.files {
        file_rules::check(file, options, &mut findings);
    }
    if options.tier >= Tier::B {
        repo_rules::check(codebase, &mut findings);
    }
    let by_path = codebase.by_path();
    findings.retain(|f| {
        detector(f.rule).tier <= options.tier
            && by_path
                .get(f.path.as_str())
                .is_none_or(|file| !allowed(file, f))
            && options.added.is_none_or(|added| in_added(added, f))
    });
    if let Some(history) = &codebase.history {
        for finding in &mut findings {
            finding.weight = history.churn.get(&finding.path).copied().unwrap_or(0);
        }
    }
    findings.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| detector(a.rule).tier.cmp(&detector(b.rule).tier))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    findings
}

/// The JSON report: `{ "schema_version", "findings": [...] }`. Each finding
/// carries its detector's tier and red flag from `rules.json`.
#[must_use]
pub fn report(findings: &[Finding]) -> Value {
    let findings: Vec<Value> = findings
        .iter()
        .map(|f| {
            let d = detector(f.rule);
            json!({
                "rule": f.rule,
                "flags": d.flags,
                "tier": d.tier,
                "path": f.path,
                "line": f.line,
                "symbol": f.symbol,
                "message": f.message,
                "evidence": f.evidence,
                "weight": f.weight,
            })
        })
        .collect();
    json!({ "schema_version": SCHEMA_VERSION, "findings": findings })
}

/// True when a comment on the finding's line or the line above says
/// `software-design-allow: <rule> <reason>`. A suppression without a reason
/// does not count: the reason is the design decision the next reader needs.
fn allowed(file: &File, finding: &Finding) -> bool {
    file.facts.comments.iter().any(|c| {
        (c.span.contains(finding.line) || c.span.end + 1 == finding.line)
            && c.text.lines().any(|line| {
                line.split_once("software-design-allow:")
                    .and_then(|(_, rest)| rest.trim().split_once(char::is_whitespace))
                    .is_some_and(|(rule, reason)| rule == finding.rule && !reason.trim().is_empty())
            })
    })
}

fn in_added(added: &AddedLines, finding: &Finding) -> bool {
    added.get(&finding.path).is_some_and(|ranges| {
        // A file-pair finding is anchored at line 1; any change to the file counts.
        finding.rule == "hidden-coupling" || ranges.iter().any(|r| r.contains(&finding.line))
    })
}

/// True for paths that hold tests, by the common conventions of each language.
pub(crate) fn is_test(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    path.split('/').any(|dir| {
        matches!(
            dir,
            "test" | "tests" | "__tests__" | "spec" | "specs" | "testing"
        )
    }) || name.contains(".test.")
        || name.contains(".spec.")
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("Test")
        || stem.ends_with("_spec")
}

fn finding(
    rule: &'static str,
    path: &str,
    line: u32,
    symbol: &str,
    message: String,
    evidence: Value,
) -> Finding {
    Finding {
        rule,
        path: path.to_owned(),
        line,
        symbol: symbol.to_owned(),
        message,
        evidence: match evidence {
            Value::Object(map) => map,
            _ => Map::new(),
        },
        weight: 0,
    }
}

/// Whether the body forwards every parameter unchanged and in order.
fn forwards_params(function: &syntax::Function) -> bool {
    matches!(&function.body, syntax::Body::Forward(call)
        if call.args.iter().map(|arg| arg.text.as_str())
            .eq(function.params.iter().map(String::as_str)))
}

#[cfg(test)]
mod tests {
    use super::forwards_params;

    #[test]
    fn forwarding_requires_all_parameters_in_order() {
        for (source, expected) in [
            ("fn f(a: i32, b: i32) { target(a, b) }", true),
            ("fn f(a: i32, b: i32) { target(b, a) }", false),
            ("fn f(a: i32, b: i32) { target(a) }", false),
            ("fn f(a: i32) { target(a, 1) }", false),
            ("fn f(a: i32) { target(1) }", false),
            ("fn f() { target() }", true),
            ("fn f(a: i32) { a + 1 }", false),
        ] {
            let facts = syntax::parse("test.rs", source).expect("Rust facts");
            assert_eq!(forwards_params(&facts.functions[0]), expected, "{source}");
        }
    }
}
