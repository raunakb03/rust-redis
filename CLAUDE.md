# CLAUDE.md

This is a Redis clone in Rust for the CodeCrafters "Build your own Redis" challenge.
The owner is learning Rust through this project.

## Issue tracker: ISSUES.md

`ISSUES.md` is how the project owner and AI assistants communicate about known problems.

- **Read `ISSUES.md` at the start of any task** that involves reviewing, debugging or changing code.
- **Keep it up to date** by following the "Instructions for AI assistants" section in that file:
  - add new issues you find, without creating duplicates;
  - mark issues that have been fixed and move them to Resolved;
  - update the "Last reviewed" line after a review pass.
- **Only fix issues when the owner asks.** Being listed in `ISSUES.md` is not a request to fix it.
- If you change code that affects a listed issue, update that entry in the same change.

## README.md

When a change adds or alters a command, or a new stage passes, update the **Status** section
of `README.md` (the stages line and the supported-commands table) in the same change.

## Working on this project

- Build: `cargo build`
- Lint: `cargo clippy --all-targets`
- Local stage tests: `./test [N]` (runs the first N stages)
- Submit: `codecrafters submit`
- Source layout: `src/main.rs` (TCP server and connection loop), `src/parser.rs` (RESP parser and `RespValue::encode`),
  `src/executor.rs` (command handling), `src/data_manager.rs` (key-value store with expiry).
