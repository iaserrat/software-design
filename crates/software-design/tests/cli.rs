//! The binary's three front ends, exercised as a user and Claude Code run them.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_software-design");

const PASS_THROUGH: &str = "export class Doc {\n  insert(text: string, at: number): void { this.area.insert(text, at); }\n}\n";

/// A committed repository with one clean file, plus an uncommitted file with a
/// pass-through method.
fn repo(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("software-design-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create repo");
    std::fs::write(dir.join("src/clean.ts"), "/** Adds two numbers. */\nexport function add(a: number, b: number): number { return a + b; }\n")
        .expect("write");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    );
    std::fs::write(dir.join("src/doc.ts"), PASS_THROUGH).expect("write");
    dir
}

fn git(dir: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git")
            .success()
    );
}

fn run(dir: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env_remove("CLAUDE_PLUGIN_OPTION_HOOK_MODE")
        .env_remove("CLAUDE_PLUGIN_DATA")
        .env_remove("SOFTWARE_DESIGN_HOOK_MODE")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stdout)))
}

#[test]
fn check_reports_findings_as_json_and_strict_fails_on_them() {
    let dir = repo("check");
    let output = run(&dir, &["check", "src"], "", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["schema_version"], 2);
    let rules: Vec<&str> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| f["rule"].as_str().expect("rule"))
        .collect();
    assert_eq!(rules, ["pass-through-method"]);
    assert_eq!(report["findings"][0]["path"], "src/doc.ts");

    assert_eq!(
        run(&dir, &["check", "--strict", "src"], "", &[])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        run(&dir, &["check", "--strict", "src/clean.ts"], "", &[])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        run(&dir, &["check", "--tier", "C"], "", &[]).status.code(),
        Some(2)
    );
    assert_eq!(run(&dir, &["frobnicate"], "", &[]).status.code(), Some(2));
}

#[test]
fn interface_prints_signatures_and_comments_without_bodies() {
    let dir = repo("interface");
    let output = run(&dir, &["interface", "src"], "", &[]);
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(
        text.contains("export function add(a: number, b: number): number\n    Adds two numbers."),
        "{text}"
    );
    assert!(
        text.contains("insert(text: string, at: number): void\n    (no interface comment)"),
        "{text}"
    );
    assert!(
        !text.contains("return a + b"),
        "bodies must not appear: {text}"
    );
}

#[test]
fn post_edit_reports_each_new_red_flag_once_per_session() {
    let dir = repo("post-edit");
    let data = dir.join(".plugin-data");
    let data = data.to_str().expect("utf-8 path");
    let input = json!({ "session_id": "s1", "cwd": dir, "tool_input": { "file_path": dir.join("src/doc.ts") } }).to_string();
    let env = [("CLAUDE_PLUGIN_DATA", data)];
    let start = run(
        &dir,
        &["hook", "session-start"],
        &json!({ "session_id": "s1", "cwd": dir }).to_string(),
        &env,
    );
    assert_eq!(start.stdout, [] as [u8; 0]);

    let first = run(&dir, &["hook", "post-edit"], &input, &env);
    let context = stdout_json(&first)["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context")
        .to_owned();
    assert!(
        context.contains("src/doc.ts:2 pass-through-method:"),
        "{context}"
    );

    let second = run(&dir, &["hook", "post-edit"], &input, &env);
    assert!(
        second.stdout.is_empty(),
        "already reported: {}",
        String::from_utf8_lossy(&second.stdout)
    );
}

/// Codex edits through `apply_patch`, which names files inside the patch text.
#[test]
fn post_edit_reads_the_files_of_a_codex_patch() {
    let dir = repo("post-edit-patch");
    let patch = "*** Begin Patch\n*** Update File: src/doc.ts\n@@\n+x\n*** End Patch\n";
    let input = json!({ "session_id": "s4", "cwd": dir, "tool_name": "apply_patch", "tool_input": { "command": patch } }).to_string();
    let output = run(&dir, &["hook", "post-edit"], &input, &[]);
    let context = stdout_json(&output)["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context")
        .to_owned();
    assert!(
        context.contains("src/doc.ts:2 pass-through-method:"),
        "{context}"
    );
}

#[test]
fn stop_blocks_only_in_block_mode_and_never_twice_in_a_row() {
    let dir = repo("stop");
    let input = json!({ "session_id": "s2", "cwd": dir, "stop_hook_active": false }).to_string();
    let block = [("CLAUDE_PLUGIN_OPTION_HOOK_MODE", "block")];
    let blocked = stdout_json(&run(&dir, &["hook", "stop"], &input, &block));
    assert_eq!(blocked["decision"], "block");
    assert!(
        blocked["reason"]
            .as_str()
            .expect("reason")
            .contains("pass-through-method")
    );

    let active = json!({ "session_id": "s2", "cwd": dir, "stop_hook_active": true }).to_string();
    assert_eq!(
        run(&dir, &["hook", "stop"], &active, &block).stdout,
        [] as [u8; 0]
    );
    assert_eq!(
        run(
            &dir,
            &["hook", "stop"],
            &input,
            &[("CLAUDE_PLUGIN_OPTION_HOOK_MODE", "context")]
        )
        .stdout,
        [] as [u8; 0]
    );
    assert!(
        run(&dir, &["hook", "stop"], &input, &[]).stdout.is_empty(),
        "context is the default"
    );
    let codex = [("SOFTWARE_DESIGN_HOOK_MODE", "block")];
    assert_eq!(
        stdout_json(&run(&dir, &["hook", "stop"], &input, &codex))["decision"],
        "block",
        "hosts without plugin options set the mode in the environment"
    );
}

#[test]
fn hooks_are_silent_when_off_or_outside_a_repository() {
    let dir = repo("off");
    let input = json!({ "session_id": "s3", "cwd": dir, "tool_input": { "file_path": dir.join("src/doc.ts") } }).to_string();
    let off = run(
        &dir,
        &["hook", "post-edit"],
        &input,
        &[("CLAUDE_PLUGIN_OPTION_HOOK_MODE", "off")],
    );
    assert!(off.stdout.is_empty() && off.status.success());

    let outside = std::env::temp_dir();
    let input =
        json!({ "session_id": "s3", "cwd": outside, "tool_input": { "file_path": "/tmp/x.ts" } })
            .to_string();
    let output = run(&outside, &["hook", "post-edit"], &input, &[]);
    assert!(output.stdout.is_empty() && output.status.success());
    let garbage = run(&dir, &["hook", "post-edit"], "not json", &[]);
    assert!(
        garbage.stdout.is_empty() && garbage.status.success(),
        "a broken hook must not break the session"
    );
}

#[test]
fn mcp_server_lists_and_runs_both_tools() {
    let dir = repo("mcp");
    let mut child = Command::new(BIN)
        .arg("mcp")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut request = |message: Value| {
        writeln!(stdin, "{message}").expect("send");
        stdin.flush().expect("flush");
    };
    let mut response = || -> Value {
        serde_json::from_str(&lines.next().expect("a response").expect("read")).expect("json")
    };

    request(
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } }),
    );
    assert!(response()["result"]["capabilities"]["tools"].is_object());
    request(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));

    request(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let listed = response()["result"]["tools"]
        .as_array()
        .expect("tools")
        .clone();
    let tools: Vec<String> = listed
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(tools, ["check", "interface"]);
    for tool in listed {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
    }

    request(
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "check", "arguments": { "paths": ["src"] } } }),
    );
    let text = response()["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
        .to_owned();
    let report: Value = serde_json::from_str(&text).expect("report json");
    assert_eq!(report["findings"][0]["rule"], "pass-through-method");

    request(
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "check", "arguments": { "tier": "Z" } } }),
    );
    assert!(
        response()["error"].is_object(),
        "a bad tier is an invalid-params error"
    );

    drop(stdin);
    let _ = child.wait();
}

/// Codex starts plugin servers in the plugin root, so a call must name the repository.
#[test]
fn mcp_server_can_require_a_directory() {
    let dir = repo("mcp-dir");
    let mut child = Command::new(BIN)
        .args(["mcp", "--require-directory"])
        // A repository: without the flag, a call without `directory` would analyse it.
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut call = |id: u32, message: Value| -> Value {
        writeln!(stdin, "{message}").expect("send");
        stdin.flush().expect("flush");
        if id == 0 {
            return Value::Null;
        }
        serde_json::from_str(&lines.next().expect("a response").expect("read")).expect("json")
    };
    call(
        1,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } }),
    );
    call(
        0,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );

    let without = call(
        2,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "check", "arguments": { "paths": ["src"] } } }),
    );
    assert!(without["error"].is_object(), "no directory: {without}");
    let with = call(
        3,
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "check", "arguments": { "paths": ["src"], "directory": dir } } }),
    );
    let report: Value =
        serde_json::from_str(with["result"]["content"][0]["text"].as_str().expect("text"))
            .expect("report json");
    assert_eq!(report["findings"][0]["rule"], "pass-through-method");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn design_and_review_requests_point_at_their_skill_and_nothing_else_does() {
    let dir = std::env::temp_dir();
    let ask = |prompt: &str, env: &[(&str, &str)]| {
        let input = json!({ "session_id": "p", "cwd": dir, "prompt": prompt }).to_string();
        run(&dir, &["hook", "user-prompt"], &input, env).stdout
    };
    let skill_for = |prompt: &str| -> Option<String> {
        let output: Value = serde_json::from_slice(&ask(prompt, &[])).ok()?;
        let context = output["hookSpecificOutput"]["additionalContext"]
            .as_str()?
            .to_owned();
        ["software-design:review", "software-design:design"]
            .into_iter()
            .find(|s| context.contains(s))
            .map(str::to_owned)
    };
    let expected = [
        (
            "Design the class interface in TypeScript for a text buffer.",
            Some("software-design:design"),
        ),
        (
            "How should I structure the billing module?",
            Some("software-design:design"),
        ),
        (
            "What methods should the Cache class have?",
            Some("software-design:design"),
        ),
        (
            "Review the code in src/ for design problems before I merge it.",
            Some("software-design:review"),
        ),
        (
            "Could a new teammate use the public API in src/buffer.ts without reading its implementation?",
            Some("software-design:review"),
        ),
        (
            "Review the design of the billing module",
            Some("software-design:review"),
        ),
        (
            "what should the interface look like for a TS client that wraps our REST endpoints?",
            Some("software-design:design"),
        ),
        (
            "planning a new python module that parses HL7 messages: what classes do I need?",
            Some("software-design:design"),
        ),
        (
            "I'm about to split UserService.php into smaller pieces. Which parts should become their own module?",
            Some("software-design:design"),
        ),
        (
            "help me design the composable for the cart drawer",
            Some("software-design:design"),
        ),
        (
            "can you look over my branch before I open the PR?",
            Some("software-design:review"),
        ),
        (
            "is helpers/format.py overcomplicated? it has 30 tiny functions",
            Some("software-design:review"),
        ),
        ("Design a landing page with a blue hero", None),
        (
            "Write unit tests for the existing RateLimiter struct in crates/limiter",
            None,
        ),
        (
            "check the auth controller for security vulnerabilities",
            None,
        ),
        (
            "add a limit parameter to the existing listOrders function in api/orders.ts",
            None,
        ),
        ("explain the strategy design pattern", None),
        ("Review my essay about the ocean", None),
        ("Fix the failing test in the billing module", None),
        ("/software-design:design a cache class", None),
        ("What is the capital of France?", None),
    ];
    for (prompt, skill) in expected {
        assert_eq!(skill_for(prompt).as_deref(), skill, "{prompt}");
    }
    let off = ask(
        "Design the class interface",
        &[("CLAUDE_PLUGIN_OPTION_HOOK_MODE", "off")],
    );
    assert_eq!(off, Vec::<u8>::new(), "hook_mode=off silences the nudge");
}
