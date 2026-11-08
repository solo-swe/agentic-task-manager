# States: a task's statuses, how it moves between them, and reviews

## Statuses and kinds
Every status has one of six kinds: `todo`, `active`, `review`, `blocked`, `done` and `skipped`. A project may name its
own statuses (`meta.statuses`); atm acts on the kind, so "done" means any status of the kind done. When a kind has
several statuses, `--status <id>` picks one; otherwise atm uses the first listed. The defaults: `todo`,
`in_progress`, `in_review`, `blocked`, `done`, `skipped`.

## Moving a task: `atm task state <id> <state>`
- `in_progress`: start it (its dependencies must be done; an archived task counts as done). `atm loop next --claim`
  does this for the next task.
- `done --note '<one-line outcome>'`: finished, once its work is committed.
- `in_review --note '<what the reviewer should check>'`: ask for a review when the task's review mode needs one
  (`blocker`: reviewed before anything goes on; `async`: reviewed while agents go on; `batch`: in the end-of-sprint
  review, so it can be done; `none`).
- `blocked --on user|agent --question '…' [--question '…']`: it can't go on without a decision. Whoever decides
  answers with `atm task state <id> todo --answer '…'` (one per question).
- `todo`: back to to do: `--reason '…'` when it failed, `--note '<handoff>'` when you stop early; a done or skipped
  task back with `--reason '…'`.
- `skipped --reason '…' [--resolution wont_do|obsolete|duplicate]`: not needed, from any open state (from review once
  rejected).

## Reviews
A review's decision is the user's: `atm task review approve <id> [--note]`, `request-changes <id> --reason '…'` or
`reject <id> --reason '…'`. The task stays in review, and atm lets it out only by the decision: approved, `done` (or
`in_progress` for what is left); changes requested, `in_progress`; rejected, `todo` (its work dropped) or `skipped`.
`atm loop next` hands a task back from review (approved, or with changes asked for) before new ones: read its latest
review note first.

## Order
Agents take tasks sprint by sprint, then by priority (`P0` … `P3`; none counts as `P2`), then by ID. A task done
together with the user (`with: user`) is taken only with `--with-user`.
