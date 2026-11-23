+++
id = "01a205a4-56e9-71ae-b820-f42c36203346"
number = 123
title = "Stage, release, resolution and reverse links from atm"
status = "approved"
tasks = ["ATM-719"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T23:43:05Z
updated = 2026-11-23T00:41:48Z
+++

Stage, release, resolution and reverse links from atm: the dashboard reads them from `derived` and works none of them out.

## What changed
- **atm-lib**
  - `tasks::Backlinks`: the links pointing at each task (depends on, relates to, duplicates), worked out once over the working tasks. `view_task` (`atm task details`) uses it too, so both read one rule.
  - `query::task_resolution`: a finished task's resolution, with the default filled in.
  - `TaskSprint.closed`: the task's sprint is closed.
  - `task_views` serves `resolution`, `required_by`, `related_by`, `duplicated_by` and `in_closed_sprint`; each is left out when it doesn't apply.
- **Schema** (`TaskDerived`) and `docs/multi-project-api.md`: the five fields.
- **Dashboard**
  - `stageOf` and `effectiveRelease` lose their fallbacks. Archived tasks (the `?archived=true` route) and a worktree's copies (`worktrees.json` views) already come with atm's values.
  - `resolutionOf` and `relationsOf` now read atm's fields.
  - The panel's Required By comes from `required_by`.
  - `scope.ts`'s `leftovers` is gone; the state filters on `in_closed_sprint`.
- **Test fakes** (`views.ts`): `withKinds` also gives a stage and a resolution. The new `withLinks` gives links, release and the closed-sprint flag, and `loadIntoDashboard` applies it.

## Tests
- atm-lib:
  - resolution by kind, with and without its own;
  - links both ways, including a text `related` and its own `related` left out;
  - a closed sprint: open in it versus finished there.
- Dashboard: the panel shows atm's answers when they differ from what the fields would give. The tests of the removed rules now check that the dashboard reads atm's fields.

## To look at
- An archived task's `required_by` and the rest come from the working tasks, as the dashboard did before.
- `relationsOf` keeps the task's own `related` (stored data) and appends atm's `related_by`.
