//! Exercise the curl entry point without changing real agent installations.
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str, build_status: u8, tools: bool) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "software-design-installer-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("stubs")).expect("stubs");
        std::fs::create_dir_all(dir.join("home")).expect("home");
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for file in [
            "scripts/software-design",
            "crates/software-design/Cargo.toml",
        ] {
            let target = dir.join("template").join(file);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
            std::fs::copy(repo.join(file), target).expect("copy");
        }
        std::fs::copy(repo.join("install.sh"), dir.join("install.sh")).expect("installer");
        let fixture = Self { dir };
        fixture.stub("curl", "exit 1");
        fixture.stub("git", "exit 0");
        fixture.stub("cc", "exit 0");
        for agent in ["claude", "codex", "pi"] {
            fixture.stub(
                agent,
                &format!(
                    "echo '{agent}' \"$*\" >> \"$HOME/commands\"\n\
                     marker=\"$HOME/{agent}.installed\"\n\
                     case \"$*\" in\n\
                     'plugin list '*|list)\n\
                       if [ -f \"$marker\" ]; then\n\
                         case '{agent}' in\n\
                         claude) echo '[{{\"id\":\"software-design@software-design\",\"scope\":\"user\"}}]' ;;\n\
                         codex) echo '{{\"installed\":[{{\"pluginId\":\"software-design@software-design\"}}],\"available\":[]}}' ;;\n\
                         pi) printf 'User packages:\\n  git:github.com/iaserrat/software-design\\n' ;;\n\
                         esac\n\
                       else echo '[]'; fi\n\
                       exit ;;\n\
                     'plugin install '*|'plugin add '*|'install '*) touch \"$marker\" ;;\n\
                     'plugin update '*|'update '*)\n\
                       [ -f \"$marker\" ] || exit 1\n\
                       [ ! -f \"$HOME/fail-update\" ] || exit 1 ;;\n\
                     esac\n\
                     case '{agent}' in\n\
                     claude) target=\"$CLAUDE_CONFIG_DIR/plugins/marketplaces/software-design\" ;;\n\
                     codex) target=\"$CODEX_HOME/.tmp/marketplaces/software-design\" ;;\n\
                     pi) target=\"$PI_CODING_AGENT_DIR/git/github.com/iaserrat/software-design\" ;;\n\
                     esac\n\
                     if [ ! -d \"$target\" ]; then mkdir -p \"$target\"; cp -R \"$HOME/../template/.\" \"$target\"; fi"
                ),
            );
        }
        let version = env!("CARGO_PKG_VERSION");
        let response = if tools {
            r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"check"},{"name":"interface"}]}}"#
        } else {
            r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}"#
        };
        fixture.stub(
            "cargo",
            &format!(
                "echo build >> \"$HOME/builds\"\nexit_status={build_status}\n\
                 [ \"$exit_status\" = 0 ] || exit \"$exit_status\"\n\
                 while [ \"$1\" != --root ]; do shift; done\n\
                 mkdir -p \"$2/bin\"\ncat > \"$2/bin/software-design\" <<'BIN'\n\
                 #!/bin/sh\n\
                 if [ \"$1\" = --version ]; then echo 'software-design {version}'; exit; fi\n\
                 cat >/dev/null\necho '{response}'\n\
                 BIN\nchmod +x \"$2/bin/software-design\""
            ),
        );
        fixture
    }

    fn stub(&self, name: &str, body: &str) {
        let path = self.dir.join("stubs").join(name);
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).expect("stub");
        assert!(
            Command::new("chmod")
                .arg("+x")
                .arg(path)
                .status()
                .expect("chmod")
                .success()
        );
    }

    fn run(&self, agents: &[&str]) -> Output {
        Command::new("sh")
            .arg(self.dir.join("install.sh"))
            .args(agents)
            .env_clear()
            .env("HOME", self.dir.join("home"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.dir.join("stubs").display()),
            )
            .env("CLAUDE_CONFIG_DIR", self.dir.join("custom claude"))
            .env("CODEX_HOME", self.dir.join("custom codex"))
            .env("PI_CODING_AGENT_DIR", self.dir.join("custom pi"))
            .output()
            .expect("installer")
    }
}

#[test]
fn selected_agents_share_one_build_and_reruns_refresh_without_rebuilding() {
    let fixture = Fixture::new("all", 0, true);
    for run in 0..2 {
        let result = fixture.run(&["claude", "codex", "pi"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("Installation complete"));
        let commands =
            std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
        for install in [
            "claude plugin install software-design@software-design --scope user",
            "codex plugin add software-design@software-design",
            "pi install git:github.com/iaserrat/software-design",
        ] {
            assert_eq!(
                commands.lines().filter(|line| *line == install).count(),
                1,
                "{install}"
            );
        }
        for update in [
            "claude plugin update software-design@software-design",
            "codex plugin marketplace upgrade software-design",
            "pi update git:github.com/iaserrat/software-design",
        ] {
            assert_eq!(
                commands.lines().filter(|line| *line == update).count(),
                run,
                "{update}"
            );
        }
    }
    assert_eq!(
        std::fs::read_to_string(fixture.dir.join("home/builds")).expect("builds"),
        "build\n"
    );
    let commands = std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
    assert!(commands.contains("claude plugin marketplace update software-design"));
    assert!(commands.contains("codex plugin marketplace upgrade software-design"));
}

#[test]
fn a_cached_pi_checkout_without_registration_is_installed() {
    let fixture = Fixture::new("cached", 0, true);
    let target = fixture
        .dir
        .join("custom pi/git/github.com/iaserrat/software-design");
    std::fs::create_dir_all(target.parent().expect("parent")).expect("parent directories");
    assert!(
        Command::new("cp")
            .arg("-R")
            .arg(fixture.dir.join("template"))
            .arg(target)
            .status()
            .expect("copy checkout")
            .success()
    );
    let result = fixture.run(&["pi"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let commands = std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
    assert!(commands.contains("pi install git:github.com/iaserrat/software-design"));
    assert!(!commands.contains("pi update"));
}

#[test]
fn a_failed_update_is_not_retried_as_a_fresh_install() {
    let fixture = Fixture::new("update-failed", 0, true);
    assert!(fixture.run(&["pi"]).status.success());
    std::fs::write(fixture.dir.join("home/commands"), "").expect("clear log");
    std::fs::write(fixture.dir.join("home/fail-update"), "").expect("fail update");
    let result = fixture.run(&["pi"]);
    assert!(!result.status.success());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("Installation complete"));
    let commands = std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
    assert!(commands.contains("pi update git:github.com/iaserrat/software-design"));
    assert!(!commands.contains("pi install"));
}

#[test]
fn failed_build_does_not_register_the_plugin_or_report_success() {
    let fixture = Fixture::new("failed", 101, true);
    let result = fixture.run(&["codex"]);
    assert!(!result.status.success());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("Installation complete"));
    let commands = std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
    assert!(!commands.contains("codex plugin add"));
    assert!(String::from_utf8_lossy(&result.stderr).contains("Build failed"));
}

#[test]
fn missing_mcp_tools_fail_before_registration() {
    let fixture = Fixture::new("mcp", 0, false);
    let result = fixture.run(&["claude"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("MCP readiness check failed"));
    let commands = std::fs::read_to_string(fixture.dir.join("home/commands")).expect("commands");
    assert!(!commands.contains("claude plugin install"));
}

#[test]
fn invalid_selection_and_truncated_download_make_no_changes() {
    let fixture = Fixture::new("invalid", 0, true);
    assert!(!fixture.run(&["codex", "unknown"]).status.success());
    assert!(!fixture.dir.join("home/commands").exists());
    let script = std::fs::read_to_string(fixture.dir.join("install.sh")).expect("script");
    let truncated = script.strip_suffix("}\n").expect("guard");
    std::fs::write(fixture.dir.join("install.sh"), truncated).expect("truncate");
    assert!(!fixture.run(&["codex"]).status.success());
    assert!(!fixture.dir.join("home/commands").exists());
}
