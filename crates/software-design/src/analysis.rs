//! The two questions every front end asks: "what red flags are here?" and "what
//! is the interface of these files?". The CLI, the hooks and the MCP server all
//! answer through this module, so they always agree.

use std::fmt::Write as _;
use std::path::Path;

use history::Repo;
use rules::{AddedLines, Codebase, File, Finding, Options, Tier};
use serde_json::Value;

/// Files larger than this are generated or vendored, not designed.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// How far back churn and co-change look.
pub(crate) const DEFAULT_SINCE: &str = "12 months ago";

/// What to check.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// Report only findings under these repository-relative paths; empty means all.
    pub paths: Vec<String>,
    pub lines: Lines,
    pub tier: Tier,
    pub since: String,
}

/// Which lines findings may be on.
#[derive(Debug, Clone)]
pub(crate) enum Lines {
    All,
    /// Lines added since a commit, including uncommitted work. `None` is `HEAD`.
    AddedSince(Option<String>),
}

/// Runs the detectors and returns the findings.
///
/// Tier A rules only need the files being reported on. Tier B rules count
/// callers and copies across the whole repository and read git history, so
/// they parse every file.
pub(crate) fn check(repo: &Repo, request: &Request) -> Result<Vec<Finding>, String> {
    let whole_repo = request.tier == Tier::B;
    let added: Option<AddedLines> = match &request.lines {
        Lines::AddedSince(base) => {
            let paths: Vec<&str> = request.paths.iter().map(String::as_str).collect();
            Some(
                repo.added_lines(base.as_deref(), &paths)
                    .map_err(|e| e.to_string())?,
            )
        }
        Lines::All => None,
    };
    let selected = |path: &str| {
        let requested = request.paths.is_empty() || request.paths.iter().any(|p| under(path, p));
        requested && added.as_ref().is_none_or(|a| a.contains_key(path))
    };
    let files = load(repo, |path| whole_repo || selected(path))?;
    let history = if whole_repo {
        Some(repo.history(&request.since).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let codebase = Codebase { files, history };
    let mut findings = rules::check(
        &codebase,
        &Options {
            tier: request.tier,
            added: added.as_ref(),
        },
    );
    findings.retain(|f| selected(&f.path));
    Ok(findings)
}

/// The public interface of the files under `paths`: each public type and function
/// with its signature and interface comment, and no bodies. This is everything a
/// caller is supposed to need.
pub(crate) fn interface(repo: &Repo, paths: &[String]) -> Result<String, String> {
    let files = load(repo, |path| {
        paths.is_empty() || paths.iter().any(|p| under(path, p))
    })?;
    let mut out = String::new();
    for file in &files {
        let facts = &file.facts;
        let mut items: Vec<(u32, &str, Option<usize>)> = facts
            .types
            .iter()
            .filter(|t| t.public)
            .map(|t| (t.span.start, t.header.as_str(), t.doc))
            .chain(
                facts
                    .functions
                    .iter()
                    .filter(|f| f.public)
                    .map(|f| (f.span.start, f.header.as_str(), f.doc)),
            )
            .collect();
        if items.is_empty() {
            continue;
        }
        items.sort_by_key(|(line, ..)| *line);
        let _ = writeln!(out, "## {}", file.path);
        for (_, header, doc) in items {
            let _ = writeln!(out, "\n{header}");
            match doc {
                Some(i) => {
                    for line in facts.comments[i].text.lines() {
                        let _ = writeln!(out, "    {line}");
                    }
                }
                None => {
                    let _ = writeln!(out, "    (no interface comment)");
                }
            }
        }
        out.push('\n');
    }
    if out.is_empty() {
        return Err("no public declarations found under the given paths".to_owned());
    }
    Ok(out)
}

/// The report as JSON, with the repository root so paths can be resolved.
pub(crate) fn report(repo: &Repo, findings: &[Finding]) -> Value {
    let mut report = rules::report(findings);
    report["root"] = Value::String(repo.root().display().to_string());
    report
}

/// Repository-relative form of a path given on the command line or by a hook:
/// absolute paths and paths relative to `cwd` both resolve.
pub(crate) fn relative(repo: &Repo, cwd: &Path, path: &str) -> String {
    let absolute = cwd.join(path);
    let absolute = absolute.canonicalize().unwrap_or(absolute);
    let root = repo
        .root()
        .canonicalize()
        .unwrap_or_else(|_| repo.root().to_path_buf());
    absolute.strip_prefix(&root).map_or_else(
        |_| path.to_owned(),
        |p| p.to_string_lossy().replace('\\', "/"),
    )
}

/// True when `path` is `prefix` or inside the directory `prefix`.
fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    prefix.is_empty()
        || prefix == "."
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn load(repo: &Repo, wanted: impl Fn(&str) -> bool) -> Result<Vec<File>, String> {
    let mut files = Vec::new();
    for path in repo.files().map_err(|e| e.to_string())? {
        if syntax::Lang::for_path(&path).is_none() || !wanted(&path) || vendored(&path) {
            continue;
        }
        let full = repo.root().join(&path);
        if std::fs::metadata(&full).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        // Unreadable or non-UTF-8 files are not source code we can judge.
        let Ok(source) = std::fs::read_to_string(&full) else {
            continue;
        };
        if generated(&path, &source) {
            continue;
        }
        if let Some(facts) = syntax::parse(&path, &source) {
            files.push(File { path, facts });
        }
    }
    Ok(files)
}

/// Code written by a generator: nobody designs it, so nobody can act on findings.
fn generated(path: &str, source: &str) -> bool {
    const MARKERS: [&str; 5] = [
        "@generated",
        "auto-generated",
        "autogenerated",
        "Code generated",
        "DO NOT EDIT",
    ];
    path.contains(".gen.")
        || path.contains(".generated.")
        || path.contains("_pb2")
        || source
            .lines()
            .take(5)
            .any(|line| MARKERS.iter().any(|m| line.contains(m)))
}

/// Third-party code, build output and test inputs: nobody designs these.
fn vendored(path: &str) -> bool {
    path.split('/').any(|dir| {
        matches!(
            dir,
            "vendor"
                | "node_modules"
                | "third_party"
                | "dist"
                | "build"
                | "fixtures"
                | "testdata"
                | "__fixtures__"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::under;

    #[test]
    fn path_filters_match_directories_not_name_prefixes() {
        assert!(
            under("src/a.ts", "src") && under("src/a.ts", "src/") && under("src/a.ts", "src/a.ts")
        );
        assert!(!under("srcx/a.ts", "src"));
        assert!(under("anything.ts", "."));
    }
}
