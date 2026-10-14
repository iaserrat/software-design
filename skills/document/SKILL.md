---
name: document
description: Write or improve the interface and implementation comments of a module. Comments carry what the code cannot say (units, boundaries, invariants, errors, reasons) and never repeat the code. A comment that is hard to write is reported as a design problem. Use when the user asks to document, comment or explain a module, class, function or API, to fix poor or outdated comments, or when the software-design hooks report comments that repeat the code. Use it even when the user only asks to add comments or docstrings.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob Edit AskUserQuestion mcp__plugin_software-design_software-design__check mcp__plugin_software-design_software-design__interface
---

# Document a module

Without comments there is no abstraction: a declaration alone cannot say what a method promises. A good comment states what the code cannot: it is more precise than the code (units, boundaries, nulls, ownership, invariants) or more abstract (what a block is for and why it exists).

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, the plugin's binary is not built yet: tell the user it builds in the background at session start (log: `~/.software-design/build.log`, needs Rust from https://rustup.rs), or that `sh scripts/software-design install`, run from the plugin root, builds it now; then a new session connects the tools (in Claude Code, `/mcp` reconnects them).

## 1. Read before writing

- Follow the repository's comment conventions: format, density and length. Do not add comments the codebase's style would reject.
- Call the `check` tool with `tier: "A"` on the paths to find comments that repeat the code and interface comments that describe internals.

## 2. Interface comments

For each public type and method, describe what a caller needs and nothing more:

- one or two sentences on the behaviour as a caller sees it;
- each parameter and the return value, with units, ranges, and what null or empty means;
- side effects, errors and preconditions;
- no implementation details: if the comment must describe the implementation to be complete, the module is shallow.

Use different words from the name being documented. Test: could someone who has never seen the code write this comment from the declaration alone? If yes, it adds nothing.

## 3. Implementation comments

Comment what a block does and why, not how: a line before each major phase of a long method, a line before a loop whose purpose is not obvious, and the reason behind any code that looks wrong but is needed. Put each comment at the narrowest scope that covers its code.

## 4. Report design problems instead of hiding them

If a comment for a method or variable cannot be both short and complete, report **Hard to Describe** with the reason, and propose the design change, instead of writing a long comment. If no precise name fits a variable or method, report **Hard to Pick Name** the same way.
