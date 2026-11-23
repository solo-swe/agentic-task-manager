+++
id = "01a1f18b-b79e-70b2-8f8e-769b5b6c632d"
number = 45
title = "A number clash moves the task not on the base branch"
status = "approved"
tasks = ["ATM-636"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T02:03:47Z
updated = 2026-11-19T06:01:31Z
+++

The merge driver's rule for a number clash, as ADR-009 decides it. A clash never stops a merge.

## Change
- **`renumber_clashes`** (atm-db `merge.rs`): of two different tasks with one number, one takes the next free number and keeps its old key as an alias:
  - the task **not on the base branch**;
  - when both or neither are, the **newer** one, by its UUIDv7's time;
  - when that can't be told (version 8 UUIDs, migrated from IDs), theirs.

  The "two final numbers" conflict is gone.
- **Which tasks are on the base branch:** read from git by the driver. `merge_versions` in atm-lib reads `origin/<base>`'s task file and archive, where `<base>` is `meta.workflow.base`, else the remote's default branch. It passes them to atm-db's new `merge_files_with` / `merge_task_files_with`. Without a remote, no task is on it, so the newer moves. The unit API (`merge_task_files`) is the same with none on base.
- **Said once:** what moved is kept in `meta.renumbered` (`[{from, to}]`). The next `atm db migrate` prints "ATM-538 is now ATM-541 (both branches made ATM-538); ATM-538 still finds it." and drops it. It reports `renumbered` in `--json`, and a task file it can't read is skipped, as the other migrations do.
- **`atm docs commands`** says which task moves and where atm says so.

## One thing to check
The task asked that `atm task details ATM-538` find the moved task by its alias. While the other task holds 538, the key `ATM-538` names that one: a current key wins over an alias (ADR-007 §5, unchanged). The alias finds the moved task once no task holds the number. The migrate line says this. Say if you want a key that moved to say so when it's looked up.

## Tests
- **atm-db unit:**
  - not on base moves, either way round, however new;
  - both on base: the newer, whichever side;
  - an old `key_final` changes nothing;
  - version 8 UUIDs: theirs;
  - several clashes in one merge each get their own number, all recorded;
  - the record is said once by migrate.
- **CLI with real git:**
  - no remote: the newer (main's) moves;
  - with `origin`: main merged into the branch moves the branch's older task, migrate says so once, and merging back changes nothing more.
- **Suites:** clippy, `cargo test`, and the CLI suite's scenarios and server tests (526) pass.
