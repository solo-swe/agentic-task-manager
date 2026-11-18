+++
id = "01a1ecb6-c873-7054-ae57-2f565047b847"
number = 5
title = "The review watch tells decisions too"
status = "approved"
tasks = ["ATM-600"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T03:32:44Z
updated = 2026-11-18T05:54:37Z
+++

**The bug:** `atm review watch` printed each comment and reply about the agent's work, but not a decision. An agent relying on its one watch didn't learn that its review was approved, sent back or closed until it asked another way.

**The fix** (`crates/atm-lib/src/review_listen.rs`):
- **What's told:** the watch remembers each review's status, starting from what's there when it starts, and prints a line when a review turns approved, changes requested or closed. A restart tells nothing again, and a round that reopens a review isn't a decision.
- **Whose:** only reviews of the agent's work, its own or one naming a task it has held.
- **The line:** `decision` instead of `body`, with `review`, `task` (or `null`), `at` and `title`. The decision's words, if it had some, come just before as a comment, as before.

**Docs and skills:** `docs/agent/reviews.md` describes the line. `/atm-loop-tasks` (the watch, and step 7) and `/atm-do-task` say what to do on one: approved or changes requested, `atm loop next` hands the task back; closed, leave the task to the user. This changes skill text agents act on, so the sandbox check (a written skill review, security §3.3) is due before the release PR. The new text only reads the watch's output; it runs nothing new.

**Tests**
- atm-lib: whose a decision is (own review, a review naming the agent's task, another agent's), and what the line holds.
- The CLI suite: the agent's watch hears Hakan's request for changes on a review of its task, once.
