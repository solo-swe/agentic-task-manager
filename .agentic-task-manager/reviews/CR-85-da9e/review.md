+++
id = "01a1f9fc-900e-777f-89ab-b2c8b1ddda9e"
number = 85
title = "loop next hands a task back from review to its agent"
status = "approved"
tasks = ["ATM-657"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T17:24:01Z
updated = 2026-11-20T22:32:51Z
+++

After a review decision, `atm loop next` never handed the task back. The decision (`approve`, `request-changes`) writes a `waiting` record for the task, and the loop skipped every task with such a record, its own worktree's too. The reason it gave was wrong as well: "blocked or in review in another worktree", about the worktree it was running in.

**Change**
- `loop_order` and `next_task` (`loop_rules.rs`) take `here`, the worktree they run in. A `waiting` record from that worktree no longer holds a task back, and the reason names only the tasks waiting elsewhere, as `atm project status` does.
  - The callers pass their worktree: `loop next` (`changes.rs`), `project status`'s next task (`status.rs`) and the dashboard's Next up (`summary.rs`).
- Decisions still record the wait. Another worktree's copy doesn't know about the review, and the record keeps that worktree's loop off the task.
- One `record_start` (`changes.rs`) now handles both ways a task starts, `task state in_progress` and `loop next --claim`. Each one records the claim and clears the task's `completed` and `waiting` records. The loop's claim used to leave both behind.

**Tests**
- `review-states.json`: two scenarios that go through the real commands. They start, ask for a review, request changes (or approve), then `loop next --claim` hands the task back to its agent. Both failed before the fix with "blocked or in review in another worktree: T-1".
- A `loop_rules` unit test: records in this worktree, in another, and without a worktree, looked at from here and from outside git. Also the reason when the rest waits elsewhere.
- Passing: atm-lib's 290 tests, the CLI suite and clippy.

**Worth a look:** a record made outside git has no worktree, and from inside git it counts as another worktree's. Outside git, every record is the loop's own.

ATM-633 (the state machine) is still at its design (CR-58), so it has no code yet to replace this. This bug belongs in its table of cases.
