# Task states and status transitions

This document describes how a task moves through its lifecycle in the `atm` CLI (plugin `atm`). It is based on:

- `atm-cli/atm/cli/main.py` — argument parsing and command dispatch
- `atm-cli/atm/lib/status.py` — status-change orchestration (`change_task_status`)
- `atm-cli/atm/common/model.py` — the transition rules (`change_status`, `STATUS_CHANGES`)
- `atm-cli/atm/common/vocabularies.py` — the default statuses and kinds
- `atm-cli/atm/lib/loop.py` — claims, leases, and cross-worktree resolution

## 1. Status kinds

Every status belongs to one of six **kinds**. Rules are written against kinds, never status names, so a project may rename its statuses in `meta.statuses` as long as each kind is represented (`todo`, `active`, `done` are mandatory).

Default vocabularies (`atm/common/vocabularies.py`):

| kind | default status id | label |
|------|-------------------|-------|
| `todo` | `todo` | To Do |
| `active` | `in_progress` | In Progress |
| `review` | `in_review` | In Review |
| `blocked` | `blocked` | Blocked |
| `done` | `done` | Done |
| `skipped` | `skipped` | Skipped |

```json
[
  { "id": "todo",        "label": "To Do",        "kind": "todo" },
  { "id": "in_progress", "label": "In Progress",  "kind": "active" },
  { "id": "in_review",   "label": "In Review",    "kind": "review" },
  { "id": "blocked",     "label": "Blocked",      "kind": "blocked" },
  { "id": "done",        "label": "Done",         "kind": "done" },
  { "id": "skipped",     "label": "Skipped",      "kind": "skipped" }
]
```

A project can define additional statuses (e.g. `in_qa` of kind `review`) and pick among same-kind statuses with `--status`.

## 2. State diagram

```mermaid
stateDiagram-v2
    [*] --> Todo

    state Todo <<todo>>
    state InProgress <<active>>
    state InReview <<review>>
    state Blocked <<blocked>>
    state Done <<done>>
    state Skipped <<skipped>]

    Todo --> InProgress : start (deps done, claim created)
    InReview --> InProgress : start (changes requested, or approved with work left)

    InProgress --> InReview : review (claim released)
    InProgress --> Blocked : block (claim released)
    InProgress --> Todo : fail / release
    InProgress --> Done : done (blocker/async agents ask for a review)
    InProgress --> Skipped : skip

    InReview --> InReview : approve / request-changes / reject (decisions; stay in review)
    InReview --> Done : done (approved)
    InReview --> Todo : drop (rejected)
    InReview --> Skipped : skip (rejected)

    Blocked --> Todo : unblock (answers saved as note)
    Blocked --> Skipped : skip

    Done --> Todo : reopen
    Skipped --> Todo : reopen

    Todo --> Blocked : block
    Todo --> Skipped : skip
```

Notes on the diagram:

- `start` is the only command that moves a task *into* `active` (`in_progress`). From `todo` it requires all dependencies done. From `review` it is allowed only when the review was decided for rework (`review_state` is `changes_requested`, or `approved` with something left to do).
- **A review's decisions** (`approve`, `request-changes`, `reject`; T-438) keep the task in `review`: they set `review_state` (`approved` / `changes_requested` / `rejected`) and `reviewed_at`, and add a note (the approval, or the reason). Each opens one way out, and atm refuses the others, for an agent and a person alike: approved, `done` (or `start` for what is left); changes requested, `start` (`atm loop next` offers it to its agent first); rejected, `drop` (back to to do, the work dropped) or `skip`.
- `done` from `review` means the review passed (it was approved). `done` from `active` means the work is finished (subject to review mode — see §4). A done task is done: one that ends without its work done is skipped, with its resolution.
- `fail`, `release`, `drop`, `reopen`, and `unblock` all return a task to `todo`; `skip` sends it to `skipped` from any open state; `reopen` undoes `done` or `skipped`.

## 3. Transition table

The source of truth is the transitions in `TaskMachine` (`atm/lib/state.py`): each one's source kinds, target kind and guards.

The names below are atm-lib's status functions. On the command line (T-425) each is `atm task state <id> <state>`:
`start` is `in_progress`, `review` is `in_review`, `done`, `block` is `blocked`, `skip` is `skipped`, and `todo` runs
the one for the kind the task is in (`todo_command` in `lib/status.py`): `fail` (in progress, with `--reason`),
`release` (in progress, otherwise), `drop` (in review, once rejected), `unblock` (blocked) or `reopen` (done or
skipped). `approve`, `request-changes` and `reject` are a review's decisions,
`atm task review approve|request-changes|reject <id>`.

| command | function | from (kinds) | to (kind) | meaning |
|---------|----------|--------------|-----------|---------|
| `start` | `start_task` | `todo`, `review` | `active` | claim the task; dependencies must be done; from review once decided for rework |
| `review` | `review_task` | `active` | `review` | ask for a review; claim released |
| `done` | `done_task` | `active`, `review` | `done` | finish; record completion / commit; from review once approved |
| `fail` | `fail_task` | `active` | `todo` | back to todo, failure note |
| `release` | `release_task` | `active` | `todo` | back to todo, handoff note |
| `approve` | `approve_task` | `review` | `review` | decision: approved |
| `request-changes` | `request_changes` | `review` | `review` | decision: changes requested; agent gets it back first |
| `reject` | `reject_task` | `review` | `review` | decision: rejected (T-438) |
| `drop` | `drop_task` | `review` | `todo` | a rejected review back to todo, its work dropped |
| `block` | `block_task` | `todo`, `active` | `blocked` | waiting on user/agent decision; claim released |
| `unblock` | `unblock_task` | `blocked` | `todo` | back to todo; Q&A saved as an answer note |
| `reopen` | `reopen_task` | `done`, `skipped` | `todo` | undo finished/skipped (reason kept as note) |
| `skip` | `skip_task` | `todo`, `blocked`, `active`, `review` | `skipped` | not needed (resolution `wont_do`/`obsolete`/`duplicate`); from review once rejected |

```mermaid
stateDiagram-v2
    [*] --> Todo
    Todo --> Active : start
    Active --> Review : review
    Active --> Done : done
    Active --> Blocked : block
    Active --> Todo : fail / release
    Review --> Review : approve / request-changes / reject
    Review --> Done : done (approved)
    Review --> Active : start (decided for rework)
    Review --> Todo : drop (rejected)
    Blocked --> Todo : unblock
    Done --> Todo : reopen
    Skipped --> Todo : reopen
    Todo --> Skipped : skip
    Active --> Skipped : skip
    Blocked --> Skipped : skip
    Review --> Skipped : skip (rejected)
```

## 4. Command-by-command details

All status changes go through `change_task_status` → `change_status` (`atm/lib/status.py`, `atm/common/model.py:607`). Each runs as one atomic store change (`store.change`): status, claim, and the shared waiting/resolved records change together or not at all.

### `atm task state <id> in_progress`
- Moves `todo` (or a decided `review`) → `active`.
- Requires every dependency to be done-kind (`unfinished_dependencies`). A dependency only in the archive counts as done.
- Creates a claim: `{task, agent, branch, worktree, claimed_at, lease_expires_at}` (`lib/status.py:claim_record`).
- Clears any prior `completed` record for the task (`store["completed"].pop`).
- Sets `started_at`, `started_by`, clears `review_requested_at`, `completed_at`, `completed_by`, `agent`.
- Exit code 3 via `atm loop next --claim` when there is no task to start.

### `atm task state <id> in_review [--note TEXT]`
- Moves `active` → `review`; releases the claim.
- Sets `review_requested_at`, `review_state = "waiting"`.
- Adds a `review` note if `--note` is given.
- By review mode: an agent finishing an `active` task whose mode is `blocker` or `async` must `review` first (`check_review_mode`); a person may `done` directly.

### `atm task state <id> done [--note TEXT] [--commit SHA]`
- Moves `active` or `review` → `done`; drops the claim. From `review` only once it is approved (T-438), for
  everyone: `can't finish T-1: its review isn't approved (atm task review approve T-1 decides it)`.
- Records `completed_at`, `completed_by` (agent), optional `commit`. No `--resolution`: a done task is done (one that
  ends without its work done is skipped).
- Batch-review tasks (agent, mode `batch`, coming from `active`) get `sprint_review = "pending"`; the note kind is `review` for batch, else `outcome`.
- After done, tasks waiting on this one become newly ready; an archive hint may be emitted (at most once a day per clone).

### `atm task state <id> todo --reason TEXT`, in progress (fail)
- `active` → `todo`; adds a `failure` note with the reason.
- Wipes the attempt via `back_to_todo` but **keeps the commit** (work is in git, to be finished later).

### `atm task state <id> todo [--note TEXT]`, in progress (release)
- `active` → `todo`; adds a `handoff` note if `--note` given.
- Wipes the attempt but **keeps the commit**.

### `atm task state <id> todo`, in review (drop)
- `review` → `todo`, once the review rejected it (T-438); otherwise refused, for everyone.
- Wipes the attempt (clears `commit` too); the rejection note, with its reason, came with the decision.

### `atm task review approve <id> [--note TEXT]`
- A decision: stays in `review`; sets `review_state = "approved"`, `reviewed_at`.
- Adds an `Approved: …` note (with the note if given).
- Then `done`, or `start` for what is left.

### `atm task review request-changes <id> --reason TEXT`
- A decision: stays in `review`; sets `review_state = "changes_requested"`, `reviewed_at`.
- Adds a `rejection` note with the reason. Then `start`: `atm loop next` offers it to its agent first.

### `atm task review reject <id> --reason TEXT`
- A decision (T-438): stays in `review`; sets `review_state = "rejected"`, `reviewed_at`.
- Adds a `rejection` note with the reason. Then `todo` (drop) or `skipped`. `atm loop next` doesn't hand it out.

Every decision updates the task's `waiting` record (with its `review_state`), so an exit taken from another worktree
meets the same guard (§6). No skill pre-approves a decision (I23).

### `atm task state <id> blocked --on user|agent [--question TEXT]… [--reason TEXT]`
- `todo` or `active` → `blocked`; releases the claim.
- Requires `--on` (`user` or `agent`) and at least one of `--question` / `--reason`.
- Sets `waiting_on`, `questions`, `blocked_at`; adds a `note` if reason given.
- Records a `waiting` entry shared across worktrees (`store["waiting"]`).

### `atm task state <id> todo [--answer TEXT]…`, blocked (unblock)
- `blocked` → `todo`; clears `waiting_on`, `questions`, `blocked_at`.
- Turns questions + answers into one `answer` note (`q → a` pairs).

### `atm task state <id> todo --reason TEXT`, done or skipped (reopen)
- `done` or `skipped` → `todo`; drops claim and `completed` record.
- Adds a `Reopened: …` note; wipes the attempt.
- If reopened inside a closed sprint, reports `stranded` (the loop's default selection never looks there; use `atm sprint move`).

### `atm task state <id> skipped --reason TEXT [--resolution R] [--duplicate-of ID]`
- Any open kind → `skipped` (T-438): `todo`, `blocked`, `active` (its claim dropped) or `review` (once rejected).
- Default resolution `wont_do`; allowed `wont_do`, `obsolete`, `duplicate`.
- Sets `skipped_at`, `resolution` (only when not default), optional `duplicate_of` (must exist, not self).
- Clears `waiting_on`, `questions`, `blocked_at`; adds a `Skipped: …` note.

### Notes command (not a status change)
- `atm task note <id> TEXT` — adds a note (max 500 chars) to a task of any status; does not change status.

## 5. Claims and leases

Claims live in `store["claims"]`, shared by all worktrees of a clone (`atm/lib/claims.py`, `lib/loop.py`).

```json
{
  "task": "T-001",
  "agent": "claude:session-1",
  "branch": "atm/T-001",
  "worktree": ".agentic-task-manager/worktrees/T-001",
  "claimed_at": "2026-11-04T10:00:00Z",
  "lease_expires_at": "2026-11-04T10:30:00Z"
}
```

- Default lease: **two hours** (`meta.lease_minutes` overrides; 30 minutes before T-439). Every atm command an agent
  runs renews its claims once their last renewal is ten minutes old, as the plugin's hook does after each tool call;
  a command run as someone else (`--agent`) renews nobody's.
- `start` creates the claim; every other command drops it (a task in review, blocked or finished has none).
- Only the claiming agent may run status commands on the task; another agent's live claim → exit code **4** ("claimed by … until …").
- Expired claims are dropped and the task returns to `todo` with a `failure` note ("lease expired"), keeping the commit (`keep_commit=True`).
- `done`, `release`, `fail`, `review`, `block` give a special "your own lease expired" error (exit code 4) when the agent's own claim just expired — telling them to run `atm task state <id> in_progress` again.

## 6. Cross-worktree decisions (T-204)

A task that is `blocked` or `in_review` in another worktree is recorded in `store["waiting"]`, with its review's
decision (`review_state`) once one is made (T-438; the decisions themselves are made where the task is in review). An
exit taken in this worktree about such a task (`unblock`, `skip`, `done`, `drop`) is applied here, under the same
guards (the record's decision counts), and propagated via `store["resolved"]` so the other worktree's copy stops
waiting too (`apply_resolutions` in `lib/loop.py`).

Allowed remote exits:

| command | waiting task must be kind |
|---------|---------------------------|
| `unblock` | `blocked` |
| `skip` | `blocked`, `review` (rejected) |
| `done` | `review` (approved) |
| `drop` | `review` (rejected) |

## 7. Side effects recorded on every change

`touch(task, actor, stamp)` (`common/model.py:554`) always sets `updated_at` and `updated_by`.

| field | set by |
|-------|--------|
| `started_at`, `started_by` | `start` |
| `completed_at`, `completed_by` | `done` |
| `review_requested_at`, `review_state`, `reviewed_at` | `review` / `approve` / `request-changes` / `reject` |
| `waiting_on`, `questions`, `blocked_at` | `block` |
| `skipped_at`, `resolution`, `duplicate_of` | `skip` |
| `commit` | `done`, `commit` |
| `sprint_review = "pending"` | `done` on a batch-review task |

Attempt-clearing (`back_to_todo`) removes `started_at`, `completed_at`, `skipped_at`, `started_by`, `completed_by`, `agent`, `review_requested_at`, `waiting_on`, `questions`, `blocked_at`, `sprint_review`, `resolution`, `duplicate_of`; `commit` is removed unless `keep_commit` (`fail`, `release`, lease expiry).

Notes are capped at **500 characters** (`MAX_NOTE`).

## 8. Sprint moves

`atm sprint move <id>… --to <sprint\|next\|active\|backlog>` changes only the sprint assignment (or backlog), **not** the status. Claimed and finished tasks stay where they are. Moving into/out of a sprint starts/ends a `sprint_history` entry. Reopening a task in a closed sprint flags it as `stranded`.

## 9. CLI wiring and errors

`atm task state` (`task_state` in `atm/cli/main.py`) maps the state to a status function of `STATUS_FUNCTIONS`
(`atm/lib/status.py`; for `todo`, `todo_command` picks it by the task's kind, here or, for a task waiting in another
worktree, there), refuses options that aren't the function's (`--commit: not for in_progress (start) …`) and asks
for the ones it needs (`skipped (skip) needs --reason`). `atm task review approve|request-changes|reject` run
the decisions. Both go through `run_status`, which calls `change_task_status`: `--status` picks among same-kind
statuses; `--agent` names the actor.

Error handling (`main` in `atm/cli/main.py`):
- `AtmFailure` → `atm: <message>` on stderr, or `{"error": "...", "code": N}` with `--json`; exit `N` (claim
  conflicts / expired leases use code 4).
- Usage errors (Click's: a missing argument, an unknown option or command) → the same format, with the command's
  usage line, exit code 2.
- Unexpected errors → exit 1, message only (no traceback — skills and the merge driver read stderr).
