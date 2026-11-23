+++
id = "01a1fe5e-302b-72a0-ba8d-14d5ef24dbb7"
number = 107
title = "State machine, part 2: opening, rounds, added tasks and live approvals through lifecycle"
status = "approved"
tasks = ["ATM-712"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T13:49:08Z
updated = 2026-11-21T14:08:58Z
+++

The state machine, part 2 (ATM-633's build, §5.3 and §7 step 1): the other review entry points check through `lifecycle` first, then write. It builds on part 1 (CR-105).

**Done:**
- **Opening a review** (`reviews::create`): `lifecycle::check_open` decides each task's move, using ATM-631's openings. It also tries each move into review through the task table, on a copy, before anything is written. So a task the task table refuses no longer leaves a review opened beside it. The behaviour is otherwise the same.
- **A round** (`add_round`): this closes **hole 1** (§6.3, ATM-632's class).
  - Before, a round on a review whose task its lapse had sent back to to do was kept, and the task stayed in to do.
  - Now `check(Round)` names the author's own tasks in to do (`Outcome::take_back`). They are started again, then go in review with the round, their note saying "its claim had lapsed: taken back for this round".
  - Every move is tried before the round is kept.
  - Only the round's author takes back their own tasks. Another agent's round is refused by the review's rules, as before.
- **Naming more tasks** (`set_tasks`): `lifecycle::check_set_tasks` runs the review's rules first. It then checks the tasks added to an agent's waiting review as opening one does: blocked, finished, another agent's, or in another open review are refused, and nothing is written. Before, they were linked silently.
- **A live approval** (`apply_live_approval`): its two moves, into review and then approved, are both tried before either is written (`lifecycle::check_live_approval`). A task can no longer be left in review without its approval.
- **A lapse** (`drop_expired_claims`): the transition (back to to do, the "lease expired" note) is now `lifecycle::lapse`, the one place it is written. The behaviour is the same.
- **Agent docs:** `docs/agent/reviews.md` says what a round does with a lapsed task, and that `atm review tasks --add` checks as opening does.

**Tests** (`crates/atm-lib/tests/reviews/lifecycle.rs`, on the real store):
- a round takes the author's lapsed task back, and `check` shows it while writing nothing;
- a round takes back only the author's own task;
- an added task that can't go in review is refused, writing nothing, while one that can goes in review;
- a live approval that can't reach its task writes nothing.

The rest pass: atm-lib's tests, clippy and the CLI suite.

**Not in this part, and why:**
- **The task commands** (`atm task review approve|request-changes|reject` on a task with an open CR, the design's hole 2): this is §8's question 2, which the review of the design didn't answer. My pick there was "go through the CR"; the other choice was "refuse and point to `atm review submit`". **Which do you want?** Until then they work as before.
- **Removing a task** from the review it was in review for (§8's question 4): it stays as it is now (unlinked, with a note, its status unchanged). Back to active needs a new task-table move. Same question: back to active (my pick), or left in review?
