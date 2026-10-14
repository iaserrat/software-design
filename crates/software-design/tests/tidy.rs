//! Repository-wide consistency checks that no compiler or linter makes.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Frontmatter keys and values of a Markdown file, one `key: value` per line.
fn frontmatter(text: &str) -> Vec<(String, String)> {
    let body = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---"))
        .map_or("", |(f, _)| f);
    body.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

fn markdown_files(dir: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root().join(dir)];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The launcher reads the version from this manifest to decide when to rebuild.
#[test]
fn every_version_names_the_same_release() {
    let cargo = read("crates/software-design/Cargo.toml");
    let crate_version = cargo
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.strip_suffix('"'))
        .expect("crate version");
    for manifest in [
        ".claude-plugin/plugin.json",
        ".codex-plugin/plugin.json",
        "package.json",
    ] {
        assert_eq!(
            json(manifest)["version"],
            crate_version,
            "{manifest} and Cargo.toml"
        );
    }
}

#[test]
fn skills_are_named_after_their_directory_and_invocable_by_model_and_user() {
    let skills = markdown_files("skills");
    assert_ne!(skills, [] as [std::path::PathBuf; 0]);
    for path in skills {
        let dir = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .expect("dir");
        let text = std::fs::read_to_string(&path).expect("read");
        let fields = frontmatter(&text);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("name"), Some(dir), "{}", path.display());
        let description =
            get("description").unwrap_or_else(|| panic!("{} has no description", path.display()));
        assert!(
            (100..=1024).contains(&description.len()),
            "{}: description length {}",
            path.display(),
            description.len()
        );
        assert_eq!(
            get("disable-model-invocation"),
            None,
            "{}: skills must be model-invocable",
            path.display()
        );
        assert_eq!(
            get("user-invocable"),
            None,
            "{}: skills must be user-invocable",
            path.display()
        );
    }
}

#[test]
fn agents_are_named_after_their_file_and_declare_tools() {
    for path in markdown_files("agents") {
        let stem = path.file_stem().and_then(|n| n.to_str()).expect("stem");
        let text = std::fs::read_to_string(&path).expect("read");
        let fields = frontmatter(&text);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("name"), Some(stem), "{}", path.display());
        assert!(
            get("description").is_some_and(|d| d.len() >= 100),
            "{}",
            path.display()
        );
        assert!(
            get("tools").is_some(),
            "{}: agents declare their tools",
            path.display()
        );
    }
}

/// Every plugin-qualified name that skills, agents and the workflow mention
/// must exist: `software-design:<skill or agent>` and the MCP tool names.
#[test]
fn cross_references_resolve() {
    let plugin = json(".claude-plugin/plugin.json")["name"]
        .as_str()
        .expect("name")
        .to_owned();
    let servers = json(".claude-plugin/plugin.json")["mcpServers"]
        .as_object()
        .expect("servers")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut known: BTreeSet<String> = BTreeSet::new();
    for path in markdown_files("skills") {
        known.insert(format!(
            "{plugin}:{}",
            path.parent()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .expect("dir")
        ));
    }
    for path in markdown_files("agents") {
        known.insert(format!(
            "{plugin}:{}",
            path.file_stem().and_then(|n| n.to_str()).expect("stem")
        ));
    }
    // Workflows register under the `name` in their `meta` block.
    for entry in std::fs::read_dir(root().join("workflows")).expect("workflows") {
        let text = std::fs::read_to_string(entry.expect("entry").path()).expect("read");
        let name = text
            .split_once("name: '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(name, _)| name)
            .expect("workflow meta name");
        known.insert(format!("{plugin}:{name}"));
    }
    for server in &servers {
        for tool in ["check", "interface"] {
            known.insert(format!("mcp__plugin_{plugin}_{server}__{tool}"));
        }
    }
    let mut sources = markdown_files("skills");
    sources.extend(markdown_files("agents"));
    sources.push(root().join("workflows/audit.js"));
    for path in sources {
        let text = std::fs::read_to_string(&path).expect("read");
        let words = text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | ':')));
        for word in
            words.filter(|w| w.starts_with(&format!("{plugin}:")) || w.starts_with("mcp__plugin_"))
        {
            assert!(
                known.contains(word),
                "{} mentions unknown `{word}`",
                path.display()
            );
        }
    }
}

#[test]
fn hooks_and_mcp_run_the_launcher() {
    let launcher = "${CLAUDE_PLUGIN_ROOT}/scripts/software-design";
    let hooks = json("hooks/hooks.json");
    for (event, matchers) in hooks["hooks"].as_object().expect("events") {
        for matcher in matchers.as_array().expect("matchers") {
            for hook in matcher["hooks"].as_array().expect("hooks") {
                // A command string, not `args`: Codex runs this file too and has no `args`.
                let command = hook["command"].as_str().unwrap_or_default();
                let rest = command
                    .strip_prefix(&format!("\"{launcher}\" "))
                    .unwrap_or_else(|| panic!("{event}: {command}"));
                assert!(
                    rest.starts_with("hook ") || rest == "install --detach",
                    "{event}: {rest}"
                );
            }
        }
    }
    assert_eq!(
        json(".claude-plugin/plugin.json")["mcpServers"]["software-design"]["command"],
        launcher
    );
    // Codex expands no variables in MCP commands; it runs them from the plugin root.
    let codex = &json(".codex-plugin/plugin.json")["mcpServers"]["software-design"];
    assert_eq!(codex["args"][0], "scripts/software-design");
    assert_eq!(codex["cwd"], ".");
    let mode = std::fs::metadata(root().join("scripts/software-design")).expect("launcher exists");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(
            mode.permissions().mode() & 0o111 != 0,
            "launcher must be executable"
        );
    }
}

/// Deliberate shortcuts are marked so they can be found; none may ship.
#[test]
fn no_shortcut_markers_or_debug_leftovers_ship() {
    let banned = ["remove_me_hackyhack", "dbg!(", "todo!(", "unimplemented!("];
    let mut stack = vec![
        root().join("crates"),
        root().join("scripts"),
        root().join("skills"),
        root().join("agents"),
        root().join("workflows"),
    ];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.ends_with("tidy.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for marker in banned {
                assert!(
                    !text.contains(marker),
                    "{} contains {marker}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn license_names_the_author() {
    let license = read("LICENSE");
    assert!(license.starts_with("MIT License"));
    assert!(license.contains("Ismael Serratos"));
    assert_eq!(json(".claude-plugin/plugin.json")["license"], "MIT");
}

/// The workspace layout from matklad's "Large Rust Workspaces" (2021):
/// a virtual root manifest, every crate one level under `crates/` with the
/// directory named after the crate, internal crates at version 0.0.0 and
/// unpublished, and a `src/` directory in every crate, however small.
#[test]
fn workspace_follows_the_flat_layout() {
    let manifest = read("Cargo.toml");
    assert!(
        !manifest.contains("[package]"),
        "the root manifest is virtual"
    );
    assert!(
        manifest.contains("members = [\"crates/*\"]"),
        "members are the crates/* glob"
    );
    let binary = "software-design";
    for entry in std::fs::read_dir(root().join("crates")).expect("crates dir") {
        let dir = entry.expect("entry").path();
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8")
            .to_owned();
        let crate_manifest =
            std::fs::read_to_string(dir.join("Cargo.toml")).expect("crate manifest");
        let field = |key: &str| {
            crate_manifest
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{key} = \"")))
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(
            field("name"),
            dir_name,
            "crate directory and crate name match"
        );
        assert!(
            crate_manifest.contains("publish = false"),
            "{dir_name}: not published to crates.io"
        );
        if dir_name != binary {
            assert_eq!(
                field("version"),
                "0.0.0",
                "{dir_name}: internal crates are 0.0.0"
            );
        }
        assert!(
            dir.join("src/lib.rs").is_file() || dir.join("src/main.rs").is_file(),
            "{dir_name}: keeps src/"
        );
        let nested = std::fs::read_dir(&dir)
            .expect("read crate")
            .flatten()
            .any(|e| e.path().join("Cargo.toml").is_file());
        assert!(!nested, "{dir_name}: crates are not nested");
    }
}

/// The checkable items of Anthropic's "Skill authoring best practices" checklist
/// (platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices).
#[test]
fn skills_follow_the_authoring_guide() {
    let reserved = ["anthropic", "claude"];
    // The open Agent Skills specification (agentskills.io) allows these fields only;
    // Claude Code accepts all of them, so the skills load in every client.
    let spec_fields = [
        "name",
        "description",
        "license",
        "compatibility",
        "metadata",
        "allowed-tools",
    ];
    for path in markdown_files("skills") {
        let text = std::fs::read_to_string(&path).expect("read");
        let fields = frontmatter(&text);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let shown = path.display();
        for (key, _) in &fields {
            assert!(
                spec_fields.contains(&key.as_str()),
                "{shown}: `{key}` is not an Agent Skills field"
            );
        }
        let name = get("name");
        assert!(
            name.len() <= 64
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{shown}: name"
        );
        assert!(
            reserved.iter().all(|r| !name.contains(r)),
            "{shown}: reserved word in name"
        );
        let description = get("description");
        assert!(
            description.len() <= 1024 && !description.contains('<'),
            "{shown}: description length or XML"
        );
        assert!(
            description.contains("Use when") || description.contains("Use whenever"),
            "{shown}: says when to use it"
        );
        let lowered = description.to_lowercase();
        assert!(
            !lowered.starts_with("i ") && !lowered.contains("you can"),
            "{shown}: third person"
        );
        let body = text.splitn(3, "---").nth(2).expect("body");
        assert!(body.lines().count() < 500, "{shown}: body under 500 lines");
        assert!(!body.contains('\\'), "{shown}: forward-slash paths only");
        // MCP tools are called "the `check` tool"; a bare `check` reads as a verb.
        assert!(
            !body.contains("Call `check`") && !body.contains("call `check`"),
            "{shown}: qualified tool names"
        );
        // References stay one level deep: the only file a skill points at is rules.json.
        for word in body.split(|c: char| c.is_whitespace() || matches!(c, '`' | '(' | ')')) {
            let extension = Path::new(word)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            let other_file = matches!(extension.as_deref(), Some("md" | "json"))
                && !word.ends_with("rules.json");
            assert!(!other_file, "{shown}: references {word}");
        }
        for year in 2015..2100 {
            assert!(
                !body.contains(&format!("before {year}"))
                    && !body.contains(&format!("after {year}")),
                "{shown}: time-sensitive"
            );
        }
    }
}
