+++
id = "01a1f2d3-e127-739a-97dd-087e520440a8"
number = 55
title = "A review finds its task by the key it was written with"
status = "approved"
tasks = ["ATM-630"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T08:02:14Z
updated = 2026-11-19T08:57:37Z
+++

A review finds its task by the key it was written with, including in the case round 1 got wrong (c1): a real clash, where the old key is now another task's.

## Round 2: an ambiguous key, told apart by the review's own commit
After a clash (ADR-009 §3), the moved task has its old key as an alias, and the task that kept the number holds that key now. A review written with that key is ambiguous. Round 1 resolved it to the holder now, the wrong task.

**Now** (`atm-lib` `review_keys::task_of`), for each key a review stores:
- **Not ambiguous:** no other task has the key as an alias. It resolves as every command does, with no git read.
- **Ambiguous:** atm reads `tasks.json` at the review's own commit and takes the task that held that number then, if it's one of the two. The commit is:
  1. round 1's: a range's head, or a working round's base, unless it's a submodule's;
  2. else the commit that first added its `review.md` (`atm-vcs` `first_commit_adding`).
- **Nothing readable** (a review never committed, a commit gone after a squash): today's rule, the holder now.
- Nothing is rewritten.

**Used everywhere a review's keys meet tasks:**
- `review list --task` and the reviews page's filter (read from the working copy each review was found in);
- the watch and `watch_state` (`keyed_now`);
- an approval, a request for changes, a close with reject, a new round (`named`);
- the approved live review that covers a task (`live_approval_for`);
- removing a task from a review (`set_tasks`), by the task the review meant.

**A round's commit is data** anyone who can change the review wrote. Only a full SHA reaches git. `HEAD`, a branch name or `--output=x` are ignored and the key resolves as usual. There's a test for it and a note in the security doc's 0.41.0 section.

## Tests
- **CLI** (`reviews/keys.test.mjs`), a real clash through atm's merge driver:
  - Setup: ATM-12 on branch `a` and ATM-12 on `main` (another agent's), each with its review, then `main` merged into `a`.
  - Played twice:
    1. with a remote, `main` pushed: the task not on it moves;
    2. with no remote: the newer task moves.
  - Each time:
    - the moved task is ATM-13, and both reviews still say `ATM-12`;
    - `--task ATM-13` lists only the moved task's review, `--task ATM-12` only the other's;
    - a comment reaches the moved task's agent, about ATM-13;
    - the approval reaches ATM-13 and not ATM-12.
  - Both fail with the disambiguation turned off.
- **Unit:**
  - `review_keys.rs`:
    - a files review, read through its `review.md`'s commit, gives the moved task;
    - one never committed gives the holder now;
    - without the folder, the holder now;
    - keys that aren't ambiguous, any case, `~`;
    - a commit where a third task held the number gives the holder now;
    - a round naming `HEAD`, `main` or `--output=x` is never read, while its SHA is.
  - `atm-common` `alias_holders`; `atm-vcs` `first_commit_adding`.
- **Suites:** the whole CLI suite, the touched crates' tests, clippy and fmt.

ADR-009 §3 now says how such a review keeps its task.
