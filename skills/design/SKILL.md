---
name: design
description: Designs the interface of a new class, module, API, service or feature before its code is written. Lists the knowledge the module hides, designs three competing interfaces in parallel, tests each on a reader who has never seen the code, compares the caller code each needs, and writes interface comments first. Use whenever the user asks to design, sketch, shape, structure or plan a class, module, API, interface or service, asks which methods or signatures something should have, wants to compare designs, or says "design it twice", even when they only describe what the code must do. Typical requests include "design the class interface", "what methods should this class have", "how should I structure this module", "sketch an API for", "design X before I write it", or any description of a class, module or API that does not exist yet.
license: MIT
compatibility: Claude Code, Codex or pi with the software-design plugin, whose MCP server is built from source with Rust (rustup).
allowed-tools: Read Grep Glob Agent AskUserQuestion mcp__plugin_software-design_software-design__interface mcp__plugin_software-design_software-design__check
---

# Design a module

The target is the deepest module that covers today's needs: the most functionality behind the smallest interface, with an interface that is not tied to the one caller that exists today. The rule table, with every principle, red flag and question, is `rules.json` at the plugin root.

**Tools.** The plugin's `software-design` MCP server provides two tools: `check` returns findings as JSON; `interface` returns signatures and interface comments without bodies. In Claude Code their full names are `mcp__plugin_software-design_software-design__check` and `mcp__plugin_software-design_software-design__interface`. The plugin root is two directories above this skill. Pass `directory`, the repository's absolute path, on every call: outside Claude Code the server does not run in the project. If neither tool is available, run the global installer at `https://github.com/iaserrat/software-design` to build the runtime, then start a new session. From an installed plugin directory, `sh scripts/software-design install` also builds it synchronously (log: `~/.software-design/build.log`, needs Rust and a C compiler).

**Agents.** This skill delegates to the plugin's agents, such as `software-design:flag-verifier`. Each agent's instructions are in the plugin root's `agents` directory, in the Markdown file named after the agent. Where those agent types do not exist, as outside Claude Code, give each task to a general subagent together with that file's instructions. With no subagents at all, follow the instructions yourself, one task at a time; for a cold read, look only at the `interface` tool's output before reading any implementation, and say in the report that the read was not independent.

Copy this checklist and tick it off as you go:

```
- [ ] 1. Use cases written as concrete calls; existing modules checked
- [ ] 2. Knowledge listed and assigned to one module each
- [ ] 3. Three alternatives designed in parallel
- [ ] 4. Each alternative cold-read
- [ ] 5. Comparison table shown; user chose
- [ ] 6. Interface comments written before code; none hard to describe
```

Always run steps 3 and 4 with agents, whatever the size of the request; **Agents** above covers hosts without them. The value of this skill is the comparison between independent designs and the reader who has not seen them; a design written inline by one author has neither.

## 1. Ground the design in the code

- Read where the module will be used. Write down every **current use case** as a concrete call the code will make. Future use cases do not count: functionality covers today's needs only.
- Look for an existing module that already owns this knowledge. Use the `interface` tool on candidates. Extending a module is often deeper than adding one: every new module, class, parameter and file is something readers must learn, so it has to remove more complexity than it adds.
- Read the repository's conventions before designing: naming, error style, file layout. A design that breaks conventions costs more than it saves.
- Ask the user only what the code cannot answer, one question at a time, with your recommended answer first.

## 2. List the knowledge, then assign it

List each design decision the work involves: data formats, protocols, policies, invariants, data structures, error cases. Assign each one to exactly one module. Then check the assignment:

- **Information leakage:** a decision assigned to two modules. Merge them, or pull the decision into one.
- **Temporal decomposition:** modules that follow the order operations happen in ("read", then "parse", then "write") while sharing knowledge. Split by knowledge instead.
- **Special-general mixture:** a general mechanism holding code for one use. Move that code up to the use.

## 3. Design it twice

Spawn three `software-design:design-alternative` agents in parallel. Do this for small classes too: a second design is worth sketching even when the first one looks obviously right, because the comparison is what shows the first one's weaknesses. The agents run in parallel, so the cost is a short wait. Give each the use cases, the knowledge list, the constraints and a different **stance**. Choose stances that differ fundamentally for this problem. Draw them from these axes:

- granularity of operations: per item, per range, or per document (for a text editor's buffer: per character, per line, or per range of text);
- general versus special purpose;
- one module versus two: together when the parts share information or a combined interface is simpler, apart when each can be understood alone;
- where errors go: defined out of existence, masked inside, or aggregated in one handler.

## 4. Test every interface on a cold reader

For each design, spawn a `software-design:cold-reader` agent, even when you believe the interface is clear: its author is the person least able to see what is missing. Pass it **only** that design's signatures and interface comments. The reader lists every question it cannot answer from the interface alone. Each such question is information a caller needs that the abstraction leaves out.

## 5. Compare and recommend

Present one table:

| Measure | Why it matters |
|---|---|
| Interface elements: methods, parameters, exceptions, configuration | The interface is the module's cost |
| Caller code needed for all current use cases | Ease of use for today's needs |
| Questions the cold reader could not answer | Gaps in the abstraction |
| Methods that serve exactly one use case | Too special-purpose |
| Errors every caller must handle | Exceptions widen interfaces |
| Knowledge that appears in more than one module | Information leakage |

Recommend one design. When the measures are close, prefer the design whose methods each serve several use cases: one method per caller action (a `backspace()` beside a `delete()`) puts the caller's knowledge into the module and multiplies its interface, while a general operation such as deleting a range serves every action. If several are weak, combine their strengths, or run another round driven by the weaknesses you found. Ask the user to choose with AskUserQuestion, your recommendation listed first.

## 6. Write the comments first

For the chosen design, write the interface comment of each module and public method, then the signatures, and only then the bodies. If a comment is hard to write or comes out long, the design has a problem: flag **Hard to Describe** and revisit it before writing code. Follow the repository's comment conventions for length and format; the comment's job is what the code cannot say: units, boundaries, invariants, errors, and why.

## Where these principles stop

- Do not hide information callers need. Hiding it makes a false abstraction.
- Pulling complexity down pays only when it is related to the module's job, simplifies callers, and simplifies the interface.
- Fewer lines is not the goal. Fewer things a caller must know is.
- A long method with a simple signature is fine; do not split code only to make it short.
