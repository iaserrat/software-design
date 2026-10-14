//! What git knows about a repository: which files exist, how often each one
//! changes, which files change together, and which lines a change added.
//!
//! Everything goes through the `git` command line, so the answers match what the
//! user's own `git` reports, including `.gitignore` and repository configuration.

use std::collections::HashMap;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git's well-known hash of the empty tree: the base to diff against before the
/// first commit exists.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Commits touching more files than this are bulk edits (renames, formatting,
/// dependency bumps); their files did not change together for a design reason.
const BULK_COMMIT_FILES: usize = 30;

/// The repository containing `dir`.
#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
}

/// How often files changed over a time window.
#[derive(Debug, Default)]
pub struct History {
    /// Commits that touched each path.
    pub churn: HashMap<String, u32>,
    /// Pairs of paths and the number of commits that touched both, `a < b`.
    pub together: HashMap<(String, String), u32>,
}

impl Repo {
    /// Finds the repository that contains `dir`.
    ///
    /// # Errors
    /// When `git` is missing or `dir` is not inside a work tree.
    pub fn discover(dir: &Path) -> io::Result<Repo> {
        let out = git(dir, &["rev-parse", "--show-toplevel"])?;
        Ok(Repo {
            root: PathBuf::from(out.trim_end()),
        })
    }

    /// The work tree root. Every path in this crate is relative to it.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The current commit, or `None` before the first commit.
    ///
    /// # Errors
    /// When `git` fails for a reason other than a missing `HEAD`.
    pub fn head(&self) -> io::Result<Option<String>> {
        Ok(self
            .verify("HEAD")
            .then(|| git(&self.root, &["rev-parse", "HEAD"]))
            .transpose()?
            .map(|s| s.trim().to_owned()))
    }

    /// Tracked files plus untracked files that are not ignored.
    ///
    /// # Errors
    /// When `git ls-files` fails.
    pub fn files(&self) -> io::Result<Vec<String>> {
        let out = git(
            &self.root,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--deduplicate",
            ],
        )?;
        Ok(out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Churn and co-change counts for commits since `since` (any date `git log
    /// --since` accepts, such as `12 months ago`). Merge commits are skipped.
    ///
    /// # Errors
    /// When `git log` fails.
    pub fn history(&self, since: &str) -> io::Result<History> {
        if !self.verify("HEAD") {
            return Ok(History::default());
        }
        let since = format!("--since={since}");
        let out = git(
            &self.root,
            &[
                "log",
                "--no-merges",
                "--no-renames",
                "--name-only",
                "--format=%x00",
                &since,
            ],
        )?;
        let mut history = History::default();
        for commit in out.split('\0') {
            let mut paths: Vec<&str> = commit.lines().filter(|l| !l.is_empty()).collect();
            paths.sort_unstable();
            paths.dedup();
            for path in &paths {
                *history.churn.entry((*path).to_owned()).or_default() += 1;
            }
            if paths.len() > BULK_COMMIT_FILES {
                continue;
            }
            for (i, a) in paths.iter().enumerate() {
                for b in &paths[i + 1..] {
                    *history
                        .together
                        .entry(((*a).to_owned(), (*b).to_owned()))
                        .or_default() += 1;
                }
            }
        }
        Ok(history)
    }

    /// Lines added or changed since `base`, per path, including uncommitted and
    /// untracked files. `base` defaults to `HEAD`. Deleted files are absent.
    /// A non-empty `paths` limits the diff to those paths.
    ///
    /// # Errors
    /// When `base` is not a commit, or `git diff` fails.
    pub fn added_lines(
        &self,
        base: Option<&str>,
        paths: &[&str],
    ) -> io::Result<HashMap<String, Vec<RangeInclusive<u32>>>> {
        let base = match base {
            Some(rev) if self.verify(rev) => rev,
            Some(rev) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("not a commit: {rev}"),
                ));
            }
            None if self.verify("HEAD") => "HEAD",
            None => EMPTY_TREE,
        };
        let mut args = vec![
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            "-U0",
            base,
            "--",
        ];
        args.extend(paths);
        let mut added = parse_diff(&git(&self.root, &args)?);
        let mut args = vec!["ls-files", "-z", "--others", "--exclude-standard", "--"];
        args.extend(paths);
        let untracked = git(&self.root, &args)?;
        for path in untracked.split('\0').filter(|p| !p.is_empty()) {
            let lines = std::fs::read(self.root.join(path))
                .map_or(0, |bytes| String::from_utf8_lossy(&bytes).lines().count());
            added.insert(
                path.to_owned(),
                vec![1..=u32::try_from(lines).unwrap_or(u32::MAX)],
            );
        }
        Ok(added)
    }

    fn verify(&self, rev: &str) -> bool {
        git(
            &self.root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ],
        )
        .is_ok()
    }
}

/// Added line ranges from `git diff -U0` output.
fn parse_diff(diff: &str) -> HashMap<String, Vec<RangeInclusive<u32>>> {
    let mut added: HashMap<String, Vec<RangeInclusive<u32>>> = HashMap::new();
    let mut path = None;
    for line in diff.lines() {
        if let Some(target) = line.strip_prefix("+++ ") {
            path = target.strip_prefix("b/").map(str::to_owned);
        } else if let (Some(hunk), Some(path)) = (line.strip_prefix("@@ "), &path) {
            // `@@ -a,b +c,d @@`: the new side starts at c and spans d lines (default 1).
            let Some(new_side) = hunk.split(' ').find_map(|part| part.strip_prefix('+')) else {
                continue;
            };
            let (start, count) = new_side.split_once(',').unwrap_or((new_side, "1"));
            let (Ok(start), Ok(count)) = (start.parse::<u32>(), count.parse::<u32>()) else {
                continue;
            };
            if count > 0 {
                added
                    .entry(path.clone())
                    .or_default()
                    .push(start..=start + count - 1);
            }
        }
    }
    added
}

fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    String::from_utf8(out.stdout).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::parse_diff;

    #[test]
    fn diff_hunks_become_new_side_line_ranges() {
        let diff = "diff --git a/x.ts b/x.ts\n--- a/x.ts\n+++ b/x.ts\n@@ -3 +3,2 @@\n-a\n+b\n+c\n@@ -9,2 +10,0 @@\n-gone\n@@ -20 +20 @@\n-x\n+y\n";
        assert_eq!(parse_diff(diff)["x.ts"], [3..=4, 20..=20]);
    }

    #[test]
    fn deleted_files_have_no_added_lines() {
        let diff = "--- a/old.ts\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-a\n";
        assert!(parse_diff(diff).is_empty());
    }
}
