# Sprints, releases and components: design

**Status:** proposal, for review · **Date:** 2026-10-30 · **Starts from:** atm 0.23.0 · **IDs:** SR-

atm moves from **phases** to **sprints and releases**: a sprint is a batch of work that agents do and a person
plans and closes; a release is what ships together. With them come **components** (areas of the product), **review
modes and states**, a **`with: user`** field for work done together, a **resolution** for finished work and two
**relations**. Phases are migrated to sprints and removed.

This design turns [the tracker fields research](tracker-fields-research.md) into requirements. Its section 8 holds
the decisions this rests on (*Decided*, 2026-10-30); this document doesn't repeat the reasoning, only what follows
from it. Requirement IDs are `SR-<section>.<n>`; priorities as in [v4](v4/README.md) (P1 needed, P2 wanted).

## 1. Decisions

From the research's section 8, and the ones this design adds (marked *new*):

1. **Sprint and release, optionally linked:** a sprint delivers to at most one release; a release takes one or more
   sprints. A task's release defaults to its sprint's (research Q1).
2. **One active sprint** at a time; sprints are `planned`, `active` or `closed`. The **backlog** is the tasks with no
   sprint (Q4, 8.1).
3. **The loop works a selection** — the active sprint, a named sprint, a list of tasks, all tasks or the backlog —
   and stops when the selection is done; `--phase` becomes `--sprint` (Q2, Q3).
4. **Sprint planning and closing are sessions with the user**, each a skill; moving tasks between sprints and the
   backlog is an atm command (8.1). The end-of-sprint review of `batch` tasks is a step of closing, not a task
   (review question 3).
5. **Dates are optional:** a sprint's `start` and `end` are set when it starts and closes; `due` is planned (Q4).
6. **Release IDs are any string** (Q5). A release is `released` when its PR merges (`/atm:push`, `atm merged`).
7. **Components** carry paths and a default review mode (Q6).
8. **Review modes** `blocker`, `async`, `batch`, `none`, and **review states** `waiting`, `changes_requested`,
   `approved` inside the review kind (research 5.5, 5.6); dependents of a task in review wait (Q9); a change asked
   for at the end-of-sprint review is a follow-up task (Q10); rework goes back to the same agent first (Q11).
9. **`with: user`** marks work done together; unattended loops skip it (Q7).
10. **Phases are migrated to sprints and removed** (Q8).
11. *New:* **`atm migrate` does the migration** automatically, as it does every other one: phases become sprints the
    first time a new atm runs in a project, staged in git, not committed. The dashboard reads phases as sprints for
    projects not yet migrated (SR-8.3). (Review question 1.)
12. *New:* **Sprints are named, and the name is the ID.** When none is given, atm generates one: a historical
    person's name not yet used in the project ("Ada Lovelace"), or, if the project prefers, the time it was created
    ("May 12-10:38pm", with "-2", "-3" for more in the same minute). (Review question 2.)
13. *New:* **`meta.sprints`, `meta.releases` and `meta.components` merge item by item**, by ID, like tasks (SR-2.4).
14. *New:* **Epics (`parent`) stay out**, as the research proposed: a follow-up document.
15. *New:* **How many sprints a project has, and what goes in each, is decided in planning**, never fixed in
    advance; this design's own work is ordered by dependencies (section 9), not pre-planned into sprints.
    (Review question 4.)

## 2. Data model

Extends [v4/data-model.md](v4/data-model.md); everything optional, files without it behave as today minus phases
(SR-8).

### 2.1 `meta`

```json
{
  "sprints": [
    { "id": "Testing together", "state": "closed", "release": "0.22.0",
      "start": "2026-10-26T09:12:00Z", "end": "2026-10-27T23:14:20Z",
      "report": { "done": ["T-276", "T-280"], "carried": ["T-277", "T-279"], "to": "Themes" } },
    { "id": "Themes", "goal": "Two board themes, tried live", "state": "active",
      "release": "0.23.0", "start": "2026-10-28T08:00:00Z", "due": "2026-11-02" },
    { "id": "Ada Lovelace", "state": "planned" }
  ],
  "releases": [
    { "id": "0.22.0", "state": "released", "pr": { "number": 27, "url": "…" }, "released_at": "2026-10-27T23:14:20Z" },
    { "id": "0.23.0", "state": "planned", "target": "2026-11-02" }
  ],
  "components": [
    { "id": "dashboard", "label": "Dashboard", "paths": ["dashboard/**", "plugin/templates/**"], "review": "batch" },
    { "id": "atm-cli", "label": "atm-cli", "paths": ["atm-cli/**", "plugin/scripts/**", "tests/scripts/**"] }
  ],
  "review": "none",
  "sprint_names": "people"
}
```

| Field | Item fields | Meaning |
|---|---|---|
| `sprints` | `id` (its name), `goal`, `state`, `release`, `start`, `end`, `due`, `report` | in order: the order they are planned and worked in |
| `releases` | `id`, `name`, `state`, `target`, `pr`, `released_at` | in order |
| `components` | `id`, `label`, `paths`, `review` | `paths` are globs, matched against tasks' `files` |
| `review` | — | the project's default review mode; `none` when absent (today's behaviour) |
| `sprint_names` | — | how atm names a sprint created without a name: `people` (default) or `time` (SR-2.9) |

| ID | P | Requirement |
|---|---|---|
| SR-2.1 | P1 | Sprint states are `planned`, `active`, `closed`; at most one sprint is `active`. `start` is set when a sprint becomes active, `end` when it closes; `due` (a date) is optional and only planned. A closed sprint keeps its `report`: the tasks done in it, the tasks carried over and where to (SR-4.3). |
| SR-2.2 | P1 | Release states are `planned`, `released`, `archived`. A release's `pr` and `released_at` are set by `atm merged` when the PR that contains its tasks merges (SR-6.2), or by hand. `name` is optional (the ID is any string, Q5). |
| SR-2.3 | P1 | atm-cli validates the new lists with `meta` (DM-2.2): unique IDs, known states, one active sprint, a sprint's `release` and a component's `review` known. A sprint's ID is its name: up to 48 characters, no leading or trailing space, unique in the project (compared case-insensitively). Release and component IDs are any string of letters, digits, `.`, `-` and `_`. |
| SR-2.4 | P1 | The merge driver merges `sprints`, `releases` and `components` item by item, by ID, like tasks: an item changed on one side takes that side; changed on both, a sprint's or release's further state wins (planned < active < closed; planned < released < archived), else ours, reported as a conflict; two different items added on both sides with the same ID are a conflict to resolve by hand (a name can't be renumbered the way a task ID is). Other `meta` keys merge as today. |
| SR-2.9 | P1 | **Sprint names.** `atm sprints add` takes a name; without one, atm generates it by `meta.sprint_names`: `people` picks a historical person's name not yet used in the project from a list shipped with atm (scientists, writers, explorers, artists: a few hundred, so a project doesn't run out), `time` uses the creation time in the project's local time, `May 12-10:38pm`, with `-2`, `-3` … for more in the same minute. A sprint's name never changes once tasks refer to it (`atm sprints rename` would rewrite every task's `sprint` and `sprint_history` in both files under the lock: P2). |

### 2.2 Task fields

| Field | Set by | Meaning |
|---|---|---|
| `sprint` | `add`, `edit`, `move`, sprint close | a sprint ID; absent: the backlog |
| `sprint_history` | atm-cli, when `sprint` changes | `[{sprint, from, to}]`: each sprint the task was in and when; merged like `blocks` |
| `release` | `add`, `edit` | a release ID; when absent, the sprint's release applies |
| `components` | `add`, `edit`, suggested from `files` | component IDs |
| `review` | `add`, `edit` | `blocker`, `async`, `batch`, `none`; when absent, the first component's, then `meta.review` |
| `review_state` | `review`, `approve`, `request-changes` | in the review kind only: `waiting`, `changes_requested`, `approved` |
| `sprint_review` | `done` on a `batch` task; closing the sprint | `pending` until the end-of-sprint review, then `reviewed` (SR-4.4) |
| `with` | `add`, `edit` | `user`: the work is done with the user present |
| `resolution` | `done`, `skip` | done and skipped kinds only: `done`, `wont_do`, `duplicate`, `obsolete` |
| `related` | `add`, `edit` | task IDs, a link both ways that doesn't block |
| `duplicate_of` | `skip --duplicate-of` | a task ID, with the `duplicate` resolution |

`phase` is removed by the migration (SR-8); `sprint_history` keeps where a task was planned.

| ID | P | Requirement |
|---|---|---|
| SR-2.5 | P1 | A task is in at most one sprint (`sprint`), or in the backlog. Every change of `sprint` (by `add`, `edit`, `move`, closing a sprint) ends the open `sprint_history` entry and starts a new one, so a closed sprint's report and "added mid-sprint" (SR-7.2) can be derived. |
| SR-2.6 | P1 | A task's effective release is its `release`, else its sprint's `release`; its effective review mode is its `review`, else its first component's that has one, else `meta.review`, else `none`. Readers use these, never the raw fields alone. |
| SR-2.7 | P2 | `atm add` and `atm edit --add-file` suggest components from `files` and the components' `paths` (printed, not set, unless `--components auto`); `atm check` warns about a task whose files match no component when the project has components. |
| SR-2.8 | P1 | `related` is stored on one side and shown on both; `duplicate_of` requires the `duplicate` resolution and an existing task (in either file). Neither affects readiness: only `dependencies` do (DM-3.1). |

## 3. atm-cli

New and changed commands. Every command takes `--json`, as today.

### 3.1 Sprints and the backlog

| Command | Does |
|---|---|
| `atm sprints [--all]` | the sprints in order: ID, name, state, release, due, task counts by kind; closed ones only with `--all` |
| `atm sprints add ["<name>"] [--goal T] [--release R] [--due DATE] [--after S]` | a planned sprint, last or after `S`; without a name, atm generates one (SR-2.9) |
| `atm sprints edit <S> [--name] [--goal] [--release] [--due]` | changes a sprint |
| `atm sprints start <S>` | makes it active; refused while another is active; sets `start` |
| `atm sprints close <S> [--carry-to <S2> \| --carry-to new [--name "…"] \| --carry-to backlog]` | closes it (SR-4.3) |
| `atm move <id>... --to <S> \| next \| active \| backlog` | moves tasks: into a sprint, the sprint after their current one, the active sprint, or the backlog |
| `atm backlog` | the tasks with no sprint, as `atm list` prints them |

| ID | P | Requirement |
|---|---|---|
| SR-3.1 | P1 | `atm move` refuses a task that is claimed (active kind) or finished (done or skipped kind): work in progress or done stays where it happened. `--to next` with no planned sprint after the current one is refused with a message naming `atm sprints add`. |
| SR-3.2 | P1 | `atm list`, `atm status`, `atm archive` and `atm next` take `--sprint <S>` (repeatable), `--backlog`, `--release <R>` and `--component <C>`; `--phase` is removed (SR-8). `atm status` shows the active sprint (goal, due, progress) first. |
| SR-3.3 | P1 | `atm add` takes `--sprint`, `--release`, `--component` (repeatable), `--review`, `--with user`, `--related`; `atm edit` the same plus their removal. A task added without `--sprint` goes to the backlog. |

### 3.2 What `atm next` takes (the selection)

The loop's scope is a **selection**; `atm next` takes the same options the loop does (SR-5.1):

| Selection | `atm next …` | Takes |
|---|---|---|
| the active sprint *(default when the project has sprints)* | — | the active sprint's ready tasks |
| a sprint | `--sprint S` | that sprint's ready tasks, whatever its state but closed |
| a list | `--tasks T-1,T-5` | those tasks, in that order when ready |
| all | `--all` | every ready task: the active sprint first, then planned sprints in order, then the backlog |
| the backlog | `--backlog` | the backlog's ready tasks |

Within a selection: tasks with `review_state` `changes_requested` or `approved` first (SR-6.4), then priority rank,
then ID, as today without the phase.

| ID | P | Requirement |
|---|---|---|
| SR-3.4 | P1 | With no selection option, a project with an active sprint selects it; a project with sprints but none active selects nothing, and the reason says so ("no active sprint: plan one with /atm:plan-sprint, or run with --all"); a project without sprints selects all tasks, as today. |
| SR-3.5 | P1 | `atm next --claim` skips tasks with `with: user` unless `--with-user` is given (SR-6.6). `atm start <id>` never skips them: naming a task is choosing it. |
| SR-3.6 | P1 | When a selection has nothing ready, the reason distinguishes: all done (the selection is finished), the rest waiting (on dependencies, review, a decision, the user), or empty. The loop stops on any of them (SR-5.2). |

### 3.3 Releases

| Command | Does |
|---|---|
| `atm releases [--all]` | the releases: ID, state, target, task counts by kind and stage (done, in PR, merged) |
| `atm releases add <R> [--name] [--target DATE] [--pr N]` | a planned release; with `--pr N`, every task whose `pr` is `N` gets `release: R` (for releases recovered from PRs, SR-8.2) |
| `atm releases edit <R> …`, `atm releases archive <R>` | changes or archives it |
| `atm releases notes <R>` | a Markdown draft of the release's changes from its tasks: grouped by type, each with its ID, name and outcome note; for the CHANGELOG and WHATS-NEW entries a person writes |

| ID | P | Requirement |
|---|---|---|
| SR-3.7 | P1 | `atm merged --pr N` (exists) also marks released the release whose tasks are all in PR `N` and merged, setting its `pr` and `released_at`; a release with tasks outside the PR stays planned and `atm merged` says which. |

### 3.4 Reviews, resolution, relations

| Command | Does |
|---|---|
| `atm review <id> [--note]` *(exists)* | in review, `review_state: waiting` |
| `atm approve <id> [--note]` | `review_state: approved`: back to an agent to finish what the task asks after approval |
| `atm request-changes <id> --reason T` | `review_state: changes_requested`, with a `rejection` note; stays in the review kind |
| `atm reject <id> --reason T` *(exists)* | kept: back to `todo`, for work that should start over |
| `atm done <id> [--resolution R]` | from active, or from review only when approved or by the reviewer (as today); resolution `done` by default |
| `atm skip <id> --reason T [--resolution wont_do\|obsolete\|duplicate] [--duplicate-of ID]` | as today, with a resolution (`wont_do` by default) |

| ID | P | Requirement |
|---|---|---|
| SR-3.8 | P1 | The review mode decides how a task ends (research 5.5): `atm done` on an active task whose mode is `blocker` or `async` is refused ("this task is reviewed: `atm review`"); for `batch` it is done, with a `review` note and `sprint_review: pending`, for the end-of-sprint review (SR-4.4); for `none` it is done. A person (not an agent) may always `atm done` a task. |
| SR-3.9 | P1 | A task with `changes_requested` or `approved` is work for an agent: `atm next --claim` takes it before new tasks, preferring its `started_by` for one lease length after the review ended, then any agent (Q11). Taking it moves it to the active kind; its `reviews` history records the round and its outcome. |

## 4. The sprint workflow

Research 8.1, as atm commands and skills.

### 4.1 Planning: `/atm:plan-sprint` (new skill)

A session with the user, in the conversation:

1. Show the state: the active sprint (if any) and what's left in it, the planned sprints, the backlog grouped by
   label and component, and tasks carried over from the last closed sprint (its `report`).
2. Agree on the sprint: new or a planned one, its name, goal, release and optional due date (`atm sprints add` /
   `edit`).
3. Go through candidates with the user: pull from the backlog (`atm move … --to S`), push back what doesn't fit
   (`--to next` or `backlog`), and check dependencies (a task whose dependencies are in a later sprint or the
   backlog is flagged).
4. Start it when the user says so (`atm sprints start`), and say how to run it (`/atm:loop-tasks`).

| ID | P | Requirement |
|---|---|---|
| SR-4.1 | P1 | `/atm:plan-sprint` never starts a sprint, moves a task or sets a date without the user agreeing in the session; it proposes, the user decides. It may create follow-up tasks only through `/atm:create-tasks` (S-3.4). |
| SR-4.2 | P2 | It shows, for the proposed sprint, its size against the last closed sprints (tasks, and measured working time per task from DM-4.1), so the user sees whether it's bigger than usual. |

### 4.2 Working a sprint

`/atm:loop-tasks` with the selection (SR-5.1). Tasks can still be moved at any time (`atm move`); a task pulled into
the active sprint mid-sprint is "added" in its report (SR-7.2).

### 4.3 Closing: `/atm:close-sprint` (new skill) and `atm sprints close`

1. **Review** what the sprint got done, with the user: the done tasks with their outcome notes, and first the `batch`
   tasks waiting for this review (`sprint_review: pending`, SR-4.4), each with its `review` note. Small fixes can be
   made live; what needs more becomes a follow-up task.
2. **Decide the rest**: each unfinished task is carried to the next sprint (default), moved to the backlog, or
   skipped with a resolution.
3. **Choose where carried tasks go**: the next planned sprint; one of several, chosen by the user; or a new one,
   created now.
4. **Close**: `atm sprints close S --carry-to …`; offer `/atm:plan-sprint` for the next one.

| ID | P | Requirement |
|---|---|---|
| SR-4.3 | P1 | `atm sprints close` refuses while a task in the sprint is claimed (active kind) or a `batch` task in it is still `sprint_review: pending` (`--reviewed` marks them all reviewed, for `/atm:close-sprint` once the review step is done); otherwise it moves every task not done or skipped to `--carry-to` (a sprint, `new`, or `backlog`; required when such tasks exist), keeping their status (a task in review stays in review), sets `state: closed`, `end`, and `report: {done, carried, added, to}`. |
| SR-4.4 | P1 | **The end-of-sprint review is a step of `/atm:close-sprint`, not a task** (review question 3): a `batch` task, when done, gets `sprint_review: pending` and its `review` note, and dependents go on (research 5.5). The sprint's pending reviews show in `atm status`, `atm sprints` and the dashboard's sprint header (SR-7.2), so they are visible as waiting on the user. Closing reviews them first; what the user wants changed becomes follow-up tasks in the next sprint, `related` to the original, which stays done (Q10). |
| SR-4.5 | P1 | Carried tasks keep their `sprint_history`, so a task carried three times shows it (the dashboard flags tasks carried twice or more). |

## 5. Skills

| ID | P | Requirement |
|---|---|---|
| SR-5.1 | P1 | `/atm:loop-tasks` takes a selection: `--sprint S` (replaces `--phase`), `--tasks T-1,T-5`, `--all`, `--backlog`; none: the active sprint, or all tasks in a project without sprints (SR-3.4). `--type` and the other flags stay. `/atm:do-task <id>` is unchanged; without an ID it takes the next task of the default selection. |
| SR-5.2 | P1 | The loop stops when its selection has nothing ready and says why (SR-3.6): for a finished sprint, what it got done and what's left, and that `/atm:close-sprint` closes it (Q2). `--continue` goes on to the next planned sprint once the active one is finished: it never closes a sprint itself. |
| SR-5.3 | P1 | `--with-user` lets the loop take `with: user` tasks (the user is watching); without it they are skipped and listed at the end ("waiting for you: T-…"). |
| SR-5.4 | P1 | `/atm:do-task` and the loop end a task by its review mode (SR-3.8): `blocker` waits for the review in the session (asks the user now, or stops an unattended loop); `async` goes on; `batch` and `none` are done. The do-task text that reads "a review is asked for" from descriptions is replaced by the mode. |
| SR-5.5 | P1 | `/atm:create-tasks` proposes a sprint (the active one, a planned one, a new one) or the backlog for new tasks, and components and a review mode for each, instead of phases. |
| SR-5.6 | P1 | `/atm:push` records releases with merges (SR-3.7) and, in a release-branch project, suggests the branch name from the active sprint's release (`dev/<release>`), never renaming an existing branch. |

## 6. Reviews and working together

| ID | P | Requirement |
|---|---|---|
| SR-6.1 | P1 | Review states live in the review kind: `review_state` is `waiting` after `atm review`, `changes_requested` after `atm request-changes`, `approved` after `atm approve`; it is removed when the task leaves the review kind. A project's own review statuses keep working: the state is a field beside them. |
| SR-6.2 | P1 | Dependents of a task in the review kind wait, whatever its state (Q9): only done meets a dependency (DM-3.1). |
| SR-6.3 | P1 | `async` tasks: the agent or loop goes on with other ready tasks after `atm review`; `blocker` tasks: it takes no other task until the review ends (SR-5.4). |
| SR-6.4 | P1 | `changes_requested` and `approved` tasks are taken back by agents first (SR-3.9). The /atm:do-task workaround that records an approval as a rejection (`reject --reason 'Approved; …'` then `start`) is replaced by `atm approve`. |
| SR-6.5 | P2 | The `interactive` label is migrated: tasks with it get `with: user` (the label stays, as a label). `/atm:create-tasks` sets `with: user` for work the user asked to do together. |
| SR-6.6 | P1 | `with: user` tasks show as "needs you" on the dashboard (with In Review and Blocked on the user, D-3), and `atm next --claim` skips them in unattended loops (SR-3.5). |

## 7. Dashboard

Phases leave the dashboard; sprints take their place, and releases and components are new.

| ID | P | Requirement |
|---|---|---|
| SR-7.1 | P1 | **Filters:** a Sprint group (the active sprint first, then planned, the backlog, then closed ones folded like finished phases are today), Release and Component groups; the Phase group is removed. |
| SR-7.2 | P1 | **A sprint header** above the Kanban, List and Graph when a single sprint is selected (the active one by default): name, goal, due ("due in 3 days", overdue in red), release, progress (done of total) and, from `sprint_history`, how many tasks were added mid-sprint and carried in, and how many `batch` tasks wait for the end-of-sprint review. *T-323 made it the scope bar:* the board views open on the active sprint (whichever it is: the Sprint filter's `:active`), the bar's menu picks one scope (the active sprint, a planned one, the backlog, all tasks, a recent closed one) in step with the Sprint filter, it carries the board's one progress figure ("showing N, filtered" when other filters narrow it), and hints name the skill to run next (close the sprint once its work is done, start one when none is active). The Sprint filter scopes only the board views; the Progress tab shows every sprint and focuses one itself. |
| SR-7.3 | P1 | **Kanban:** "Group by sprint" replaces "Group by phase" (one lane per sprint, the backlog last). Cards show their sprint only when more than one is shown. |
| SR-7.4 | P1 | **Progress tab:** the phase table becomes a sprint table (closed sprints with their report: done, carried, added; the active and planned ones with progress); the burndown's "phase" range becomes the sprint's (start to end, or to due), and the forecast is per sprint. |
| SR-7.5 | P1 | **A release view** (Progress tab, or the Project tab): each release with its sprints, task counts by stage (done, in PR, merged), its PR and state, and the notes draft (SR-3.3) to copy. |
| SR-7.6 | P1 | **Detail panel:** sprint (with its history when carried), release, components, review mode and state, `with: user`, resolution, related tasks and duplicates, each linked. |
| SR-7.7 | P2 | **The overview and project cards:** the active sprint's name, progress and due replace the current phase. |
| SR-7.8 | P1 | The dashboard reads unmigrated projects' phases as sprints (SR-8.3), so a dashboard serving several projects shows them all the same way. |

## 8. Migrating phases

### 8.1 The migration

| ID | P | Requirement |
|---|---|---|
| SR-8.1 | P1 | `atm migrate` turns phases into sprints in both `tasks.json` and the archive, once (a project with `meta.sprints` is migrated): each phase that has tasks or a name becomes a sprint named after it (its name, or "Phase n" for an unnamed one; a name already taken gets " (phase n)"), in phase order; a task's `phase` becomes its `sprint` and a `sprint_history` entry (from its `created_at`), and `phase` and `meta.phases` are removed. Sprints whose tasks are all finished are `closed` (`start`: their earliest `started_at`, `end`: their latest finish time, a report without carried tasks); the others are `planned`. None is made active: the migration prints the planned sprints and says to start one with `/atm:plan-sprint` (SR-3.4). The changes are staged, not committed, as other migrations are. |
| SR-8.2 | P2 | Releases can't be derived from phases in general. `atm releases add <R> --pr N` assigns a release to every task in PR `N` (SR-3.3); for this repository, the CHANGELOG's versions and the PRs on tasks give the list, done once by hand (SR-9). |
| SR-8.3 | P1 | Until a project is migrated, the dashboard shows its phases as read-only sprints named after them (SR-7.8), and atm-cli commands keep working on it: there is no write guard, since every skill runs `atm migrate` first, which converts it (settled with Hakan on 2026-10-30, T-292: a guard would cost rewriting the phase fixtures of atm-cli's tests for little). Removing phase support from atm-cli (`--phase`, `atm phases`, the phase in `add`, `list`, `status` and `atm serve`'s overview summary) is its own task, after the dashboard reads sprints everywhere. |
| SR-8.4 | P1 | **Older atm versions** (≤ 0.23) would read a migrated file as tasks without phases: `atm next` would order by priority alone and `--phase` would select nothing, but nothing would be lost or rewritten. The changelog says to update every agent's plugin before the first run of the new version in a shared project, as for the archive (DM-6.2). |
| SR-8.5 | P1 | The merge driver handles a branch migrated on one side and not on the other: a task with `phase` on the unmigrated side and `sprint` on the migrated side keeps `sprint`; `meta.phases` is dropped when the merged `meta` has `sprints`. The next `atm migrate` finishes any task that still has a `phase`. |

### 8.2 Compatibility

| ID | P | Requirement |
|---|---|---|
| SR-8.6 | P1 | A project that never uses sprints, releases or components works as today without phases: `atm next` takes all tasks by priority and ID, the dashboard shows no sprint header or groups. A migrated project with one planned sprint per former phase is the usual starting point; it can also move everything open to the backlog (`atm move … --to backlog`) and plan afresh. |

## 9. Implementation order

How this design gets built in this repository. It is an order of work by dependency, not a plan of sprints: how
many sprints that takes, and what goes in each, is decided in planning (decision 15). Until sprints exist, the tasks
are created in **phase 27**, the only grouping atm has today; the migration turns it into a sprint like every other
phase, and from then on `/atm:plan-sprint` moves the remaining tasks into sprints as it goes.

| Step | Builds | Needs | Requirements |
|---|---|---|---|
| **1. Sprints in atm-cli** | the data model and merge for sprints, sprint names, `atm sprints`, `atm move`, `atm backlog`, selections in `atm next`, `--sprint` everywhere; the migration in `atm migrate`, with the dashboard reading phases as sprints so nothing breaks before step 2 | — | SR-2.1, 2.3–2.6, 2.9, 3.1–3.6, 8.1, 8.3–8.6 |
| **2. Sprints on the dashboard** | Sprint filters, the sprint header, Group by sprint, the sprint table, burndown and forecast per sprint; phases removed from the dashboard | 1 | SR-7.1–7.4, 7.6–7.8 |
| **3. The sprint workflow** | `/atm:plan-sprint`, `/atm:close-sprint` (with the end-of-sprint review step), `atm sprints close` with carry-over and report; the loop's selections and stopping; create-tasks proposing sprints | 1 | SR-4.1–4.3, 4.5, 5.1, 5.2, 5.5 |
| **4. Releases** | `meta.releases`, `atm releases` (with notes), `atm merged` releasing, `/atm:push` suggesting branches, the release view; this repository's releases recovered from its PRs | 1 (dashboard part: 2) | SR-2.2, 3.7, 5.6, 7.5, 8.2 |
| **5. Reviews and working together** | components, review modes and states, `approve` and `request-changes`, `sprint_review` and its step in closing, `with: user`, rework to the same agent; the `interactive` label migrated | 3 | SR-2.7, 3.8, 3.9, 4.4, 5.3, 5.4, 6.1–6.6 |
| **6. Resolution and relations** | `resolution`, `related`, `duplicate_of`, their CLI options and dashboard display | 1 | SR-2.8, §3.4 (`skip`, `done`) |

Steps 2, 3, 4 and 6 can go in parallel once step 1 is in; step 5 needs the closing workflow of step 3.

**This repository's migration** happens when step 1 ships: phases 0 to 27 become sprints named after them (closed
where finished, planned otherwise), and the first `/atm:plan-sprint` (step 3) chooses what goes in the first sprint
from them, including the open tasks of phase 24 (T-277, T-279) and the CompMD work (phase 26). The tasks for all six
steps are created with `/atm:create-tasks` once this design is approved, with their dependencies as in the table.

**Docs:** the v4 documents are updated where they define phases ([data model](v4/data-model.md) §2, the
[atm-cli](v4/atm-cli.md) commands, the [skills](v4/skills-and-workflows.md)), `plugin/CLAUDE.md`, Help and the
README, in the sprint that changes each.

**Security** ([security-architecture.md](security-architecture.md)): the new fields agents act on (`with`,
`review`, `review_state`, `sprint_review`) and the new skills change what agents do from task data, so steps 3 and 5
run the agent sandbox and update that document; no new HTTP route or CI permission.

## 10. Questions for review

Settled with Hakan on 2026-10-30:

1. **Automatic migration:** yes, in `atm migrate` (decision 11).
2. **Sprint IDs:** names, generated when none is given: a historical person's name, or the creation time (decision
   12, SR-2.9).
3. **The end-of-sprint review:** a step of `/atm:close-sprint`, not a task (decision 4, SR-4.4).
4. **The number of sprints:** project-dependent and decided in planning; this design's own work is an implementation
   order (decision 15, section 9).
