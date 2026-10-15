//! The plugin launcher (`scripts/software-design`) builds the plugin's source into
//! `~/.software-design`, shares builds across agent directories, and never
//! breaks a hook. A stub `cargo` stands in for the real build, and a stub `curl`
//! for main's manifest, which names version 99.0.0.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A plugin directory with the launcher and the crate manifest it reads the
/// version from, a home directory, and a directory holding a stub `cargo`.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    /// `cargo` is `Some(exit status)` to put a stub `cargo` on `PATH`, `None` for none.
    fn new(name: &str, cargo: Option<u8>) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "software-design-launcher-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (from, to) in [
            ("scripts/software-design", "plugin/scripts/software-design"),
            (
                "crates/software-design/Cargo.toml",
                "plugin/crates/software-design/Cargo.toml",
            ),
        ] {
            let target = dir.join(to);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
            std::fs::copy(repo.join(from), target).expect("copy");
        }
        std::fs::create_dir_all(dir.join("home")).expect("home");
        std::fs::create_dir_all(dir.join("stubs")).expect("stubs");
        let curl = dir.join("stubs/curl");
        std::fs::write(&curl, "#!/bin/sh\necho 'version = \"99.0.0\"'\n").expect("curl");
        Command::new("chmod")
            .arg("+x")
            .arg(&curl)
            .status()
            .expect("chmod");
        if let Some(status) = cargo {
            // Logs where and how it ran, then writes a binary that reports VERSION.
            let stub = format!(
                "#!/bin/sh\nprintf '%s|%s\\n' \"$PWD\" \"$*\" >> '{log}'\n[ {status} -eq 0 ] || exit {status}\n\
                 while [ \"$1\" != --root ]; do shift; done\nmkdir -p \"$2/bin\"\n\
                 printf '#!/bin/sh\\n[ \"$1\" = --version ] && echo \"software-design {VERSION}\" && exit 0\\necho \"ran $*\"\\n' > \"$2/bin/software-design\"\n\
                 chmod +x \"$2/bin/software-design\"\n",
                log = dir.join("cargo.log").display()
            );
            let path = dir.join("stubs/cargo");
            std::fs::write(&path, stub).expect("stub");
            Command::new("chmod")
                .arg("+x")
                .arg(&path)
                .status()
                .expect("chmod");
        }
        Fixture { dir }
    }

    fn launch(&self, args: &[&str]) -> Output {
        Command::new("sh")
            .arg(self.dir.join("plugin/scripts/software-design"))
            .args(args)
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.dir.join("stubs").display()),
            )
            .env("HOME", self.dir.join("home"))
            .output()
            .expect("run launcher")
    }

    fn cargo_runs(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("cargo.log"))
            .map(|log| log.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    fn installed(&self) -> PathBuf {
        self.dir.join("home/.software-design/bin/software-design")
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn install_builds_the_plugin_source_into_the_home_directory_and_runs_it() {
    let fixture = Fixture::new("install", Some(0));
    let install = fixture.launch(&["install"]);
    assert!(install.status.success(), "{}", text(&install.stderr));
    let plugin = fixture.dir.join("plugin");
    let home = fixture.dir.join("home/.software-design");
    assert_eq!(
        fixture.cargo_runs(),
        [format!(
            "{}|install --locked --force --path crates/software-design --root {}",
            plugin.display(),
            home.display()
        )]
    );
    assert!(fixture.installed().is_file());
    assert_eq!(
        text(&fixture.launch(&["--version"]).stdout).trim(),
        format!("software-design {VERSION}")
    );
    assert_eq!(
        text(&fixture.launch(&["check", "src"]).stdout).trim(),
        "ran check src"
    );
}

#[test]
fn install_does_nothing_when_the_installed_version_matches() {
    let fixture = Fixture::new("current", Some(0));
    assert!(fixture.launch(&["install"]).status.success());
    assert!(fixture.launch(&["install"]).status.success());
    assert_eq!(fixture.cargo_runs().len(), 1);
}

#[test]
fn a_build_of_another_version_is_refused_and_rebuilt() {
    let fixture = Fixture::new("stale", Some(0));
    std::fs::create_dir_all(fixture.installed().parent().expect("bin dir")).expect("mkdir");
    std::fs::write(
        fixture.installed(),
        "#!/bin/sh\necho software-design 0.0.1\n",
    )
    .expect("write");
    Command::new("chmod")
        .arg("+x")
        .arg(fixture.installed())
        .status()
        .expect("chmod");

    let refused = fixture.launch(&["check"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(text(&refused.stderr).contains(&format!("version {VERSION} is not built")));
    assert!(fixture.launch(&["install"]).status.success());
    assert_eq!(fixture.cargo_runs().len(), 1);
    assert_eq!(text(&fixture.launch(&["check"]).stdout).trim(), "ran check");
}

#[test]
fn a_build_from_another_plugin_directory_is_shared_at_the_same_version() {
    let fixture = Fixture::new("moved", Some(0));
    assert!(fixture.launch(&["install"]).status.success());
    // An old install's source stamp must not make the shared binary stale.
    std::fs::write(
        fixture.dir.join("home/.software-design/source"),
        "/old/plugin\n",
    )
    .expect("old stamp");
    let other = Fixture::new("other-agent", Some(0));
    let result = Command::new("sh")
        .arg(other.dir.join("plugin/scripts/software-design"))
        .arg("install")
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", other.dir.join("stubs").display()),
        )
        .env("HOME", other.dir.join("home"))
        .env(
            "SOFTWARE_DESIGN_HOME",
            fixture.dir.join("home/.software-design"),
        )
        .output()
        .expect("other agent");
    assert!(result.status.success(), "{}", text(&result.stderr));
    assert_eq!(fixture.cargo_runs().len(), 1);
    assert_eq!(other.cargo_runs().len(), 0);
    assert_eq!(text(&fixture.launch(&["check"]).stdout).trim(), "ran check");
}

#[test]
fn without_rust_hooks_explain_once_and_never_fail() {
    let fixture = Fixture::new("no-rust", None);
    let start = fixture.launch(&["hook", "session-start"]);
    assert!(start.status.success());
    assert!(
        text(&start.stdout).contains("Run the installer"),
        "{}",
        text(&start.stdout)
    );
    let edit = fixture.launch(&["hook", "post-edit"]);
    assert!(edit.status.success() && edit.stdout.is_empty());
    let install = fixture.launch(&["install"]);
    assert_eq!(install.status.code(), Some(1));
    assert!(text(&install.stderr).contains("https://rustup.rs"));
}

#[test]
fn a_failed_build_points_at_its_log_and_releases_the_lock() {
    let fixture = Fixture::new("failed", Some(101));
    let install = fixture.launch(&["install"]);
    assert_eq!(install.status.code(), Some(1));
    assert!(text(&install.stderr).contains("build.log"));
    assert!(
        !fixture
            .dir
            .join("home/.software-design/build.lock")
            .exists()
    );
    let start = fixture.launch(&["hook", "session-start"]);
    assert!(
        text(&start.stdout).contains("Run the installer"),
        "a later session explains how to retry"
    );
}

/// A host that kills the hook's process group leaves the lock behind with a dead pid.
#[test]
fn a_lock_left_by_a_killed_build_does_not_block_the_next_one() {
    let fixture = Fixture::new("killed", Some(0));
    let mut gone = Command::new("true").spawn().expect("spawn");
    let pid = gone.id();
    gone.wait().expect("wait");
    let lock = fixture.dir.join("home/.software-design/build.lock");
    std::fs::create_dir_all(&lock).expect("lock");
    std::fs::write(lock.join("pid"), format!("{pid}\n")).expect("pid");

    let install = fixture.launch(&["install"]);
    assert!(install.status.success(), "{}", text(&install.stderr));
    assert_eq!(fixture.cargo_runs().len(), 1);
    assert!(!lock.exists());
}

#[test]
fn an_active_build_cannot_be_reported_as_a_completed_install() {
    let fixture = Fixture::new("busy", Some(0));
    let lock = fixture.dir.join("home/.software-design/build.lock");
    std::fs::create_dir_all(&lock).expect("lock");
    std::fs::write(lock.join("pid"), format!("{}\n", std::process::id())).expect("pid");
    let install = fixture.launch(&["install"]);
    assert!(!install.status.success());
    assert!(text(&install.stderr).contains("another build is running"));
    assert_eq!(fixture.cargo_runs().len(), 0);
    assert!(lock.exists(), "leave the other process's lock alone");
}

#[test]
fn session_start_says_when_main_has_a_newer_version() {
    let fixture = Fixture::new("update", Some(0));
    assert!(fixture.launch(&["install"]).status.success());
    let start = text(&fixture.launch(&["hook", "session-start"]).stdout);
    assert!(
        start.contains(&format!("99.0.0 is out (you have {VERSION})")),
        "{start}"
    );
    std::fs::write(fixture.dir.join("home/.software-design/latest"), VERSION).expect("latest");
    let start = text(&fixture.launch(&["hook", "session-start"]).stdout);
    assert!(!start.contains("is out"), "{start}");
}
