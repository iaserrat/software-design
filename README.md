<div align="center">
<pre>
█▀▀ █▀█ █▀▀ ▀█▀ █ █ █▀█ █▀▄ █▀▀   █▀▄ █▀▀ █▀▀ ▀█▀ █▀▀ █▀█
▀▀█ █ █ █▀▀  █  █▄█ █▀█ █▀▄ █▀▀   █ █ █▀▀ ▀▀█  █  █ █ █ █
▀▀▀ ▀▀▀ ▀    ▀  ▀ ▀ ▀ ▀ ▀ ▀ ▀▀▀   ▀▀  ▀▀▀ ▀▀▀ ▀▀▀ ▀▀▀ ▀ ▀
</pre>
</div>

<p align="center"><b>Smaller, deeper software, while your coding agent writes it.</b></p>

A plugin for Claude Code, Codex and pi that applies *A Philosophy of Software Design* by John Ousterhout while your agent writes code. It finds the book's red flags in the lines the agent just wrote and reports them before the turn ends.

It also helps the agent design new modules the way the book describes. Design it twice, test the interface on someone who has never seen the code, and write the comments first.

Works on TypeScript, JavaScript, React, Vue, Python, PHP and Rust.

---

## Why

Coding agents add code quickly and rarely notice what each addition costs. A new flag or a method that only passes its arguments along looks harmless on its own. Enough of them make a codebase hard to change.

This plugin checks the lines the agent changed after every edit and reports red flags while the context is still fresh. It weights each finding by how often its file changed in the last year. Complexity in code nobody touches costs little, while complexity in a hotspot costs you every week.

Every rule carries the exemptions the book gives, and the plugin checks candidates against them before it asks anyone to act. For new code, it has the agent sketch three competing interfaces, lets a fresh reader try each one, and compares the code that callers would have to write.

## What it looks like

The agent writes a method that only forwards to another one. Before it moves on, it hears this:

```text
software-design: the edit introduced design red flags. Fix them now, or keep the code
and add `software-design-allow: <rule> <reason>` in a comment above it.
- src/editor.ts:6 pass-through-method: `insert` only forwards its arguments to `this.area.insert`.
```

It can fix the code, or keep it and say why:

```ts
// software-design-allow: pass-through-method keeps the v1 API stable for plugin authors
insert(text: string, at: number) { this.area.insert(text, at); }
```

The plugin ignores a suppression without a reason, because the reason is the design decision the next reader needs.

Ask for a review, a design or an audit, and the matching skill takes over. You can also call one by name.

## Install

The plugin builds from source on your machine, so you need Rust and a C compiler first. Install Rust with [rustup](https://rustup.rs):

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If you don't have a C compiler yet, install one:

```
xcode-select --install             # macOS
sudo apt install build-essential   # Debian and Ubuntu
```

You don't need to pick a Rust version: `rust-toolchain.toml` pins it, and rustup fetches it on the first build.

**Claude Code**

```
/plugin marketplace add iaserrat/software-design
/plugin install software-design@software-design
```

**Codex**

```
codex plugin marketplace add iaserrat/software-design
codex plugin add software-design@software-design
```

**pi**

```
pi install git:github.com/iaserrat/software-design
```

### The first session

The first session after installing starts a build in the background. It runs `cargo install --locked` on the plugin's own source. The binary goes to `~/.software-design/bin/software-design` and the log to `~/.software-design/build.log`. A cold build takes a few minutes, and it keeps running if you close the session.

The red-flag checks start as soon as it finishes. In Claude Code, run `/mcp` to reconnect the plugin's tools in that first session; in Codex and pi, start a new session.

To build before your first session, run the launcher yourself. For Claude Code:

```
sh ~/.claude/plugins/marketplaces/software-design/scripts/software-design install
```

When the plugin updates to a new version, the next session rebuilds. The launcher won't run a binary whose version differs from the plugin's, so an old build never runs against new skills. Once a day the launcher checks the version on `main`, and a session start tells you when a newer one is out.

## What you get

| | What it does |
|---|---|
| `design` skill | Designs a new module before its code exists. It lists the knowledge the module should hide and designs three alternatives in parallel. A cold reader tests each interface, the skill compares the caller code each one needs, and it writes the interface comments before the code. |
| `review` skill | Reviews a change, a branch or a set of paths. It checks each candidate against its rule's exemptions and reports the five findings that cost the most. |
| `change` skill | Before editing existing code, it compares the smallest patch with the design the code would have if someone had planned for this change from the start. You choose. |
| `audit` skill | Audits a whole repository. You get hotspots ranked by churn, verified findings, cold reads of the most-changed modules, and the investments to make first. |
| `document` skill | Writes interface and implementation comments that say what the code cannot. When a comment is hard to write, it reports that as a design problem. |
| `perf` skill | Makes code faster without making it more complex. It measures first and looks for a fundamental fix. Failing that, it designs around the critical path. |
| Hooks | After each edit, they report red flags in the lines just written, once per session. In `block` mode, they stop the turn from ending while those red flags remain. |
| MCP tools | `check` finds red flags and returns them as JSON. `interface` prints signatures and interface comments, without bodies. |

In Claude Code, the skills also use three agents. `flag-verifier` checks one finding against its exemptions. `cold-reader` sees only an interface and lists every question it cannot answer. `design-alternative` produces one complete design under an assigned stance.

### On each host

| | Claude Code | Codex | pi |
|---|---|---|---|
| Calling a skill by name | `/software-design:review` | `$software-design:review` | `/skill:review` |
| MCP tools | ✓ | ✓ | ✓ (pi's built-in MCP) |
| Hooks after edits and at the end of a turn | ✓ | ✓ | ✓ |
| Plugin agents and the audit workflow | ✓ | general subagents | general subagents, or inline |
| Choosing the hook mode | `/config` | `SOFTWARE_DESIGN_HOOK_MODE` | `SOFTWARE_DESIGN_HOOK_MODE` |

Every skill also starts on its own when a request matches it: "review this branch", "how should I structure this module", "where is this codebase worst".

In pi, the package registers its MCP server through pi's built-in MCP support. If you turned that off with `"extensions": ["-builtin:mcp"]` and use an MCP package instead, that package has to connect servers that extensions register.

## Configuration

**Hook mode.** In Claude Code, set `hook_mode` in `/config` or when you enable the plugin. In Codex and pi, set the environment variable `SOFTWARE_DESIGN_HOOK_MODE`.

| Mode | Behaviour |
|---|---|
| `context` (default) | After each edit, the plugin tells the agent about red flags in the lines it just wrote. |
| `block` | The same, and the agent cannot end its turn while those red flags remain in the session's changes. |
| `off` | No hooks. |

**Paths.** Set `SOFTWARE_DESIGN_HOME` to build somewhere other than `~/.software-design`, or `SOFTWARE_DESIGN_BIN` to run a binary you built yourself.

## The rules

Tier A detectors are precise enough to report as they are, and the hooks use them. Tier B detectors produce candidates that a reader verifies before anyone acts on them. [`rules.json`](rules.json) holds the full table, with each rule's signal, exemptions and fixes, and the skills read the same file.

| Detector | Tier | Red flag or principle | Book section |
|---|---|---|---|
| `pass-through-method` | A | Pass-Through Method | 7.1 |
| `comment-repeats-code` | A | Comment Repeats Code | 13.2 |
| `interface-mentions-private` | A | Implementation Documentation Contaminates Interface | 13.5 |
| `exposed-collection` | A | Information Leakage | 5.6 |
| `stale-interface-comment` | A | Nonobvious Code | 16.5 |
| `shallow-function` | B | Shallow Module | 4.5 |
| `ignored-error` | B | Define errors out of existence | 10.3 |
| `repeated-code` | B | Repetition | 9.3 |
| `constant-argument` | B | Overexposure | 5.7 |
| `hidden-coupling` | B | Information Leakage | 5.2 |
| `vague-name` | B | Vague Name | 14.3 |
| `result-without-return` | B | Vague Name | 14.3 |
| `tuple-return` | B | Nonobvious Code | 18.2 |

The book's other red flags need judgment, so the skills check them by reading the code: Temporal Decomposition, Special-General Mixture, Conjoined Methods, Hard to Pick Name and Hard to Describe.

Every finding carries a `weight`: the number of commits that touched its file in the last twelve months. Heavy findings come first.

### In React code

A component counts as called wherever it appears in JSX, and each prop is a named argument. So the detectors that count callers work on components too, and props match by name whatever order you write them in:

```tsx
<Badge tone="info" label="Inbox" />
<Badge label="Sent" tone="info" />
<Badge tone="info" label="Drafts" />
```

`software-design check --tier B` reports, among other fields:

```json
{ "rule": "constant-argument", "path": "src/Badge.tsx", "line": 1,
  "message": "All 3 calls to `Badge` pass `\"info\"` as `tone`." }
```

A plain lowercase tag such as `<div>` is a DOM element, not a call; `<Icons.spinner>` is a component. A component made with `forwardRef` or `memo` takes the name of its `const`.

## Command line

The binary works on its own too, for example in CI:

```
software-design check [--tier A|B] [--diff | --base <rev>] [--since <date>] [--strict] [PATH...]
software-design interface PATH...
```

`check` prints JSON. `--strict` exits with status 1 when there are findings. `check` skips generated files, vendored code and test fixtures.

```console
$ software-design check src
{
  "findings": [
    {
      "evidence": {
        "params": ["text", "at"],
        "target": "this.area.insert"
      },
      "flags": "pass-through-method",
      "line": 6,
      "message": "`insert` only forwards its arguments to `this.area.insert`.",
      "path": "src/editor.ts",
      "rule": "pass-through-method",
      "symbol": "insert",
      "tier": "A",
      "weight": 0
    }
  ],
  "root": "/home/you/project",
  "schema_version": 2
}
```

## How it works

```text
  your agent edits a file
          │
          ▼
  hook ──► scripts/software-design ──► ~/.software-design/bin/software-design
                                              │
                    tree-sitter parse ◄───────┤
                    git churn         ◄───────┤
                    rules.json        ◄───────┘
                                              │
          ┌───────────────────────────────────┘
          ▼
  red flags in the lines just written ──► back to the agent, before the turn ends
```

One Rust binary does all the analysis. The hooks, the MCP server and the command line all call the same analysis code, so they report the same findings. A small POSIX shell launcher picks the build that matches the installed plugin version and builds it when it is missing. A hook never fails because the binary isn't there yet.

## Limits

- The detectors match calls to definitions by name. A name defined twice is ambiguous, so the detectors that count callers skip it. That is why those detectors are tier B.
- In a Vue component, the plugin analyses only the `<script>` block.
- The launcher is a POSIX shell script, so Windows needs WSL.

## Development

```
cargo test --workspace               # unit, fixture, CLI, hook, MCP, launcher and consistency tests
cargo build --release                # a source checkout's launcher uses target/release first
sh scripts/software-design install   # what users get: build into ~/.software-design
claude --plugin-dir .                # try the plugin from this checkout
claude plugin eval . --scaffold --trust-plugin --mocks off \
  --allow-tools "mcp__plugin_software-design_software-design__*"
```

The workspace follows [matklad's flat layout](https://matklad.github.io/2021/08/22/large-rust-workspaces.html):

| Crate | Owns |
|---|---|
| `crates/syntax` | Parsing with tree-sitter; the only code that knows grammar node names. |
| `crates/history` | Everything learned from git: files, churn, co-change, added lines. |
| `crates/rules` | The detectors and `rules.json`. Sees facts, never syntax trees. |
| `crates/software-design` | The binary: CLI, hooks and MCP server, all through one analysis module. Its tests also cover the launcher and repository-wide consistency. |

Each detector has a positive and a negative example per language in `crates/rules/tests/fixtures`. The negative examples encode the exemptions the book gives.

The host adapters are thin. `.claude-plugin/` and `.codex-plugin/` are manifests over the same `skills/` and `hooks/hooks.json`. `pi/index.ts` maps pi's events onto the same hook protocol.

### Releasing

Bump the version in four files, then push: `crates/software-design/Cargo.toml`, `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json` and `package.json`. A test fails if they differ. Each user's next session rebuilds.

### Uninstalling

Removing the plugin leaves `~/.software-design` behind. Delete it with `rm -rf ~/.software-design`.

## Credits

The ideas come from *A Philosophy of Software Design* by [John Ousterhout](https://web.stanford.edu/~ouster/cgi-bin/home.php), a professor of computer science at Stanford. **This project is not affiliated with the author or publisher.**

The rules paraphrase the book. The plugin can flag a problem, but the book explains why it is one and how to think about the tradeoffs. It is short and worth reading, and if this plugin helps you, please buy a copy:

- [Buy the book on Amazon](https://www.amazon.com/dp/173210221X): second edition, in paperback and e-book.
- [The book's page](https://web.stanford.edu/~ouster/cgi-bin/book.php) on the author's website.
- [What's new in the second edition](https://web.stanford.edu/~ouster/cgi-bin/aposd2ndEdExtract.pdf): a free PDF of the new material, for owners of the first edition.

The section numbers in [The rules](#the-rules) point to the first edition, v1.01. The second edition, from July 2021, adds a chapter ("Decide What Matters") and reworks Chapter 6, so some numbers may differ.

## License

MIT © Ismael Serratos
