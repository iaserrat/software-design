---
name: perf
description: Make code faster without making it more complex. Measures before changing anything, looks for a fundamental fix first, and otherwise redesigns around the critical path with special cases moved off it. Use when the user asks to optimise, speed up, or reduce the latency, allocations or memory of code, or to review a change made for performance. Use it even when the user names a specific micro-optimisation, because measuring comes first.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob Bash AskUserQuestion mcp__plugin_software-design_software-design__interface
---

# Design for performance

Clean design and speed usually agree: simple code does less redundant work, and deep modules cross fewer layers. Intuition about what is slow is unreliable, so nothing changes before it is measured.

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, the plugin's binary is not built yet: tell the user it builds in the background at session start (log: `~/.software-design/build.log`, needs Rust from https://rustup.rs), or that `sh scripts/software-design install`, run from the plugin root, builds it now; then a new session connects the tools (in Claude Code, `/mcp` reconnects them).

## 1. Measure

- Find or write a benchmark that reproduces the slow operation. Record the baseline numbers with the command that produced them.
- Measure below the top level until a small number of specific places account for most of the time. If the user cannot share a production profile, ask for one with AskUserQuestion rather than guessing.

## 2. Look for a fundamental fix

A cache, a different algorithm or data structure, or avoiding work entirely. Prefer the naturally efficient choice that is just as simple: a hash map over an ordered map when order is unused, one allocation instead of many.

## 3. Otherwise, design around the critical path

1. Write down the smallest amount of code the most common case must run, ignoring the current structure: "the ideal".
2. Find a clean design that comes as close to the ideal as possible.
3. Move special cases off the critical path: ideally one test at the start detects every special case, and the common case runs without further checks.
4. Remove shallow layers on the path; each crossing costs time and adds interface.

## 4. Keep only what helps

Re-run the benchmark with the same command. Keep a change only if it is measurably faster or simpler. Report the before and after numbers.
