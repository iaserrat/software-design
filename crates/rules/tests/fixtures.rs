//! Behaviour of every detector, by example.
//!
//! `tests/fixtures/<rule>.pos.<ext>` must produce at least one finding of `<rule>`;
//! `<rule>.neg.<ext>` must produce none. Negative fixtures encode each rule's
//! exemptions from `rules.json`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use rules::{Codebase, File, Finding, Options, Tier, check, detectors, report};

fn file(path: &str, source: &str) -> File {
    File {
        path: path.to_owned(),
        facts: syntax::parse(path, source).expect("supported language"),
    }
}

fn run(files: Vec<File>, options: &Options<'_>) -> Vec<Finding> {
    check(
        &Codebase {
            files,
            history: None,
        },
        options,
    )
}

const ALL: Options<'static> = Options {
    tier: Tier::B,
    added: None,
};

#[test]
fn every_fixture_behaves_as_its_name_says() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut failures = Vec::new();
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("fixtures dir") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 name")
            .to_owned();
        let mut parts = name.splitn(3, '.');
        let (rule, kind) = (
            parts.next().expect("rule"),
            parts.next().expect("pos or neg"),
        );
        let source = std::fs::read_to_string(&path).expect("read fixture");
        let hits = run(vec![file(&name, &source)], &ALL)
            .into_iter()
            .filter(|f| f.rule == rule)
            .count();
        let ok = if kind == "pos" { hits > 0 } else { hits == 0 };
        if !ok {
            failures.push(format!("{name}: {hits} finding(s) of {rule}"));
        }
        checked += 1;
    }
    assert!(checked > 0);
    assert!(
        failures.is_empty(),
        "fixtures failed:\n{}",
        failures.join("\n")
    );
}

/// Every detector in `rules.json` has examples in each language it supports.
#[test]
fn every_detector_has_positive_and_negative_examples() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let names: BTreeSet<String> = std::fs::read_dir(&dir)
        .expect("fixtures dir")
        .map(|e| e.expect("entry").file_name().into_string().expect("utf-8"))
        .collect();
    // Tested below with git history or a diff instead of a single file.
    let programmatic = ["hidden-coupling", "stale-interface-comment"];
    for detector in detectors()
        .iter()
        .filter(|d| !programmatic.contains(&d.id.as_str()))
    {
        for ext in ["ts", "py", "php", "rs"] {
            // PHP has no tuples.
            if detector.id == "tuple-return" && ext == "php" {
                continue;
            }
            for kind in ["pos", "neg"] {
                let fixture = format!("{}.{kind}.{ext}", detector.id);
                assert!(names.contains(&fixture), "missing fixture {fixture}");
            }
        }
    }
}

#[test]
fn files_that_change_together_without_importing_each_other_are_coupled() {
    let files = vec![
        file("api/orders.ts", "export function total() { return 1; }\n"),
        file("web/OrderView.ts", "import { format } from './format';\n"),
        file("web/format.ts", "import { x } from '../api/orders';\n"),
    ];
    let mut history = history::History::default();
    for path in ["api/orders.ts", "web/OrderView.ts", "web/format.ts"] {
        history.churn.insert(path.to_owned(), 4);
    }
    history.together.insert(
        ("api/orders.ts".to_owned(), "web/OrderView.ts".to_owned()),
        4,
    );
    history
        .together
        .insert(("api/orders.ts".to_owned(), "web/format.ts".to_owned()), 4);
    let findings = check(
        &Codebase {
            files,
            history: Some(history),
        },
        &ALL,
    );
    let coupled: Vec<_> = findings
        .iter()
        .filter(|f| f.rule == "hidden-coupling")
        .map(|f| f.symbol.as_str())
        .collect();
    assert_eq!(
        coupled,
        ["web/OrderView.ts"],
        "format.ts imports orders, so only OrderView is hidden"
    );
    assert!(findings.iter().all(|f| f.weight == 4));
}

#[test]
fn a_signature_change_without_a_comment_change_is_stale() {
    let source = "/** Sums two numbers. */\nexport function add(a: number, b: number, c: number): number { return a + b + c; }\n";
    let files = || vec![file("math.ts", source)];
    let signature_only = HashMap::from([("math.ts".to_owned(), vec![2..=2])]);
    let both = HashMap::from([("math.ts".to_owned(), vec![1..=2])]);
    let stale = |added| {
        run(
            files(),
            &Options {
                tier: Tier::A,
                added: Some(added),
            },
        )
        .iter()
        .any(|f| f.rule == "stale-interface-comment")
    };
    assert!(stale(&signature_only));
    assert!(!stale(&both));
    assert!(
        !run(files(), &ALL)
            .iter()
            .any(|f| f.rule == "stale-interface-comment"),
        "needs a diff"
    );
}

#[test]
fn diff_mode_reports_only_findings_on_added_lines() {
    let source = "export class A {\n  get(x: number) { return this.inner.get(x); }\n  put(x: number) { return this.inner.put(x); }\n}\n";
    let added = HashMap::from([("a.ts".to_owned(), vec![3..=3])]);
    let findings = run(
        vec![file("a.ts", source)],
        &Options {
            tier: Tier::A,
            added: Some(&added),
        },
    );
    assert_eq!(
        findings
            .iter()
            .map(|f| f.symbol.as_str())
            .collect::<Vec<_>>(),
        ["put"]
    );
}

#[test]
fn tier_a_runs_no_candidate_detectors() {
    let source = "export function pair(): [number, boolean] { return [1, true]; }\n";
    let findings = run(
        vec![file("p.ts", source)],
        &Options {
            tier: Tier::A,
            added: None,
        },
    );
    assert_eq!(findings, [] as [rules::Finding; 0]);
    assert!(
        detectors()
            .iter()
            .filter(|d| d.tier == Tier::A)
            .all(|d| d.id != "tuple-return")
    );
}

#[test]
fn an_allow_comment_with_a_reason_suppresses_one_rule() {
    let with_reason = "export class A {\n  // software-design-allow: pass-through-method keeps the v1 API stable\n  get(x: number) { return this.inner.get(x); }\n}\n";
    let without_reason = "export class A {\n  // software-design-allow: pass-through-method\n  get(x: number) { return this.inner.get(x); }\n}\n";
    assert!(
        run(vec![file("a.ts", with_reason)], &ALL)
            .iter()
            .all(|f| f.rule != "pass-through-method")
    );
    assert!(
        run(vec![file("a.ts", without_reason)], &ALL)
            .iter()
            .any(|f| f.rule == "pass-through-method")
    );
}

#[test]
fn the_report_carries_rule_tier_and_schema_version() {
    let source = "export class A {\n  get(x: number) { return this.inner.get(x); }\n}\n";
    let json = report(&run(vec![file("a.ts", source)], &ALL));
    assert_eq!(json["schema_version"], 2);
    let first = &json["findings"][0];
    assert_eq!(
        (first["rule"].as_str(), first["tier"].as_str()),
        (Some("pass-through-method"), Some("A"))
    );
    assert_eq!(first["flags"], "pass-through-method");
}

#[test]
fn every_detector_flags_a_red_flag_or_principle_that_exists() {
    let table: serde_json::Value = serde_json::from_str(rules::RULES_JSON).expect("valid json");
    let known: BTreeSet<&str> = ["red_flags", "principles"]
        .iter()
        .flat_map(|k| {
            table[k]
                .as_array()
                .expect("array")
                .iter()
                .map(|x| x["id"].as_str().expect("id"))
        })
        .collect();
    for detector in detectors() {
        assert!(
            known.contains(detector.flags.as_str()),
            "{} flags unknown {}",
            detector.id,
            detector.flags
        );
    }
    assert_eq!(
        table["red_flags"].as_array().expect("array").len(),
        14,
        "the rule table describes 14 red flags"
    );
}
