+++
id = "01a2083b-1ee3-73a3-acf7-c6c40326adaf"
number = 129
title = "The front ends: one locked(), one dry-run rule, one host check"
status = "approved"
tasks = ["ATM-682"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T11:47:01Z
updated = 2026-11-23T15:56:19Z
+++

The front ends' repetition (audit §3–§5, theme 5), one area per commit, and C17's dry-run rule.

## What changed
- **The app** (`22310de`): one `locked()` for the app's mutexes (13 copies of `lock().unwrap_or_else(PoisonError::into_inner)`); `AppError` from `tauri::Error`, `io::Error` and `serde_json::Error`, so the commands use `?` instead of a `map_err` each. The review commands already shared one `as_user`.
- **The CLI** (`ec42133`), with the one behaviour change:
  - `Context::dry(what)`: says what would be done and whether to stop. The three private copies call it.
  - Audit C17: a dry run comes after the command has checked and found what it changes, so it refuses what the real run would. `task edit` (the description file read, the task and the new references found), `task note`, `task review`, and the review commands `comment`, `poke`, `reply`, `edit`, `resolve`, `submit` and `close` now find the task or review first.
  - `Project::working_task(reference)`: a reference resolved and checked to be a working task (saying when it is archived). In a file from before task keys, `resolve_task` gives the reference back unchecked, so a dry run on an unknown ID said "would edit T-9".
- **The server** (`2ef792e`): `Served::is_own_host`, the one check for the Host header and a change's Origin (DNS rebinding).

## Tests
Scenarios for the dry runs: `task note`, `task edit` and `task review approve` on an unknown task exit 1 with "no task T-9". Clippy, `cargo test --workspace` and the CLI suite.

## To look at
Not done, and worth a task of its own each, as they touch most commands:
- The flattened `As { agent }` (38 fields, 17 calls).
- `trait Shown` with one `show(ctx, &out)` (92 `if ctx.json` branches).
- The server's `Member` held for a request, and `change::<Body, Out>` for the review change handlers.
- The app's one command list generating the manifest and the capability.
- The other dry-run sites (`release`, `component`, `vcs`, `db`, `claim`, `maintenance`) still print their own line; most already check first.
