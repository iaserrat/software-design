//! Claude Code hook handlers. Each reads the hook's JSON from stdin and writes
//! the hook's JSON response to stdout.
//!
//! - `session-start` remembers `HEAD`, so later hooks know which lines this
//!   session wrote.
//! - `post-edit` reports tier A red flags on lines the session added to the edited
//!   file, each at most once per session: the cheapest moment to fix a red flag
//!   is right after writing it.
//! - `stop`, in `block` mode, refuses to end the turn while tier A red flags remain
//!   in the session's changes, so the diff is checked before it is committed.
//! - `user-prompt` points a request to design or review code at the skill that
//!   serves it, before Claude answers with the first design that comes to mind.
//!
//! A hook must never break the session: every failure ends in "nothing to say".

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use history::Repo;
use rules::{Finding, Tier};
use serde_json::{Value, json};

mod prompt;
mod session;

use prompt::nudge;
use session::{Session, State};

use crate::analysis::{self, DEFAULT_SINCE, Lines, Request};

/// The `hook_mode` plugin option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Off,
    /// Tell the model after each edit; never block.
    Context,
    /// Also block the end of a turn while red flags remain.
    Block,
}

/// Findings listed in one message; the rest are counted.
const MAX_LISTED: usize = 5;

/// Handles one hook `event` given its JSON `input`. Returns the JSON to print, or
/// `None` to print nothing. Errors are for the log only: callers must still exit 0.
pub(crate) fn run(event: &str, input: &str) -> Result<Option<Value>, String> {
    // Claude Code passes the plugin option; other hosts have no plugin options.
    let mode = match std::env::var("CLAUDE_PLUGIN_OPTION_HOOK_MODE")
        .or_else(|_| std::env::var("SOFTWARE_DESIGN_HOOK_MODE"))
        .as_deref()
    {
        Ok("off") => Mode::Off,
        Ok("block") => Mode::Block,
        _ => Mode::Context,
    };
    if mode == Mode::Off {
        return Ok(None);
    }
    let input: Value =
        serde_json::from_str(input).map_err(|e| format!("hook input is not JSON: {e}"))?;
    if event == "user-prompt" {
        return Ok(input["prompt"].as_str().and_then(nudge).map(|context| {
            json!({ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "additionalContext": context } })
        }));
    }
    let cwd = input["cwd"].as_str().map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let Ok(repo) = Repo::discover(&cwd) else {
        return Ok(None);
    };
    let state = Session::open(input["session_id"].as_str());
    match event {
        "session-start" => {
            Session::prune();
            if let (Some(state), Ok(Some(head))) = (&state, repo.head()) {
                state.save(&State {
                    base: Some(head),
                    reported: BTreeSet::new(),
                });
            }
            Ok(None)
        }
        "post-edit" => post_edit(&repo, &cwd, &input, state.as_ref()),
        "stop" if mode == Mode::Block && input["stop_hook_active"] != true => {
            stop(&repo, state.as_ref())
        }
        "stop" => Ok(None),
        other => Err(format!("unknown hook event `{other}`")),
    }
}

fn post_edit(
    repo: &Repo,
    cwd: &Path,
    input: &Value,
    state: Option<&Session>,
) -> Result<Option<Value>, String> {
    // Claude Code's Edit and Write name one `file_path`; Codex's apply_patch passes
    // the patch as `command`, with a `*** Update File: <path>` line per file.
    let tool_input = &input["tool_input"];
    let files: Vec<&str> = match tool_input["file_path"].as_str() {
        Some(file) => vec![file],
        None => tool_input["command"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                l.strip_prefix("*** Update File: ")
                    .or_else(|| l.strip_prefix("*** Add File: "))
                    .or_else(|| l.strip_prefix("*** Move to: "))
            })
            .collect(),
    };
    let paths: Vec<String> = files
        .into_iter()
        .map(|file| analysis::relative(repo, cwd, file.trim()))
        .filter(|path| syntax::Lang::for_path(path).is_some())
        .collect();
    if paths.is_empty() {
        return Ok(None);
    }
    let mut saved = state.map(Session::load).unwrap_or_default();
    let findings = session_findings(repo, &saved, paths)?;
    let fresh: Vec<&Finding> = findings
        .iter()
        .filter(|f| saved.reported.insert(key(f)))
        .collect();
    if fresh.is_empty() {
        return Ok(None);
    }
    if let Some(state) = state {
        state.save(&saved);
    }
    // software-design-allow: repeated-code this message and the stop message share a shape, not logic
    let context = format!(
        "software-design: the edit introduced design red flags. \
         Fix them now, or keep the code and add `software-design-allow: <rule> <reason>` in a comment above it.\n{}",
        list(&fresh)
    );
    Ok(Some(json!({
        "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context }
    })))
}

fn stop(repo: &Repo, state: Option<&Session>) -> Result<Option<Value>, String> {
    let saved = state.map(Session::load).unwrap_or_default();
    let findings = session_findings(repo, &saved, Vec::new())?;
    if findings.is_empty() {
        return Ok(None);
    }
    let all: Vec<&Finding> = findings.iter().collect();
    let reason = format!(
        "software-design (hook_mode=block): this session's changes still contain red flags. \
         Fix each one, or add `software-design-allow: <rule> <reason>` above it if the design is intended.\n{}",
        list(&all)
    );
    Ok(Some(json!({ "decision": "block", "reason": reason })))
}

/// Tier A findings on lines added since the session's base commit (or since
/// `HEAD` without session state), under `paths` or everywhere when empty.
fn session_findings(
    repo: &Repo,
    saved: &State,
    paths: Vec<String>,
) -> Result<Vec<Finding>, String> {
    let request = Request {
        paths,
        lines: Lines::AddedSince(saved.base.clone()),
        tier: Tier::A,
        since: DEFAULT_SINCE.to_owned(),
    };
    analysis::check(repo, &request)
}

/// Identifies a finding across edits, when its line number moves.
// software-design-allow: shallow-function the name states how findings are identified, a decision callers should not repeat
fn key(finding: &Finding) -> String {
    format!("{}:{}:{}", finding.rule, finding.path, finding.symbol)
}

fn list(findings: &[&Finding]) -> String {
    let mut lines: Vec<String> = findings
        .iter()
        .take(MAX_LISTED)
        .map(|f| format!("- {}:{} {}: {}", f.path, f.line, f.rule, f.message))
        .collect();
    if findings.len() > MAX_LISTED {
        lines.push(format!("- and {} more", findings.len() - MAX_LISTED));
    }
    lines.join("\n")
}
