+++
id = "01a2051a-29d2-752b-9ea0-defa231980b5"
number = 122
title = "State machine, part 5: the agent docs’ state tables, checked against atm’s"
status = "approved"
tasks = ["ATM-715"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T21:12:10Z
updated = 2026-11-23T00:41:21Z
+++

State machine, part 5: the agent docs checked against the table (docs/task-review-state-machine.md §7 step 5).

## What changed
- **`docs/agent/states.md`** (`atm docs states`): a table of every move, the kinds a task may be in for it and the kind it leaves, with how to run each (`start` is `in_progress`, `fail` is `todo --reason`, …) and what review a move from review needs.
- **`docs/agent/reviews.md`** (`atm docs reviews`): a table of how a review's status moves under a round, approve, request changes, comment and close; an approved or closed review takes none.
- **The checks**, beside the tables they check, each reading the doc as atm embeds it:
  - `atm-lib` `state/tests/docs.rs`: every `Command` has its row, with `sources()` and `destination()` as the docs say; no row names a move atm hasn't.
  - `atm-review` `rules/tests/docs.rs`: each documented move is the review table's (`review_rows`), and each move the table allows for those events is documented, a refused one not.
- `docs/security-architecture.md`: the entry.

## Tests
The two checks; a documented row changed on purpose (request changes from `live`) failed with the table's refusal, then put back. Workspace tests and the CLI suite (its docs check: every command a topic names exists) green.

## To look at
- The task named atm-cli's tests; I put the checks where the tables are (atm-lib, atm-review), which atm-cli can't see inside.
- The review table has an `ask for review` event (a draft to waiting) that no command reaches, so the docs don't list it.
- `docs/agent/` changed: the agent sandbox, or the written skill review, before the release PR.
