---
name: review
description: Reviews code for design red flags, such as shallow modules, pass-through methods, information leakage, repeated code, comments that repeat code, vague names and ignored errors, and tests whether public interfaces can be used without reading their implementation. Ranks findings by how often the code changes and verifies each against the rule's own exemptions. Use when the user asks to review a diff, branch, pull request, file or directory for design quality, complexity or maintainability, asks whether code is well designed, or asks whether an API, class or module is clear or usable without reading its code.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob Agent AskUserQuestion mcp__plugin_software-design_software-design__check mcp__plugin_software-design_software-design__interface
---

# Review for red flags

A red flag says code is probably more complicated than it needs to be. It is evidence, not a verdict: every rule has limits, and `rules.json` lists them as exemptions. The rule table is `rules.json` at the plugin root; each detector there lists its signal, its exemptions and its fixes.

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, run the global installer at `https://github.com/iaserrat/software-design` to build the runtime, then start a new session. From an installed plugin directory, `sh scripts/software-design install` also builds it synchronously (log: `~/.software-design/build.log`, needs Rust and a C compiler).

**Agents.** This skill delegates to the plugin's agents, such as `software-design:flag-verifier`. Each agent's instructions are in the plugin root's `agents` directory, in the Markdown file named after the agent. Where those agent types do not exist, as outside Claude Code, give each task to a general subagent together with that file's instructions. With no subagents at all, follow the instructions yourself, one task at a time; for a cold read, look only at the `interface` tool's output before reading any implementation, and say in the report that the read was not independent.

Copy this checklist and tick it off as you go:

```
- [ ] 1. Scope chosen; check run at tier B
- [ ] 2. Tier A confirmed by reading; tier B verified by flag-verifier agents
- [ ] 3. Changed code read for the judgment-only red flags
- [ ] 4. Public interfaces cold-read
- [ ] 5. At most five findings reported, heaviest first
- [ ] 6. User decided; fixes re-checked until check reports none
```

## 1. Choose the scope

- With uncommitted work or a branch: review the change. Call the `check` tool with `changed: true`, or `base` set to the branch's merge base (for example `main`).
- With paths: review those paths.
- A whole repository is an audit; use the `software-design:audit` skill for it.

Call the `check` tool with `tier: "B"` and the scope. Findings come ordered by `weight`, the number of commits that touched the file in the last year: complexity in code nobody changes costs little.

## 2. Verify before reporting

- **Tier A** findings are precise. Read the line to confirm the context, then report.
- **Tier B** findings are candidates. Spawn one `software-design:flag-verifier` agent per candidate, in parallel, for the ten with the highest weight. Pass the finding JSON and its detector entry from `rules.json`. Report only CONFIRMED findings. Say how many candidates were not verified.

## 3. Read for what a parser cannot see

Read the changed code for the red flags that need judgment, using the `ask` question of each in `rules.json`: Temporal Decomposition, Special-General Mixture, Conjoined Methods, Overexposure, Hard to Pick Name, Hard to Describe, Nonobvious Code. Report one only when you can point at the lines and name the knowledge involved.

## 4. Test public interfaces on a cold reader

For each file whose public interface the review covers (or the one the user asked about), spawn a `software-design:cold-reader` agent with the file's path. It sees only signatures and interface comments, and lists the questions a caller cannot answer from them. Each such question is information the interface leaves out; report the ones that would change how a caller writes code.

## 5. Report

Report at most five findings, highest weight first. For each:

- the red flag and its section, and `path:line`;
- the evidence: what the code does, in one or two sentences;
- the cost: change amplification (one change needs edits in many places), cognitive load (a developer must know a lot to make a change) or unknown unknowns (it is not clear what must change);
- the fix options from `rules.json`, with the one you recommend;
- the question for the user when the decision is theirs, for example whether callers need the information being hidden.

List the rest in one line each. Do not report style preferences that no rule in `rules.json` covers.

## 6. Act only on the user's decision

Ask with AskUserQuestion which findings to fix, recommendation first. After fixing, call the `check` tool again with `changed: true` and `tier: "A"` to confirm the red flags are gone. A deliberate exception stays in the code with a comment `software-design-allow: <rule> <reason>`; the reason is the design decision the next reader needs.
