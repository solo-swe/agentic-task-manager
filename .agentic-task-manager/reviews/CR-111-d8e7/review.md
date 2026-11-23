+++
id = "01a1ff18-69c7-7052-a3b5-4ad3fc11d8e7"
number = 111
title = "Split the large files, part 5: the app’s commands.rs and lib.rs"
status = "approved"
tasks = ["ATM-711"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T17:12:32Z
updated = 2026-11-21T19:50:04Z
+++

The split, part 5, the last: the app's `commands.rs` and `lib.rs`, as planned on CR-106. Each is a move only, with the same tests passing.

**`commands.rs`:**
- It is split by area:
  - `commands/state.rs`: `AppState`, `AppError`, the filters, the answers;
  - `commands/reads.rs`: the workspace, project, task, document and file reads, with their plain functions;
  - `commands/reviews.rs`: the review and draft commands;
  - `commands/windows.rs`: the subscriptions, the links, and the agents' skills.
- `commands.rs` re-exports each, so every command keeps its path, and Tauri's and tauri-specta's macros find them as before.
- A few items became `pub(super)` because they are now used across files: `AppState`'s `workspaces` and `links`, `watch` and `unwatch`, `JsonShape`, `workspace`, `member` and `review_ref`.
- The unit tests are in `commands/tests.rs`. The integration test is in `commands/integration_tests.rs`, not the crate's `tests/`, because it reads `AppState`'s own fields.

**`lib.rs`:**
- The code stays (244 lines).
- Its tests are in `tests.rs`, and its integration tests in `integration_tests.rs`. They stay in the crate, as their header says, because the commands' handler macros and the app's context are the crate's own.

**How it was moved:**
- `csplit` cut the files at their line numbers.
- The two parts of `reads.rs` were joined with `dd`, which copies bytes exactly. No text was retyped.

**Checked:**
- `cargo test -p atm-app`: the same 119 tests pass.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt` pass.

**A note:** the disk filled up during the build. I removed `target/debug/incremental` (cargo's incremental cache, rebuilt as needed), which freed 14 GB.
