---
name: audit
description: Audit a whole repository or a large directory for design complexity. Ranks red flags by how often each file changes, verifies candidates against each rule's exemptions in parallel, has a cold reader judge the interfaces of the most-changed modules, and returns the investments worth making first. Use when the user asks for an audit, a design or complexity review of an entire codebase or package, a hotspot map, or where design work would pay off most. Use it even when the user only asks where the code is worst or what to clean up first.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob Agent AskUserQuestion Workflow mcp__plugin_software-design_software-design__check mcp__plugin_software-design_software-design__interface
---

# Audit a codebase

Complexity is what developers experience in the code they work on, weighted by how often they work on it. The audit therefore starts from churn: findings in files that change every week come first; findings in files nobody touches come last.

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, the plugin's binary is not built yet: tell the user it builds in the background at session start (log: `~/.software-design/build.log`, needs Rust from https://rustup.rs), or that `sh scripts/software-design install`, run from the plugin root, builds it now; then a new session connects the tools (in Claude Code, `/mcp` reconnects them).

**Agents.** This skill delegates to the plugin's agents, such as `software-design:flag-verifier`. Each agent's instructions are in the plugin root's `agents` directory, in the Markdown file named after the agent. Where those agent types do not exist, as outside Claude Code, give each task to a general subagent together with that file's instructions. With no subagents at all, follow the instructions yourself, one task at a time; for a cold read, look only at the `interface` tool's output before reading any implementation, and say in the report that the read was not independent.

Copy this checklist and tick it off as you go:

```
- [ ] 1. Depth chosen: workflow only if the user asked for an audit
- [ ] 2. Hotspots, confirmed findings and cold reads collected
- [ ] 3. Investments ordered by churn times findings removed
- [ ] 4. User chose the first investment
```

## 1. Choose the depth

- **The user asked for an audit in their own words:** run the bundled multi-agent workflow:

  `Workflow({ name: "software-design:deep-audit", args: { paths: [<directory, or omit for the whole repository>], verify: 12, modules: 3 } })`

  It runs the `check` tool at tier B, verifies the twelve heaviest candidates in parallel with `software-design:flag-verifier`, and sends the interfaces of the three most-changed modules to `software-design:cold-reader`. It logs what it did not cover.

  Where the `Workflow` tool does not exist, as outside Claude Code, run the same steps yourself: the `check` tool at tier B, `software-design:flag-verifier` on the twelve heaviest candidates, and `software-design:cold-reader` on the directories of the three heaviest findings. Log what you did not cover.

- **You decided an audit would help, without the user asking for one:** do not start the workflow. Call the `check` tool with `tier: "B"` yourself, summarise the hotspots, and offer the full audit, stating that it runs about fifteen agents.

## 2. Report

1. **Hotspots:** the files with the most churn and the most findings, as a table of `path`, commits in the last year, confirmed findings, and red flags.
2. **Confirmed findings:** grouped by red flag, each with `path:line`, its section, and the recommended fix.
3. **Interfaces a newcomer cannot use:** the cold reader's unanswered questions per module.
4. **Investments, in order:** each a concrete change, the files it touches, and the red flags it removes. Order by churn times the number of findings removed. A steady 10 to 20 percent of development time spent on such investments pays for itself without stalling feature work, so propose a first step that fits that budget.

Ask the user with AskUserQuestion which investment to start, recommendation first, and hand the chosen one to the `software-design:change` skill.
