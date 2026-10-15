# Changelog

## 0.1.2

- The plugin installs on Claude Code versions before 2.1.271. `hook_mode` in `/config` is now a text field; values other than `off` and `block` mean `context`.

## 0.1.0

First release.

- Red-flag detectors for TypeScript, JavaScript, React, Vue, Python, PHP and Rust: five precise (tier A) and eight candidate (tier B) design rules, ranked by git churn. React components count as called wherever they appear in JSX, with each prop as a named argument.
- Skills: `design`, `review`, `change`, `audit`, `document`, `perf`.
- Agents: `flag-verifier`, `cold-reader`, `design-alternative`.
- A `deep-audit` workflow that ranks findings by churn and verifies the heaviest ones.
- A `software-design` CLI with `--tier`, `--diff`, `--base`, `--since` and `--strict`.
- MCP server with `check` and `interface` tools.
- Hooks that report red flags in lines Claude just wrote, with an optional `block` mode that refuses to end a turn while tier A flags remain.
- Codex plugin and pi package with the same skills, MCP server and hooks.
- A launcher that builds the plugin into `~/.software-design` in the background at session start, refuses a binary of another version, and says when a newer version is out.
