+++
id = "01a1fa21-3fd4-7720-9bc3-a439332c20d8"
number = 89
title = "Errors typed: Invalid and a new Conflict, so the server answers 400, 404 and 409"
status = "approved"
tasks = ["ATM-664"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T18:04:05Z
updated = 2026-11-20T22:37:30Z
+++

Audit findings C2, C17 and H3. Usage errors and "claimed by another" were built as `Error::Failed { code: 2 }` and `{ code: 4 }`, and the server answers every `Failed` with 500. The tasks route mapped errors its own way, and "no such task" was built with three different kinds.

**Change**
- **`atm-common/error.rs`:**
  - a new `Error::Conflict` (exit code 4, HTTP 409);
  - the one exit-code table, `error::exit`: `OK` 0, `ERROR` 1, `USAGE` 2, `NOTHING` 3, `TIME_UP` 3, `CONFLICT` 4;
  - `Error::code()` maps each kind to the table.
- **The sites:**
  - the 19 `code: 2` errors are now `Error::Invalid`: unknown sprint, release, component or label, a note too long, a wrong `--status`, a logo refused, a registry path;
  - the 4 `code: 4` errors are now `Error::Conflict`: claimed by another, a lease run out, no live claim, a review of a task another holds;
  - "no task …" is `NotFound` in all seven places that built it with `failed`.
- **atm-api:** one `status_of` mapping that every route uses. `routes/tasks.rs` now goes through `error_response` instead of its own `code() == 2` check.
- **The app:** `AppError` has `Conflict`, and the page's IPC maps it to 409, as the server answers.
- **`docs/agent/commands.md`:** the exit codes now say what each covers: 3 includes a wait whose time was up, and 4 includes one's own lease run out.

**Not done as the task says**
- **The wait's exit code:** the task asks for a separate code for a wait's timeout. Agents know it as 3 (`docs/agent/reviews.md`, and the loop skill treats 3 as "nothing"), so it keeps that value under a name of its own, `exit::TIME_UP`. Changing the value would change what agents act on, which is a skill and docs change with the agent sandbox. Say if you want it, say as 5.
- **A `task_at` helper:** the task asks for one beside `Working`. I made every "no task" `NotFound` but left their archive hints where they are; one helper would be a refactor of five call sites, which ATM-681 (the atm-lib refactor) covers.

**Tests**
- `error.rs`: each kind's code and the table.
- atm-api: `a_usage_error_is_400_an_unknown_task_404_and_a_claim_held_by_another_409`, with the errors as atm-common builds them. The shared refusals table has `Conflict`, and the tasks route's test now expects 404 and 409 by kind; it was 500 for an unknown task.
- The app: `AppError`'s conversion and JSON for `conflict`.
- The dashboard: the IPC's `conflict` is 409.
- Passing: `cargo test --workspace` and the CLI suite (every scenario keeps its exit code).

**Worth a look:** the tasks route answers an unknown thing with 404 now, where it said 500.
