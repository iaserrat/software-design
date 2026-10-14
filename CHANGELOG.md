# Changelog

## 0.1.0

- Detectors for TypeScript, JavaScript, React, Vue, Python, PHP and Rust: five precise (tier A) and eight candidate (tier B) design rules, ranked by git churn.
- React: a component counts as called wherever it appears in JSX, with each prop as a named argument, so `shallow-function`, `constant-argument` and `hidden-coupling` see React code. `forwardRef` and `memo` components take the name of their `const`.
- Skills: `design`, `review`, `change`, `audit`, `document`, `perf`.
- Agents: `flag-verifier`, `cold-reader`, `design-alternative`.
- Hooks that report red flags in lines Claude just wrote, with an optional mode that blocks the end of a turn.
- MCP server with `check` and `interface` tools.
- Codex plugin and pi package: the same skills, MCP server and hooks.
- A launcher that builds the plugin's source into `~/.software-design` from a detached `SessionStart` hook that survives the host exiting, and refuses a binary of another version.
