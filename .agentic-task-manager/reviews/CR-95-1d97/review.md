+++
id = "01a1faf1-e6f1-75ce-b1eb-8588def11d97"
number = 95
title = "One rule for a met dependency (yours), live claims and a done task stage"
status = "approved"
tasks = ["ATM-667"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T21:51:59Z
updated = 2026-11-20T22:39:20Z
+++

Audit findings L4, L7, L10 and L11: one rule each, where two or three disagreed.

**L4, a met dependency.** This is your rule: a skipped dependency is met only when it was skipped as a duplicate (`duplicate_of`) of a task that is done.
- **The rule:** `rules::dependency_met(kind, dependency, duplicate_done)`, used for this project's tasks, its archived ones and another project's (`foreign::status`).
- **The archive check:** it now returns the archived task (`rules::ArchivedTask`, an `Fn(&str) -> Result<Option<Task>>`), not a yes or no. Before, an archived dependency was met whatever it was, and a live skipped one never was.
- **An archived task:** it was finished, so it counts as done, whatever status an older vocabulary gave it, unless it was skipped. Then the rule applies.
- **A duplicate:** done here, in the archive (finished as itself, not skipped) or in another project.
- *Not done as the task says:* it asks for a `Dependencies { tasks, archive }` type. I changed what the existing closures return instead. Their nine signatures now name one type alias, and the four identical constructors share `query::archived_task`.

**L7, live claims.** `atm project status` and the dashboard's Next up handed stored claims to the loop's rule, expired ones included, while `loop next` drops those first. Now there is one read-only `leases::live_claims(store, now)`; the review code's `holder` check moved to `leases` to be shared.

**L10, `todo_command`.** It now uses the project's vocabulary, not the default one, and live claims, so an expired claim no longer reads as held.

**L11, a done task's stage.** `releases::stage_of` is deleted, and `release list` uses `query::task_stage`, as the task views do. A behaviour change: a task with `merged_at` and no PR, which a direct-workflow push records, now counts as merged in `release list`, as the dashboard already showed it. The test said "merged needs a PR", so I changed that expectation.

**Tests**
- atm-common, `a_skipped_dependency_is_met_only_as_a_done_tasks_duplicate`. It covers:
  - live and archived tasks;
  - a duplicate that is done, to do, or itself skipped;
  - skipped with no duplicate;
  - an archived task with a status from an older vocabulary.
- `next.json`: with another agent's claim expired, `project status` names T-1, as `loop next` would. The old code named T-2.
- Passing: atm-common's and atm-lib's tests, the CLI suite and clippy.

**Worth a look**
- A project with a skipped dependency that isn't a done task's duplicate now waits on it, in the archive too. Its dependents need it removed, or the dependency marked as a duplicate.
