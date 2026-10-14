---
name: change
description: Plan a change to existing code strategically. Before editing, works out what the code would look like had it been designed with this change in mind, compares that with the smallest patch, and lets the user choose. After editing, checks the diff for red flags and stale comments. Use when the user asks to add a feature to, fix, or extend existing code and the change adds a parameter, flag, special case or exception, touches a module's interface, or spreads across several files. Use it even when the request sounds like a small patch, because small patches are how complexity accumulates.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob AskUserQuestion mcp__plugin_software-design_software-design__check mcp__plugin_software-design_software-design__interface
---

# Change existing code strategically

Every small patch that adds a special case, a parameter or a dependency makes the design a little worse, and the damage accumulates. After a change, the code should look as if it had been designed with the change in mind from the start. The user decides how much that is worth today.

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, the plugin's binary is not built yet: tell the user it builds in the background at session start (log: `~/.software-design/build.log`, needs Rust from https://rustup.rs), or that `sh scripts/software-design install`, run from the plugin root, builds it now; then a new session connects the tools (in Claude Code, `/mcp` reconnects them).

Copy this checklist and tick it off as you go:

```
- [ ] 1. Touched modules, callers and existing red flags read
- [ ] 2. Options laid out with costs; user chose
- [ ] 3. Change implemented, comments updated in the same edit
- [ ] 4. Diff checked until check reports no tier A findings
```

## 1. Understand what the change touches

- Read the modules the change touches and their callers. Call the `interface` tool on them to see the abstraction callers rely on.
- Call the `check` tool with `tier: "A"` on those paths to see the red flags already there.
- Name the knowledge the change needs and where that knowledge lives now. If it lives in several places, that is information leakage, and the change will amplify it.

## 2. Lay out the options

Describe two or three options. For each, give the files it touches, the callers it changes and the complexity it leaves behind:

- **Smallest patch.** Count what it adds: special cases, parameters, exceptions, flags, places a future change must also touch.
- **Designed-in.** The structure the module would have if this need had been known at the start: a redefined operation that makes the special case normal, knowledge moved into one module, a deeper method replacing several shallow ones.
- **Middle option**, when there is one: nearly as clean as designed-in at a fraction of the cost.

Recommend one. Ask the user to choose with AskUserQuestion. When the designed-in option is too large for now, say so plainly and propose recording the follow-up where the next developer will see it.

## 3. Implement

- Follow the repository's conventions.
- Keep comments next to the code they describe. Information a future developer needs goes in the code, not only in the commit message.
- Update interface comments in the same edit as the signatures they describe.

## 4. Check the diff

Call the `check` tool with `changed: true` and `tier: "A"`. Fix each finding, or keep it with `software-design-allow: <rule> <reason>` when the design is intended. Then read the diff once more for comments that no longer match the code and for debugging code left behind.
