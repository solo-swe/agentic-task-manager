+++
id = "01a1f0d7-b39d-7562-8d26-4c747f9de9ee"
number = 32
title = "Opening a review moves its task into review, or refuses"
status = "approved"
tasks = ["ATM-631"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T22:47:10Z
updated = 2026-11-19T06:04:31Z
+++

The CompMD bug: an agent's claim lapsed while its checks ran, and then `atm review create --task` opened CR-6 beside a task that stayed in to do. There were two ways in:
- the lapse had already put the task back in to do, so `create` skipped it silently (it only moved tasks in progress);
- or the lease expired during the command, and moving the task failed *after* the review was written.

## Change (`atm-lib`'s `reviews::create`)
When an **agent** opens a review that waits on its reviewers, every task is checked before anything is written. The command either opens the review with every task in review, or refuses, writes nothing, and says what to do instead:

| The task | What happens |
|---|---|
| in progress, held by the agent or by no one | into review (as before) |
| the agent's own claim lapsed (back in to do, or about to be) | taken back, then into review; its note says "its claim had lapsed: taken back to open this review" |
| in review, no other review waiting on it | linked |
| in review, another review waiting or changes requested | refused: add a round to that review |
| claimed by another agent | refused, exit code 4 |
| to do, not started by this agent; blocked; done; skipped | refused, with the command to run |

## One choice to check: a person's review
A **person** opening a review keeps today's behaviour: tasks in progress go into review, others are linked as they are. The sandbox's red-team seed does exactly that, a reviewer's comments on files for a task an agent hasn't started yet, and the strict rule broke it. Should a person's review be stricter too? If so, that belongs in the state machine (ATM-633).

## Tests
- Unit: one row per task state, a task taken back, in review with or without another review, one refusal moving none of the other tasks, a person's review, and a draft.
- CLI: reproduces the CompMD case with a lease of under a second, plus refusals that leave no review behind.
- clippy, `cargo test` and the CLI suite pass.
- `atm docs reviews` states the rule.
