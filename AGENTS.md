# Engineering charter

We ship a plugin that teaches software design. Its own code must set the example.
Write code for humans: simple to read, correct, idiomatic, and safe.

## Keep the solution small

- Follow KISS and YAGNI. Build the smallest complete solution to the task.
- Reuse existing code and platform features before adding dependencies.
- Add abstractions only when they hide a real decision or remove repeated logic.
- Share knowledge and behavior that must stay consistent. Similar syntax alone does not justify a helper.
- Remove dead code and obsolete paths when a change makes them unnecessary.
- Avoid speculative options, configuration, wrappers, and extension points.

## Give each module a clear job

- Follow the single responsibility principle. Group code by what it does and why it changes.
- Keep files small enough to understand in one sitting. Split at responsibility boundaries, not arbitrary line counts.
- Keep related code together. A split should make the code easier to follow.
- Keep implementation details private. Expose the smallest interface callers need.
- Use names that explain the domain and comments that explain decisions or contracts.

## Write correct, idiomatic Rust

- Prefer standard types, borrowing, iterators, and explicit error handling.
- Avoid needless allocation, cloning, conversion, and collection of intermediate values.
- Keep `unsafe` forbidden. Validate external input at the boundary.
- Propagate actionable errors. Handle expected absence without panicking.
- Use `expect` only for a documented invariant under our control, backed by a check.
- Preserve validation and error handling when moving or merging code.
- Consider malformed input, empty input, Unicode, large files, and deep syntax trees where relevant.
- Preserve public APIs, output formats, and stored data unless the task calls for a change.

## Respect the crate boundaries

- `crates/syntax`: extract language-neutral facts; keep grammar details here.
- `crates/history`: read repository files, changes, and history through Git.
- `crates/rules`: evaluate facts and report evidence; keep detector metadata in `rules.json`.
- `crates/software-design`: adapt shared analysis to the CLI, hooks, and MCP.
- Keep front ends consistent by routing them through shared analysis.
- Hooks must tolerate analysis and persistence failures without breaking the user's session.

## Make changes complete and reviewable

- Read the affected code and its callers before editing.
- Trace changes through tests, fixtures, configuration, exports, and documentation.
- Fix root causes in shared code. Avoid caller-specific patches for a shared defect.
- Keep refactors focused and preserve behavior. Add a regression test for a bug fix.
- Test meaningful behavior and edge cases. Avoid tests that merely repeat the implementation.
- Do not weaken checks or add suppressions just to make a change pass.
- Report what changed, what passed, and any checks skipped or remaining risk.

Run these checks for Rust changes:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
