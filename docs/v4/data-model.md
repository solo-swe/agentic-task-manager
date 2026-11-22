# v4 data model: tasks.json, the archive and what they hold

Part of the v4 design ([overview](README.md)). This document defines every file and field; the
[atm-cli](atm-cli.md), the [skills](skills-and-workflows.md) and the [dashboard](dashboard.md) only use them.

## 1. Files

All in `.agentic-task-manager/` at the project root.

| File | Committed | Written by | Holds |
|---|---|---|---|
| `tasks.json` | yes | atm-cli only | `meta` and the working tasks: open ones and recently finished ones |
| `tasks-archive.json` (new) | yes | `atm archive` / `unarchive` | finished tasks moved out of `tasks.json`, same format |
| `tasks-dashboard.html` | no (`.gitignore`) | `atm migrate` | a copy of the plugin's dashboard |
| `.gitattributes` | yes | `atm init` / `migrate` | the merge driver for both task files |
| claims store | no (in the clone's git dir) | atm-cli | claims, leases, once-a-day hints |

| ID | P | Requirement |
|---|---|---|
| DM-1.1 | P1 | Agents and skills never read or write the task files directly; every read and change goes through `atm` ([atm-cli](atm-cli.md)). Humans may still edit them by hand: both stay indented, readable JSON. |
| DM-1.2 | P1 | `tasks-archive.json` has the same format as `tasks.json` (`{"meta": {…}, "tasks": […]}`; its `meta` holds only `archived_at` of the last run). A task ID is in at most one of the two files. A missing archive is the same as an empty one. |
| DM-1.3 | P1 | Both files merge task by task through atm's merge driver. |

## 2. `meta`

| Field | Default when absent | Meaning |
|---|---|---|
| `id` | set by `atm init` | the project's ID (the dashboard's saved view) |
| `project`, `description`, `created` | — | as today |
| `statuses` | the six defaults (3) | `[{id, label, kind}]` |
| `phases` | numbers only | `[{id: number, name}]` |
| `types` | the seven defaults | `[{id, label}]` |
| `priorities` | P0–P3 (4) | `[{id, label}]`, most urgent first |
| `default_priority` | `P2` | rank of tasks without a priority |
| `labels` | none | optional descriptions of labels, `[{id, label, description}]` (4.2) |
| `workflow` | none (asked on first push) | `{type, base, verify, max_rounds, commit_template, trailers}` ([workflows](skills-and-workflows.md#1-workflows)) |
| `archive` | `{threshold_kb: 1024, threshold_tasks: 500, keep_days: 14, auto: true}` | when to suggest archiving, and whether agents may do it (5) |
| `next_id` | the highest ID in both files + 1 | the number the next new task gets |
| `sprints` | none | `[{id, goal, state, release, start, end, due, report}]`, in order; the ID is the sprint's name; states `planned`, `active` (at most one), `closed` ([sprints design](../sprints-and-releases-design.md) SR-2.1, SR-2.3) |
| `releases` | none | `[{id, name, state, target, pr, released_at}]`, in order; the ID is letters, digits, `.`, `-` and `_`; states `planned`, `released`, `archived`; `target` a date. A sprint's or a task's `release` must be one of them (`atm check`; SR-2.2) |
| `components` | none | `[{id, label, paths, review}]`: the parts of the project; `paths` are globs over the files (`*` and `**` both cross folders), `review` its default review mode. A task's `components` must name them; `atm add` and `edit --add-file` suggest them from the task's files (`--components auto` sets them), and `atm check` warns about a task whose files match none (SR-2.7) |
| `sprint_names` | `people` | how a sprint created without a name is named: `people` or `time` (SR-2.9) |
| `review` | `none` | the project's default review mode: `blocker`, `async`, `batch` or `none` (SR-2.6) |

| ID | P | Requirement |
|---|---|---|
| DM-2.1 | P1 | Every list or setting that is absent falls back to its default, so 0.12.0 files keep working unchanged. |
| DM-2.2 | P1 | atm-cli validates `meta` when it reads the file: every status has one of the six kinds; at least one `todo`-, one `active`- and one `done`-kind status; unique IDs; a known workflow type. An invalid `meta` is an error naming the problem, and nothing is written. |
| DM-2.3 | P1 | `next_id` is kept by atm-cli: `atm add` advances it (never behind the highest ID in the file), and `atm archive` sets it before any task leaves `tasks.json`, so new IDs never require reading the archive and two agents never get the same ID. IDs keep the project's pattern (`T-001`, or the prefix and width of the existing IDs). |

## 3. Statuses and kinds

Each status has a **kind**; atm-cli, the skills and the dashboard act on kinds, never on status names. The six
kinds are fixed, because atm-cli's rules are written against them.

| Kind | Default status | Label | Meaning |
|---|---|---|---|
| `todo` | `todo` | To Do | not started |
| `active` | `in_progress` | In Progress | an agent works on it (claimed) |
| `review` | `in_review` | In Review | the work is finished and waits for approval |
| `blocked` | `blocked` | Blocked | the work can't go on until someone decides or something changes |
| `done` | `done` | Done | finished; meets dependencies |
| `skipped` | `skipped` | Skipped | won't be done |

| ID | P | Requirement |
|---|---|---|
| DM-3.1 | P1 | Only `done`-kind statuses meet a dependency. A dependency that is only in the archive is done. |
| DM-3.2 | P1 | When a kind has several statuses, commands that move a task to that kind take `--status <id>` to choose one; otherwise the first one listed is used. A `--status` of the wrong kind is an error. |
| DM-3.3 | P1 | The merge driver ranks kinds: todo < active, blocked, review < done, skipped. |

**In Review vs Blocked.** Both release the claim and keep dependents waiting; they differ in what they wait for:

| | Blocked: waiting on a decision | In Review: waiting on approval |
|---|---|---|
| When | the work can't go on until someone answers | the work is finished |
| Ends | answered → `todo`, the next agent continues | approved → `done`; rejected → `todo` with the reason |

## 4. Task fields

Existing fields stay: `id`, `name`, `phase`, `type`, `description`, `dependencies`, `status`, `requirements`,
`use_cases`, `files`, `created_at`, `updated_at`, `started_at`, `completed_at`. `agent` is replaced by `started_by`
(4.1), and `last_failure` (written by `atm fail` today) by a `failure` note; both old fields are still read.

New fields, all optional:

| Field | Set by | Removed by | Meaning |
|---|---|---|---|
| `priority` | `add`, `edit` | — | a priority ID (`P0`…`P3` by default) |
| `labels` | `add`, `edit` | — | free-form tags, e.g. `["spike-2", "ux"]` (4.2) |
| `created_by`, `updated_by`, `started_by`, `completed_by` | atm-cli | see 4.1 | who did it (4.1) |
| `review_requested_at` | `review` | `reject` (a return to `todo`) | when the review was asked for; kept by `done`, since the task's work ended there (DM-4.1) |
| `waiting_on` | `block` | `unblock` | `user` or `agent` |
| `questions` | `block` | `unblock` (moved to `notes` with the answers) | the open questions, a list of strings |
| `blocked_at` | `block` | `unblock` | when it started waiting |
| `blocks` | leaving a blocked status (`unblock`, `skip`) | — | `[{from, to, on}]`: each past block, from its `blocked_at` to when it ended, and who it waited on (T-244). Merged like notes, one per `from` |
| `reviews` | leaving a review status (`done`, `reject`) | — | `[{from, to}]`: each past review, from its `review_requested_at` to its end. Merged like `blocks` |
| `attempts` | going back to to do (`release`, `fail`, `reopen`, `drop`, a lapse) | — | `[{from, to, by, outcome?}]`: each past attempt, from its `started_at` to its end (a lapse: its last update, `outcome: lapsed`), and who started it (ATM-721), so its work still counts once the attempt's fields are cleared. A task's work is its attempts and its current one, less its `reviews` and `blocks` (atm-lib's `work`, served as `derived.work`). Merged like `blocks` |
| `skipped_at` | `skip` | `reopen` (a return to `todo`) | when it was skipped: a skipped task's finish time, as `completed_at` is a done one's (T-181). `atm migrate` fills in `completed_at` and `skipped_at` from `updated_at` for finished tasks from before these existed, so a later edit (which moves `updated_at`) doesn't move them in time |
| `notes` | `note`, `--note`, `--reason`, `--answer` | — | `[{at, by, kind, text}]`; kind is `handoff`, `outcome`, `review`, `rejection`, `failure`, `answer` or `note` |
| `commit` | `commit`, `done --commit` | a return to `todo` | the SHA of the task's commit |
| `pr` | `pr` | — | `{number, url}` of the PR that contains the task |
| `merged_at` | `merged` | — | when that PR was merged |
| `sprint` | `add --sprint`, `edit --sprint` | `edit --backlog` | the sprint the task is in; absent: the backlog ([sprints design](../sprints-and-releases-design.md) SR-2.5) |
| `sprint_history` | atm-cli, when `sprint` changes | — | `[{sprint, from, to}]`: each sprint the task was in; the open entry has no `to` (SR-2.5) |
| `with` | `add --with user`, `edit --with user` | `edit --with ''` | `user`: the work is done together with the user; `atm next` skips it unless `--with-user`, `atm start <id>` takes it. `atm migrate` gives unfinished tasks labelled `interactive` `with: user` once (`meta.migrations` records it) (SR-3.5, SR-6.5) |
| `review_state` | `review` (`waiting`), `approve` (`approved`), `request-changes` (`changes_requested`) | leaving the review kind | the review's state beside the task's review status; `approved` and `changes_requested` make it work for an agent again: `atm next` offers it first (SR-6.1, SR-3.9) |
| `reviewed_at` | `approve`, `request-changes` | leaving the review kind | when the review was decided: its agent gets the task back first for one lease length after it; the round then goes into `reviews` with its `outcome` |
| `sprint_review` | `done` by an agent on a `batch` task | `sprints close --reviewed` (`reviewed`), `reopen` | `pending` until the end-of-sprint review, then `reviewed` (SR-4.4) |

| ID | P | Requirement |
|---|---|---|
| DM-4.1 | P1 | A task's work runs from `started_at` to `review_requested_at` when it asked for a review, else to `completed_at`. Time in review or blocked is shown separately and never counted as agent work. |
| DM-4.2 | P1 | A note's text is at most 500 characters (longer text is refused with a message suggesting a file in the repo instead). |
| DM-4.3 | P1 | **Stages of Done** are derived, not stored as statuses: Done (no `pr`), In PR (`pr`, no `merged_at`), Merged (`merged_at`). They never affect dependencies, and a workflow change can't leave a task without a status. |

### 4.1 Who: `created_by`, `updated_by`, `started_by`, `completed_by`

Today a task has one `agent` field: set by `atm start`, removed when the task goes back to `todo`. It answers "who is
working on this" but not who created the task, who finished it, or who approved a review.

**Actors.** Every command already works out who runs it: `--agent`, else `ATM_AGENT`, else `claude:<session>` in
Claude Code, else `user@host:path`. The last one is unreadable for a human running `atm` in a terminal, so it becomes
the git user (`git config user.name`, e.g. `Hakan Serce`), falling back to `user@host`. An actor starting with
`claude:` (or set with `--agent`) is an agent; anything else is a person.

| Field | Set when | Removed when | Notes |
|---|---|---|---|
| `created_by` | `atm add` | never | `/atm:create-tasks` passes the user as the actor when the user asked for the tasks |
| `updated_by` | every change, with `updated_at` | never | the last actor; for "last changed by" only |
| `started_by` | `start`, `next --claim` | the task returns to `todo` | replaces `agent`; the claim's owner |
| `completed_by` | `done` | the task returns to `todo` | differs from `started_by` when someone else approves a review, or a person marks a task done |

**History across handoffs.** When a task is released or fails and another agent finishes it, today's `agent` loses
the first agent. The `handoff` and `failure` notes keep it (`by`), so the detail panel can show every attempt. No
separate history list is needed.

| ID | P | Requirement |
|---|---|---|
| DM-4.4 | P1 | atm-cli writes the four fields as above and no longer writes `agent`. Readers use `started_by`, falling back to `agent`; `atm migrate` doesn't rewrite old tasks, so there is no one-off churn in git. |
| DM-4.5 | P1 | The actor for a person running `atm` outside Claude Code is the git user name, else `user@host`. |

**On the dashboard** ([dashboard](dashboard.md#4-notes-stages-and-actors), D-4.5):
- **Agents tab and utilisation:** lanes and work time stay per `started_by`, which is today's `agent`, so nothing
  changes in what they measure. People who start tasks get a lane too, with a person icon instead of the bot icon.
- **Review approvals:** when `completed_by` differs from `started_by`, the task's bar on the Agents tab ends with a
  small marker ("approved by Hakan Serce"); the review time isn't counted as work for either.
- **Detail panel:** one line, "Created by X · started by Y · completed by Z", plus "last changed by" when it adds
  something; attempts from `handoff` and `failure` notes.
- **Filter:** an **Agent** filter group (by `started_by`, with counts), shown when more than one actor appears.

### 4.2 Labels

Free-form tags instead of markers in the text (`SPIKE-2` in the name or description today).

| ID | P | Requirement |
|---|---|---|
| DM-4.6 | P1 | `labels` is a list of strings: lowercase letters, digits, `-` and `_`, up to 32 characters each. Any label is allowed. |
| DM-4.7 | P2 | `meta.labels` may describe labels (`[{id, label, description}]`) for the dashboard's label text and order; it never restricts which labels a task can have. |

atm-cli: `add --label`, `edit --add-label/--remove-label`, `list --label`, counts in `status`. Dashboard: chips on
cards, a Labels filter group, search matches labels (D-2.4). Skills: `/atm:create-tasks` proposes labels for groups
of tasks (spikes, areas) instead of writing them into names.

## 5. Size and the archive

This repository: 110 tasks, 119 KB, about 1.1 KB per task. Measured with the benchmark
([atm-cli](atm-cli.md#7-benchmark), `npm run bench`) on generated files with every v4 field (priority, labels,
actors, notes, commit; 90% done), on a developer's Mac (Python 3.14, Playwright's Chromium):

| | 1k tasks | 10k tasks |
|---|---|---|
| `tasks.json` without archiving | 1.4 MB | 14.3 MB |
| Each `atm` write (the whole file, under the lock; including Python's start) | 0.23 s | 0.31 s |
| The merge driver (three versions) | 0.19 s | 0.41 s |
| Dashboard: first load (Kanban) | 0.17 s | 3.5 s |
| Dashboard: a reload that changes a task (every 5 s while agents work) | 0.08 s | 3.2 s |
| Git, per commit | ~200 KB compressed blob (estimate) | ~2 MB compressed blob until `gc` packs it (estimate) |
| GitHub | fine | "Large diff not rendered"; hard to edit by hand |

atm-cli copes with 10k tasks; the dashboard is what doesn't: at 10k tasks it spends most of every 5 seconds
reloading. Conditional fetch (D-5.1) removes the reloads where nothing changed; the archive keeps the working file
small enough that the rest stays fast.

Most of a large project is finished tasks that never change again; a format change doesn't stop that (one task per
line saves only the indentation, about 15%). Moving them out of the working file does.

| ID | P | Requirement |
|---|---|---|
| DM-5.1 | P1 | `atm archive` moves `done`- and `skipped`-kind tasks to `tasks-archive.json`, except those finished in the last `keep_days` days and those in an open PR (`pr` without `merged_at`). |
| DM-5.2 | P1 | A move writes one file, then the other (`archive`: the archive first; `unarchive`: `tasks.json` first), so a crash in between leaves a task in both files, never in neither. A task in both keeps its `tasks.json` copy: the next `archive` or `unarchive` drops the archive's copy (an interrupted archive simply hasn't happened; an interrupted unarchive is complete), and `atm check` reports it meanwhile. |
| DM-5.3 | P1 | When `tasks.json` is over `archive.threshold_kb` or has more than `archive.threshold_tasks` archivable tasks, atm-cli suggests archiving ([atm-cli](atm-cli.md#5-hints)). |
| DM-5.4 | P1 | `atm archive` and `atm unarchive` record the archive's size as `meta.archived` in `tasks.json` (written with the move, so no extra churn), so the dashboard can show "N archived" without loading the archive (dashboard D-5.4). |

**Options considered:** one task per line (~15% smaller, doesn't stop growth); SQLite (binary: no diffs, no merges,
and the dashboard would need a WebAssembly reader); an append-only event log (not readable or editable; every read
replays it); one archive file per phase (many files over time).

## 6. Compatibility

| ID | P | Requirement |
|---|---|---|
| DM-6.1 | P1 | Files without the new `meta` fields or task fields behave as in 0.12.0, with `in_review` added to the default statuses. |
| DM-6.2 | P1 | atm-cli versions older than v4 don't know the archive and would see a dependency on an archived task as missing. The changelog says to update every agent's plugin before the first `atm archive`. |
| DM-6.3 | P1 | **Phases become sprints** ([sprints design](../sprints-and-releases-design.md) SR-8.1): `atm migrate` (which every skill runs first; `atm serve` leaves phases alone until T-320) converts a project with phases, in `tasks.json` and the archive, once. Each phase that has tasks or a name becomes a sprint, in phase order, named after the phase ("Phase n" when unnamed; " (phase n)" added to a name already taken); a sprint whose tasks are all finished is `closed` (`start` the earliest `started_at`, `end` the latest finish time, `report: {done, carried: []}`), the others `planned`, none active. A task's `phase` becomes `sprint` plus a `sprint_history` entry from its `created_at`; a task without a phase goes to the backlog; `phase` and `meta.phases` are removed. The files are staged, not committed. A task that still has a `phase` in a migrated project (merged in from an unmigrated branch, SR-8.5) is moved to its phase's sprint by the next `atm migrate`. Commands keep working on unmigrated files (no write guard, SR-8.3); atm-cli ≤ 0.23 reads a migrated file as tasks without phases (SR-8.4), so the changelog says to update every agent's plugin before the first run in a shared project. |
