+++
id = "01a1fe89-7ec5-7096-9536-a983c150483e"
number = 108
title = "State machine, part 3: the composed table test, and the two bugs it found"
status = "approved"
tasks = ["ATM-713"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T14:36:26Z
updated = 2026-11-21T15:03:11Z
+++

The state machine, part 3 (ATM-633's build, §7 step 2): the composed table test. On its first run it found two bugs, both fixed here.

**The table** (`crates/atm-lib/tests/reviews/table.rs`, an integration test: it needs the real review store and tasks, so it lives in the crate's `tests/` as the Rust guide asks, not in `src/lifecycle/tests.rs` as the task first said):
- **Seven scenes:** a review and its task in each state they can be in together:
  - draft;
  - waiting;
  - sent back;
  - sent back and taken back to fix;
  - taken back, then lapsed;
  - approved;
  - closed.

  Each scene is built by the events that lead there: Hakan's decisions, the agent's start, a real lapse (no state is written into a file).
- **Six events:** the author's round, and Hakan's approve, request changes, comment, close, and close rejecting its tasks.
- **42 rows:** each says what `lifecycle::check` answers (the review's next status, or the refusal's words). For an allowed event, the review and the task are checked after the real command has run. For a refused one, the test checks that nothing was written.
- A scene and event without a row, or with two, fails the test. The rows are all checked, and every difference is listed at once.
- **The three bugs** as rows:
  - ATM-632's class is `(Lapsed, Round)`: the task is taken back and goes in review.
  - ATM-630: the approval still reaches a task whose number a merge moved (its old key an alias).
  - ATM-631: opening a review takes back the author's lapsed task.

**Found and fixed:**
1. **A round on a draft put its task in review** while the review stayed a draft. Now a draft's round moves no task, in `check` and in `add_round`.
2. **A round on a review sent back left its task marked sent back** while the review waited again (the design's invariant 3). This happened when the task wasn't taken back first, for example a person's task.
   - The task table now has §3.2's move, changes requested → waiting by a round: `review` from `review`, only when changes were requested.
   - The last round is kept in `reviews` with its outcome, `reviewed_at` is cleared, and `review_state` goes back to waiting.
   - From waiting, approved or rejected it is refused, naming why.
   - `ask_review` (opening, rounds, added tasks) takes such a task too.
   - `docs/task-states.md` says so, and the task table's own table test has the row, plus a test of its own.

**What the table records as it is, for you to check:**
- **Taken back to fix, then approved after all:** the review is approved, while the task stays in progress (it isn't in review, so the decision waits). Its agent finishes it with a round or done. Is that what you want, or should approving put it in review, approved?
- **Closed without rejecting:** the task is left as it was (still in review, waiting). This is §4.2's "unlinked, the task as it was", but a task in review with no review waiting looks stuck. Back to active, as for question 4?

clippy, atm-lib's tests (276 unit, 37 integration) and the CLI suite (858) pass.
