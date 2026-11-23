+++
id = "01a1f314-9d5d-75ea-b4d4-1165e7bd350b"
number = 58
title = "One state machine for tasks and their reviews: the design"
status = "approved"
tasks = ["ATM-633"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T09:12:56Z
updated = 2026-11-20T23:11:28Z
+++

The design for ATM-633: one state machine for tasks and their code reviews, including live reviews. For review before anything is built.

## What's in it
- **The state, in three parts** (§2): a task's status kind with its decision, its claim, and its code reviews. Each part has its own diagram (§3):
  - the status kinds;
  - the decision inside review;
  - the claim (held, lapsed, taken back);
  - a code review's statuses;
  - a live review.
- **How they move together** (§4):
  - six invariants, for example "a task in review has an open static review" and "a task's decision is its latest code review's";
  - every event, with who, what it does to the review and to each task, and its refusals;
  - ATM-630, ATM-631 and ATM-632's class as rows of the table.
- **Where it lives** (§5):
  - one new module, `atm-lib::lifecycle`, composing the task table (`state.rs`, kept) with atm-review's review table (`rules.rs`, kept, since that crate can stand alone);
  - every front end calls it;
  - the dashboard asks it (a `can` on the review page) instead of repeating the rules;
  - review events check everything before writing, write the review then the tasks, and reconcile from the review if the second write fails.
- **Today, mapped** (§6): where each rule is decided now, the copies (three `REWORK_STATES`, five lists of open statuses, two "who needs to act"), and five holes the table closes. Two holes are real today:
  - a round after a lapsed claim leaves the review waiting on a task in to do;
  - `atm task review approve` decides a task's review around its open CR (its author can approve their own work that way).
- **The build** (§7), and **open questions** (§8).

## For you to decide (§8)
1. atm-review keeps its own table, or everything moves into atm-lib?
2. `atm task review approve` on a task with an open CR: through the CR, or refused?
3. A draft is asked for review by `atm review round`, or a new `atm review ready`?
4. A task removed from the review it was in review for: back to active, or left in review?
5. Split ATM-633 into this design and build tasks?

Rendered in the review page's Rendered view (the Mermaid diagrams included).
