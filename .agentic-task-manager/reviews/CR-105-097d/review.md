+++
id = "01a1fb80-df8f-7419-9bb4-37fcdce9097d"
number = 105
title = "The state machine, part 1: the review table, lifecycle::check and can, the page reading can"
status = "approved"
tasks = ["ATM-633"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T00:28:09Z
updated = 2026-11-21T13:50:39Z
+++

The state machine's build, the first part, as §5.4 and §7 of the design you approved (CR-58).

**Done:**
1. **The review table, pure** (atm-review `rules.rs`):
   - `review_next(review, event, by)` returns the next status, or the refusal in the words and exit code each command has always used. `ReviewEvent` covers ask for review, round, checkpoint, submit, close, set tasks and edit summary.
   - `Reviews::{ask_for_review, add_round, checkpoint, submit, check_submit, close, set_tasks, edit_summary}` call it before writing; their old checks are gone.
   - The table test has one row per status × event (54), and fails on a pair without a row. A second test checks the table writes nothing.
2. **`atm-lib::lifecycle`** (new):
   - `check(project, review, event, by) -> Outcome` composes the review table with the task table. It finds every task the event moves (ATM-594) and checks each through `state::change_status` on a copy (ATM-670); the first refusal wins and nothing is written.
   - `can(project, review, by, drafts)` asks it for each action.
   - `reviews::{submit, close, add_round}` now call `check` first, then write.
   - One change in behaviour: `close` with reject now checks its tasks before closing, where before it closed and then failed on a task.
   - Tests: `tests/reviews/lifecycle.rs` has three tests on the real store.
3. **`can` on the review page** (§5.2):
   - `GET …/reviews/<CR>`, and the app's command, answer `can: {approve, request_changes, comment, round, close}`, each `{ok}` or `{ok: false, why}`, with the reader's drafts counted as `submit` counts them.
   - The page shows Approve and Request changes from it, with its `why`.
   - Tests: an atm-lib test on a live review, and the e2e test §7 asks for (approve blocked, then allowed).

**Not done yet**, in this task:
- **The other entry points** going through `lifecycle`: `create` (with ATM-631's openings), `set_tasks`, the task commands (`change_task_status`), `apply_live_approval` and `drop_expired_claims`.
- **The composed table test** with §4's rows and the three bugs as cases.
- **The copies removed** (step 3): the page still falls back to its own `approveBlocked` and `sentBack` when a page has no `can` (the e2e tests' stand-in server sends none yet), and `REWORK_STATES` and the open-status lists are still there.
- **The agent docs** checked against the table (step 5).

Also here, found on the way: the app's file and document commands still read `at=HEAD`, though the REST routes refuse it since ATM-671. The rule is now one function in atm-lib that both use (committed under ATM-671).

To check: whether `close` refusing up front, rather than closing then failing, is what you want.
