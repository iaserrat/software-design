---
name: release
description: Bump the version in every manifest, update the changelog, tag main and publish a GitHub release marked latest.
disable-model-invocation: true
---

# Release

One version names the whole release. It lives in exactly these places:

- `crates/software-design/Cargo.toml` (`version = "…"` under `[package]`; the launcher reads it)
- `Cargo.lock` (the `software-design` package entry; cargo rewrites it)
- `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json`, `package.json` (`"version"`)
- `CHANGELOG.md` (a `## X.Y.Z` heading)

The internal crates (`history`, `rules`, `syntax`) stay at `0.0.0`: they are unpublished. `tests/tidy.rs::every_version_names_the_same_release` enforces the manifests agree.

## 1. Preconditions

Stop and tell the user if any fails:

- `git status --porcelain` is empty and the branch is `main`.
- `git fetch origin && git status -sb` shows main is not behind `origin/main`.
- `gh auth status` succeeds.

## 2. Propose the bump

- Current version: from `crates/software-design/Cargo.toml`.
- Last release: `git describe --tags --abbrev=0` (none yet → everything since the first commit).
- Read `git log <last-tag>..HEAD --oneline` and the diff stat. Classify:
  - **major**: removed or renamed skill/agent/MCP tool/CLI flag, changed JSON output schema (`SCHEMA_VERSION`), changed hook behavior users rely on. While on `0.x`, a breaking change bumps **minor**.
  - **minor**: new rule, skill, agent, tool, language or option.
  - **patch**: fixes, docs, internal refactors only.
- Ask the user with AskUserQuestion: options patch / minor / major, each label showing the resulting version, the recommended one first with "(Recommended)" and a one-line reason drawn from the commits. Do not continue without an answer.

## 3. Bump

1. Replace the old version with the new one in the three JSON manifests and in `crates/software-design/Cargo.toml` (the `[package]` `version` line only — never dependency versions).
2. Add `## X.Y.Z` at the top of `CHANGELOG.md` (under `# Changelog`), with terse bullets in the existing style summarising user-visible changes since the last tag. Show it to the user and apply their edits.
3. Run the CI checks; they also refresh `Cargo.lock`:
   ```bash
   cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
   ```
4. Confirm alignment: `git grep -n "X.Y.Z"` lists every file above, and `git diff --stat` touches nothing else besides `Cargo.lock`.

## 4. Commit, tag, push

Show the diff and get an explicit yes before pushing.

```bash
git commit -am "Release X.Y.Z"
git tag -a vX.Y.Z -m "X.Y.Z"
git push origin main vX.Y.Z
```

## 5. GitHub release

Notes are the `## X.Y.Z` section of `CHANGELOG.md` (without the heading):

```bash
awk '/^## /{p = ($2 == "X.Y.Z"); next} p' CHANGELOG.md > "$TMPDIR/notes.md"
gh release create vX.Y.Z --verify-tag --title "X.Y.Z" --notes-file "$TMPDIR/notes.md" --latest
```

Then `gh release list --json tagName,isLatest --jq '.[] | select(.isLatest) | .tagName'` must print `vX.Y.Z`; give the user the URL from `gh release view vX.Y.Z --json url`.
