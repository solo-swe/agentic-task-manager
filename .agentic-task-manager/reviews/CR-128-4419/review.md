+++
id = "01a20825-847b-77b7-b06d-1712b0734419"
number = 128
title = "atm-lib: one helper for each repetition the audit found"
status = "approved"
tasks = ["ATM-681"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T11:23:26Z
updated = 2026-11-23T15:55:33Z
+++

The audit's repetition in atm-lib (docs/codebase-audit-2026-11.md §2), one helper per commit. No behaviour changes: the existing tests are the check, with a test for each new rule.

> [!NOTE]
> The range also holds other tasks' commits, each with its own review: ATM-699 (CR-124), ATM-727 (CR-125), ATM-701 (CR-126), ATM-722 (CR-127), and the archive chore. ATM-681's are the twelve below.

## The commits
1. `9edf734` `Project::change<T>` returns what the change returned, replacing the `let mut outcome = None` sites.
2. `3150b84` one task lookup (`lookup::task_at`, `no_task`), saying when the task is archived.
3. `ed0d33a` one expiry step (`leases::expire`) and one claims-store section (`leases::section`).
4. `fe527cf` `atm_common::task_id`, `valid_label` and `in_open_pr`.
5. `910c015` `repository::web_address` and `Vocabulary::find_release`.
6. `26cee81` `Vocabulary::active_sprint`.
7. `26cdcad` `rules::awaits_agent`: a task in review back with its agent.
8. `9a9832a` one change over the tasks of both files (`lookup::in_both`).
9. `bb57019` the review status groups as `Status` methods (`is_decision`, `waits_on_reviewers`, `in_play`).
10. `28e025a` `Vocabulary::sprint_of` and `sprint_id_of`: the one lookup of a task's sprint.
11. `8df8f81` `Project::review_actor`: the actor a review records, four copies (one in the CLI) made one.
12. `083c355` `home::keyed_state_file` and `keyed_file_name`: the watch cursor, the commit cache and the drafts named by one rule.

## Tests
`cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` between the steps; the CLI suite with ATM-722.

## To look at
- Left out: `set_or_remove` and `count_by_kind`. Each had two uses, and a helper would hide more than it saved.
- `keyed_file_name` keeps the names as they were (the first 16 hex digits of the key's SHA-256), so existing watch cursors and drafts are still found.
