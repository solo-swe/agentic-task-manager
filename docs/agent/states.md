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
- `in_review --note '<what the reviewer should check>'`: in review, when the task's review mode needs one
  (`blocker`: reviewed before anything goes on; `async`: reviewed while agents go on; `batch`: in the end-of-sprint
  review, so it can be done; `none`). **An agent's review is a code review**: its review puts the task in review
  (`atm review create --task <key> …`, `atm review round CR-N`; `atm docs reviews`), and atm refuses an agent's
  `in_review` with no review the task waits on (waiting, live or sent back), as the dashboard decides a task only
  through its review. Work with no change to review is blocked on the user instead. A person may still ask for a
  review this way and decide it themselves; `atm doctor` names a task in review that no review waits on.
- `blocked --on user|agent --question '…' [--question '…']`: it can't go on without a decision. Whoever decides
  answers with `atm task state <id> todo --answer '…'` (one per question).
- `todo`: back to to do: `--reason '…'` when it failed, `--note '<handoff>'` when you stop early; a done or skipped
  task back with `--reason '…'`.
- `skipped --reason '…' [--resolution wont_do|obsolete|duplicate]`: not needed, from any open state (from review once
  rejected).

## The table
Each move, by the kind a task is in before it and the kind it leaves it in (atm's own table; a test checks every row,
so this says what atm does). Run: `start` is `in_progress`; `review` is `in_review`; `fail` is `todo --reason`;
`release` is `todo --note`; `unblock` is `todo --answer`; `reopen` is `todo` from done or skipped; `drop` is `todo`
from a rejected review; `approve`, `request-changes` and `reject` are the user's `atm task review` decisions.

| Move | From | To |
|---|---|---|
| start | todo, review | active |
| review | active, review | review |
| done | active, review | done |
| fail | active | todo |
| release | active | todo |
| approve | review | review |
| request-changes | review | review |
| reject | review | review |
| drop | review | todo |
| block | todo, active | blocked |
| unblock | blocked | todo |
| reopen | done, skipped | todo |
| skip | todo, active, review, blocked | skipped |

From review, `start` needs changes asked for or an approval (rework), `review` changes asked for (a round), `done` an
approval, and `skip` or `drop` a rejection.

## Reviews
A review's decision is the user's: `atm task review approve <id> [--note]`, `request-changes <id> --reason '…'` or
`reject <id> --reason '…'`. The task stays in review, and atm lets it out only by the decision: approved, `done` (or
`in_progress` for what is left); changes requested, `in_progress`; rejected, `todo` (its work dropped) or `skipped`.
`atm loop next` hands a task back from review (approved, or with changes asked for) before new ones: read its latest
review note first.

## Order
Agents take tasks sprint by sprint, then by priority (`P0` … `P3`; none counts as `P2`), then by ID. A task done
together with the user (`with: user`) is taken only with `--with-user`.
