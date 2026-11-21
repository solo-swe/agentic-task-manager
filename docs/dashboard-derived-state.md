# The dashboard's own rules, and moving them behind atm's API

ATM-716, 2026-11-21. A scan of `dashboard/src` for every place that works out what task, review, sprint, claim or
project data *means* (states, waiting, readiness, timers, counts, progress, what may happen next) instead of showing
what atm computed. Each rule should live once, in atm-lib, reach the dashboard through the REST routes and the app's
commands, and leave the dashboard with presentation only: formatting, layout, charts' geometry, display order.

## 1. Why

ATM-711's card showed **Approved** (CR-106's decision) and, under it, **Waiting for review · 1 h 21 m**. Two rules
decided the two lines:
- the chip reads `review_state`;
- the line is `reviewWait` (`lib/waiting.ts:25`), which checks only that the status is review-kind and counts from
  `review_requested_at`.

atm's own rule (`summary.rs:770`, the needs-you count) knows that a task approved or sent back is waiting on its
agent, not on the reviewer. So does `needsYouReason` in the same file as `reviewWait`. Three copies of one rule, and
one of them wrong.

The scan finds this pattern throughout. The dashboard parses `meta` again, works out kinds about 40 times, rebuilds
the sprints from `meta.sprints`, computes work time, progress and forecasts, and re-derives review actions the server
already answers. atm has copies of some of these too, one of them (`summary.rs` `work_span`) commented "as the
dashboard counts it".

## 2. What atm gives the dashboard today

| Source | What |
|---|---|
| `api/v1/projects/<id>/tasks` (`query.rs` `task_views`) | each task with `derived`: `kind`, `ready`, `waiting_for`, `waiting_on`, `priority`, `effective_review`, `effective_release`, `stage`, `archived`, `problems`, `foreign`, `review_statuses` |
| `api/projects` (`summary.rs`) | per project: counts by kind, `needs_you` and `needs_you_tasks`, `working`, `next`, `last_done`, `stats` (started, tasks, agent minutes), `board`, the active sprint's and recent sprints' done/total/percent, `last_activity`, `git` |
| `…/reviews`, `…/reviews/<r>` (`review_pages`) | the list with `needs_you`, `waiting`, `open`; a review's page with `needs_you`, `you` and `can` (`lifecycle::can`: approve, request changes, comment, round, close, each ok or why not) |
| `…/claims`, `…/worktrees` | the live claims snapshot; the worktrees with their changed tasks as views |

Not served: the sprints (with their counts, carried and added), the releases (their counts and notes), the project's
stats beyond the summary, the resolved vocabulary, and anything about time spent.

## 3. The inventory

Severity: **wrong** (gives a wrong answer today), **drift** (agrees today, but is a second copy of atm's rule),
**rule** (a rule atm doesn't have yet, which belongs there).

### A. Who a task waits on, and since when

| Where | What it decides, from | atm today | Severity |
|---|---|---|---|
| `waiting.ts:25` `reviewWait`; shown by `KanbanCard`, `TaskCard`, `DetailPanel` | "Waiting for review · …" for any review-kind task, from `review_requested_at` | nothing per task; `summary.rs:770` has the right rule for needs-you | **wrong**: an approved or sent-back task is waiting on its agent (ATM-711) |
| `KanbanCard.svelte:68`, `DetailPanel.svelte:246` | the review-state chip from raw `review_state` | — | drift: the chip and the waiting line can contradict each other (as on ATM-711) |
| `waiting.ts:93` `needsYouReason`, `needsYouItems` | needs-you per task: review not sent back or approved, blocked on the user, together and ready | the same rule in `summary.rs:767`, only per project (`needs_you_tasks`) | drift: two copies |
| `waiting.ts:53` `blockWait` | blocked on whom, since `blocked_at`, the questions | `derived.waiting_on` | drift |
| `forecast.ts:132` `NO_AGENT_WORK` | review-kind tasks need no agent work | — | **wrong**: approved and sent-back tasks are left out of the remaining work, so the estimate is too short |
| `burndown.ts:134` | "waiting" = blocked or review | — | **wrong**: the same class |
| `component-progress.ts:47` | open / in review / blocked by kind | — | **wrong**: a sent-back task is counted as in review, not open |
| `timeuse.ts:80` `waitingIntervals` | time waiting on you: each review to now | — | **wrong**: an approved task still counts as waiting on you until done |
| `TaskCard.svelte:38`, `KanbanCard.svelte:42`, `DetailPanel.svelte:80` | "its code review" = `code_reviews.at(-1)` | `derived.review_statuses` (statuses only) | drift: the latest linked review isn't always the one the task waits on |

### B. Status kinds and the vocabulary

| Where | What | atm today | Severity |
|---|---|---|---|
| `kindOf(vocab, status)`, about 40 calls (`scope`, `insights`, `forecast`, `burndown`, `velocity`, `kanban`, `sprints`, `releases`, `timeline`, `cardfacts`, `stages`, `work`, `relations`, `worktrees`, `component-progress`, `agents`, `timeuse`, `overview-data`, `projectstats`, `projectinfo`, the cards, the panel, the graph, `ScopeBar`) | a status's kind | `derived.kind` on every task | drift |
| `vocab.ts:79` `resolveVocabulary` | parses `meta.statuses`, `types`, `priorities`, `default_priority`, `labels`, `phases` again; a list it can't use is replaced by the defaults | atm-common `Vocabulary`, strict; a tolerant read reports problems (T-377) | drift: a vocabulary atm reports as broken is shown as the defaults |
| `vocab.ts:146` `priorityOf`, `priorityRank` | the default priority, and the middle one when none is named | `derived.priority` | drift |
| `projectinfo.ts:93` `workflowOf`, `components.ts:15`, `stages.ts:71` `workflowType` | parse `meta.workflow` and `meta.components` | atm-lib reads both | drift |
| `overview-data.ts:111` `mergedTasks` | rewrites each status to the default vocabulary's status of the same kind, so charts can mix projects | — | drift: a workaround for having no kind on the task |

### C. Stage, release, resolution and links

| Where | What | atm today | Severity |
|---|---|---|---|
| `stages.ts:24` `stageOf` (filters, releases, chips) | done / in PR / merged | `derived.stage` (`query.rs:243`) | drift |
| `sprints.ts:109` `effectiveRelease` (panel, filters, releases) | its own release, else its sprint's; atm's when present, its own rule otherwise | `derived.effective_release` | drift: the fallback is a second copy |
| `relations.ts:16` `resolutionOf`, `notDone` | the default resolution (done; won't do for skipped) | atm's `show` prints it | rule |
| `relations.ts:31` `relationsOf`; `DetailPanel.svelte:87` `requiredBy` | related both ways, duplicated by, required by | — | rule: reverse links belong with the task's view |

### D. Work time and people

| Where | What | atm today | Severity |
|---|---|---|---|
| `work.ts:65` `workSpan`, `taskTimes` | work = `started_at` until `review_requested_at`, else `completed_at` or `updated_at`, or until now while active | the same rule, copied, in `summary.rs:495` | drift: two copies. Both see only the last attempt and the last review request, so a task reviewed in rounds or taken back is measured from its last start |
| `cardfacts.ts:24` `cardFacts` | which moment a card shows (done / skipped / started / created) and "took" | — | rule |
| `projectstats.ts:146` `timeHistogram`, `forecast.ts:91` `taskDurations`, `insights.ts` cycle times | each done task's minutes | — | drift: three more uses of the span rule, each slightly different (with and without the gap fallback) |
| `agents.ts:49` `agentTimeline`, `timeuse.ts:101` `timeUse`, `projectinfo.ts:44` `actors` | agent lanes, parallel agents, busy time, people's counts, rules for files from before v4 | `summary.rs:516` `agent_minutes` | drift |
| `actors.ts:7` `isAgent` | an actor starting `claude:` is an agent | `edits.rs:233` `is_agent`; `summary.rs:520` repeats the literal | drift |
| `actors.ts:12` `approvedBy`, `:33` `earlierAttempts` | who finished it, if not who started it; earlier workers from handoff and failure notes | — | rule |
| `forecast.ts:17` `INTERACTIVE_LABEL`, used by `projectstats`, `insights`, `forecast` | "done with you" = the label `interactive` | the field `with: user` (needs-you, the cards) | **wrong**: two notions of one idea; a `with: user` task without the label counts in the agents' pace |

### E. Sprints and releases

| Where | What | atm today | Severity |
|---|---|---|---|
| `sprints.ts:37` `readSprints`, `:76` `sprintModel` | parses `meta.sprints`; an unknown state counts as planned | atm-common's sprints | drift |
| `sprints.ts:57` `sprintsFromPhases` | the phases-to-sprints migration's rule, for an unmigrated file | `atm db migrate` | drift: the migration runs before any server serves a project |
| `sprints.ts:166` `sprintSummary`, `:125` `addedMidSprint`, `:187` `carriedCount` | a sprint's done, total, added, carried, due-in, sprint review pending | `summary.rs:66` done/total/percent (active and recent only); the close report (done, carried, added) once closed | rule, partly drift |
| `insights.ts:43` `share` vs `summary.rs:77` | a group's completion: no tasks gives 0 % here, 100 % in atm's summary | — | **wrong**: the scope bar and the overview's card disagree for an empty sprint |
| `sprints.ts:133` `daysUntil` vs `burndown.ts:83` `endOfDay` | when a due date passes: the viewer's local day vs the end of the day in UTC | — | **wrong**: two rules for one date |
| `scope.ts:80` `leftovers`, `scope.ts:41` `lastClosed` | open tasks in closed sprints; the last closed sprint | `open_in_closed_sprints` (`archive.rs:168`, in `atm project status`) | drift |
| `velocity.ts:23` | done per closed sprint: its report, else its tasks done now | the close report | drift: the fallback counts a closed sprint's tasks as they are now |
| `projectinfo.ts:119` `vocabCounts` | the backlog's size, finished tasks included | `sprints.rs:160` `in_backlog`: unfinished only | **wrong**: the Project tab's backlog count differs from `atm sprint list` |
| `releases.ts:36` `readReleases`, `:92` `releaseSummaries`, `:58` `releaseNotes` | releases parsed, their counts by stage, the notes draft | `releases.rs:525` `release_notes` (`atm release notes`) | drift: a second notes writer |
| `filters.ts:174` `sprintOptions`, `projectinfo.ts:132` | the order sprints are listed in (active, planned, backlog, closed newest first) | `atm sprint list` order | drift |

### F. Progress and forecasts

| Where | What | atm today | Severity |
|---|---|---|---|
| `insights.ts:63` `progressBreakdown`, `projectstats.ts:37` `projectStats` | overall progress and stats; archived tasks not loaded yet count as done | summary `stats`, counts by kind | **wrong** while the archive isn't loaded: an archived skipped task counts as done |
| `insights.ts:83` `byPhase` | progress by `phase` | phases are sprints since `atm db migrate` | drift: a dead grouping |
| `forecast.ts:139` `forecastSprints`, `pace`, `nextInLoop` | pace from the last ten durations, remaining work in the loop's order (active, planned, backlog) | the loop's order is `atm loop next`'s | rule (the model) and drift (the order) |
| `burndown.ts:88` | scope from `created_at`, not from when a task joined the sprint (`sprint_history`) | — | rule |
| `overview-data.ts:132` `projectProgress`, `:169` `velocityByPeriod` | the overview's Progress tab loads every project's tasks and recomputes progress, sprints and releases | the cards beside it use the summary | **wrong**: two sources for one number on one page; `velocityByPeriod` reads `completed_at` only, `timeline.ts` falls back to `updated_at` |

### G. Reviews: what may happen next

| Where | What | atm today | Severity |
|---|---|---|---|
| `ReviewPage.svelte:160` `ended`, `live`, `sentBack`, `settled`; `:609` `approveBlocked` fallback | which buttons show: Submit, Close, Publish comments, Approve | `can` on the page (`page.rs:130`), used only for approve and request changes, with the old rules as fallbacks | drift: close and comment are still shown by the page's own rule |
| `ChainBox.svelte:74` `editable` | a comment is editable by its author, not a poke | — | rule |
| `reviews/page.ts:30` `groupReviews` | the list's groups from status, `needs_you`, `open` | `needs_you`, `waiting`, `open` are atm's | mostly presentation; "Sent back" and "Waiting on others" are status rules |

### H. Claims and worktrees

| Where | What | atm today | Severity |
|---|---|---|---|
| `claims.ts:26` `parseClaims` | drops claims whose lease has run out, by the viewer's clock | the snapshot as atm wrote it | drift: a lease rule, on another clock |
| `worktrees.ts:29` `PROGRESS`, `mergeWorktrees` | which worktree's copy "got further": todo < blocked < active < review < done | the merge driver's `kind_rank` (`merge.rs:43`): active = blocked = review | drift: two rankings for the same question |

**Pure presentation**, checked and kept:
- `format.ts`, `timeline.ts` `formatDate`;
- `chart.ts`, `graph/layout.ts` (edges from `dependencies`, geometry only);
- `kanban.ts` columns and their display order (readiness and next-up are atm's already);
- `allboard.ts` (atm's board);
- `projects.ts` (sums of summary fields, the overview's "active" filter);
- `notes.ts` labels;
- `stages.ts` `doneSections`;
- `list-groups.ts`;
- `filters.ts` matching (on values atm gives, once C and E are done).

## 4. The proposal

### 4.1 One rule, one place

Each rule moves into atm-lib, as a function with its own table of cases (the testing conventions: every state
combination, not a coverage number). Its answer goes out on the task's view, a project's routes, or the summary. The
REST route and the app's command both return it from the same call (I27). The dashboard reads fields and shows them.

### 4.2 On every task's view (`derived`)

| Field | What | Replaces |
|---|---|---|
| `waiting`: `{ on, since, why? }` | who the task waits on now:<br>• `reviewer` while its review waits (since the request);<br>• `agent` once approved or sent back (since `reviewed_at`);<br>• `user` or `agent` when blocked (since `blocked_at`, with the questions);<br>• `dependencies` when to do and not ready;<br>• none otherwise | `reviewWait`, `blockWait`, the review-state chip's rule (A) |
| `needs_you`: reason or none | review, blocked, together; the summary's `needs_you_tasks` is built from this field | `needsYouReason`, and the copy in `summary.rs` |
| `agent_work`: bool | whether agent work is left: to do, active, sent back or approved but not finished | `NO_AGENT_WORK`, burndown's and the component table's splits |
| `review`: `{ id, ref, status }` | the code review it waits on, not the latest link | `code_reviews.at(-1)` |
| `work`: `{ minutes, live, spans }` | its work, one rule over its attempts and rounds (`reviews`, `blocks`, handoffs) | `workSpan` and its four variants, `summary.rs` `work_span` |
| `resolution`, `required_by`, `related_by`, `duplicated_by` | how it ended; the links pointing at it | `resolutionOf`, `relationsOf`, `requiredBy` |
| `in_closed_sprint`: bool | open in a closed sprint | `leftovers` |

`kind`, `priority`, `stage` and `effective_release` are there already: the dashboard stops computing them, and the
archived tasks and worktree copies come with them as the others do.

### 4.3 New project routes, and app commands

| Route | What | Replaces |
|---|---|---|
| `…/vocabulary` (or in the tasks response) | the vocabulary as atm resolved it: statuses with kinds and labels, types, priorities in order with the default, labels, components, workflow; with its problems | `resolveVocabulary`, `workflowOf`, `projectComponents` |
| `…/sprints` | each sprint in atm's order:<br>• its state;<br>• done, total and percent, by one rule;<br>• added, carried, sprint review pending;<br>• its due date as a date, and days left by one rule;<br>• its close report;<br>• the backlog's size, as `atm sprint list` counts it | `sprintModel`, `sprintSummary`, `velocity`, `vocabCounts`' sprints and backlog, `lastClosed` |
| `…/releases` | each release with its sprints, counts by stage, and its notes (`release_notes`) | `readReleases`, `releaseSummaries`, `releaseNotes` |
| `…/stats` | the project's stats, the time histogram, people and agents, the agents' lanes, the pace and the forecast per sprint in the loop's order | `projectStats`, `timeHistogram`, `actors`, `agentTimeline`, `timeUse`, `forecastSprints`, `progressBreakdown` |

The overview's Progress tab reads the summaries (`api/projects`, extended with what the tab shows) instead of every
project's tasks, so a card and the tab beside it show one number.

### 4.4 Reviews

The page shows a control only when `can` allows it, for every action: approve, request changes, comment, round, close.
The fallbacks go. Each comment comes with `editable`.

### 4.5 Claims and worktrees

- The claims route leaves out an expired lease, by atm's clock.
- The worktrees route says which copy is shown and from which branch, using one ranking shared with the merge driver.

### 4.6 What the dashboard keeps

- Formatting and words: "2 h 10 m", "due tomorrow", the labels for atm's values.
- Layout, charts' geometry, colours.
- Display order.
- Filtering and search over the fields atm gives.

A guard test (like `tokens.test.ts`) fails when `dashboard/src` reads a raw state field to decide something:
`kindOf(`, `.review_state`, `.started_at`, `.completed_at`, `.review_requested_at`, `.blocked_at`, `meta.sprints`,
`meta.statuses` and so on. The few files that only show these fields are on an allow-list.

## 5. The order

Each step is one task in this sprint: atm-lib's rule with its table of cases, the route's or summary's field with its
route test, the app's command, then the dashboard switched over with its old rule deleted.

1. **Waiting and needs-you** (`waiting`, `needs_you`, `agent_work`, `review`; A). This fixes ATM-711's card and the four
   wrong counts that depend on it. **P0.**
2. **Kinds and the vocabulary** (B): the dashboard reads `derived.kind` and atm's vocabulary, and `vocab.ts`'s parser
   goes. **P1.**
3. **Stage, release, resolution, links** (C): mostly deleting fallbacks; the reverse links are new. **P2.**
4. **Sprints and releases** (E): the two routes, the empty-sprint and backlog disagreements, one due-date rule.
   **P1.**
5. **Work time** (D): one span rule over attempts and rounds, `derived.work`, and the "with you" question below.
   **P1.**
6. **Stats, progress and forecasts** (F): the stats route, the overview's Progress tab from summaries. **P2.**
7. **Reviews' actions** (G): `can` for every control. **P1.**
8. **Claims, worktrees and the guard test** (H, §4.6). **P2.**

**Tests:**
- Each rule is atm-lib's unit test, a table over its states, the way the task table is (ATM-713).
- The routes' tests check the fields.
- The dashboard's unit tests give components the API's fields and check what they show; its rules' tests go with
  the rules.
- The e2e fixture already takes its views from the binary, so the specs see atm's answers.

## 6. Questions for you

1. **"With you":** one notion? Proposed: `with: user` decides, and the `interactive` label is migrated to it once.
2. **The forecast:** in atm, so the CLI could say "the sprint finishes in about 2 h"? Proposed: yes, in `…/stats`. The
   dashboard only draws it.
3. **A broken vocabulary:** the dashboard has shown the defaults where atm reports a problem. Proposed: it shows what
   atm serves, with atm's problems in the footer, as a malformed task file is shown now.
4. **Work over several attempts:** count every attempt and every round, or only the last, as both copies do now?
   Proposed: every attempt and round, from `reviews`, `blocks` and the start and lapse notes.
