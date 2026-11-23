+++
id = "01a1f182-93f5-73e0-a940-9dae3046fb47"
number = 44
title = "No provisional keys: plain from creation"
status = "approved"
tasks = ["ATM-635"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T01:53:49Z
updated = 2026-11-19T02:09:36Z
+++

ADR-009 in atm: task keys are plain from creation. No provisional keys, no `~`, no `key_final`.

## Change
- **`atm-common` keys:**
  - `label_of` returns the key as it is; `key_final()` is gone;
  - the new `without_draft_mark`, used by `resolve` itself, so every reference still takes a leading `~` (old notes, commits, review files) and ignores it. A `~` alone is no key.
- **Gone:** `finalize_keys` and its callers.
  - `atm db migrate` no longer makes keys final.
  - `atm vcs pushed --pr N` records the merge and releases, and touches no key.
  - `atm vcs pushed` without `--pr` says "Nothing to record without --pr: a direct push changes no task." It stays because skills installed before still call it. `/atm-push` no longer runs it after a direct push.
- **Model, schemas, `--json`:** `key_final` leaves `task.schema.json` and `api.schema.json`, `atm task add`, the task list's rows, the summary's entries and the foreign-task view. A file that still has it, whatever its value, loads (the type check is gone).
- **Migration:** `atm db migrate` drops `key_final` from `tasks.json` and the archive, once.
  - It says "Task keys are plain now (ADR-009): dropped key_final from N tasks (staged, not committed)."
  - It reports `key_final_dropped` in `--json`.
  - The format-2 migration (`--keys`) no longer writes the field.
- **Not here:**
  - the merge driver still has its two-final-numbers case, which no task reaches any more: with no `key_final`, a clash always moves the incoming task. ATM-636 rewrites that to ADR-009's rule;
  - the dashboard's draft mark is ATM-637, committed next. Until then the dashboard doesn't type-check against the new API types.
- **Docs and skills:** `atm docs tasks` and `atm docs commands` say keys are plain; `atm-push` loses its direct-push step. A skill changed, so the written skill review before the release covers it.

## Tests
- **atm-common:** keys read plain whatever a file says; `~ATM-527`, ` ~atm-527 ` and `~T-527` find their task; `~` alone finds none.
- **atm-db:**
  - the migration drops it from tasks and archive once, says so, and is stable after;
  - `--keys` writes none;
  - an old `key_final: "yes"` loads.
- **atm-lib:** a new task has no `key_final`; the summary carries none.
- **CLI:**
  - `task add` prints `Added ATM-3`;
  - the list is plain and its `--json` has no `key_final`;
  - `~ATM-3` still finds the task;
  - `db migrate` drops an old field once;
  - `vcs pushed` without `--pr` changes nothing;
  - the foreign-task view and the summary have no `key_final`.
- **Suites:** clippy, `cargo test` and the CLI suite pass (its scenarios 476 of 476; only its known TODOs are marked).
