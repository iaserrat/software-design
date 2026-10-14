//! Runs against real throwaway repositories.

use std::path::{Path, PathBuf};
use std::process::Command;

use history::Repo;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("history-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    run(&dir, &["init", "-q", "-b", "main"]);
    run(&dir, &["config", "user.email", "test@example.com"]);
    run(&dir, &["config", "user.name", "Test"]);
    dir
}

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn commit(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        std::fs::write(dir.join(path), text).expect("write");
    }
    run(dir, &["add", "-A"]);
    run(dir, &["commit", "-q", "-m", "change"]);
}

#[test]
fn counts_churn_and_files_that_change_together() {
    let dir = scratch("churn");
    commit(&dir, &[("a.ts", "1"), ("b.ts", "1")]);
    commit(&dir, &[("a.ts", "2"), ("b.ts", "2")]);
    commit(&dir, &[("a.ts", "3")]);
    let repo = Repo::discover(&dir).expect("repo");
    let history = repo.history("10 years ago").expect("log");
    assert_eq!(history.churn["a.ts"], 3);
    assert_eq!(history.together[&("a.ts".to_owned(), "b.ts".to_owned())], 2);
}

#[test]
fn added_lines_cover_edits_and_untracked_files_before_the_first_commit() {
    let dir = scratch("added");
    std::fs::write(dir.join("new.ts"), "a\nb\n").expect("write");
    let repo = Repo::discover(&dir).expect("repo");
    assert_eq!(repo.head().expect("head"), None);
    assert_eq!(
        repo.added_lines(None, &[]).expect("diff")["new.ts"],
        [1..=2]
    );

    commit(&dir, &[("new.ts", "a\nb\nc\n")]);
    let base = repo.head().expect("head").expect("a commit");
    std::fs::write(dir.join("new.ts"), "a\nB\nc\nd\n").expect("write");
    let added = repo.added_lines(Some(&base), &[]).expect("diff");
    assert_eq!(added["new.ts"], [2..=2, 4..=4]);
    assert!(
        repo.added_lines(Some(&base), &["other.ts"])
            .expect("diff")
            .is_empty()
    );
    assert!(repo.added_lines(Some("no-such-rev"), &[]).is_err());
    assert_eq!(repo.files().expect("files"), ["new.ts"]);
}
