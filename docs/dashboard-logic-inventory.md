# What the dashboard computes that atm-lib should (T-435)

The [Svelte guide](guides/svelte.md#where-logic-lives) puts validation, queries, ordering, search and what atm derives
from tasks in atm-lib, served through the v1 API ([layered-architecture.md](layered-architecture.md) §6.2), and keeps
the UI to showing data and sending intent. This is where `dashboard/src/lib/` stands against that, module by module
(as of 0.31.0), and the order proposed for moving what should move. It is a proposal for review: the follow-up tasks
are created from it once it is agreed.

## 1. What the server sends today

| Route | What atm-lib computes for it |
|---|---|
| `api/v1/projects/<p>/tasks` (`task_views`) | per task: `kind`, `ready`, `waiting_for`, `waiting_on`, `priority` (or the default), `effective_review`, `effective_release`, `stage`, `archived`, `problems`; filters as `atm list` takes them |
| `api/projects` (`project_summary`) | per project: counts and percent, the active sprint's numbers, recent sprints, who is working, next up (loop order), last done, `needs_you` and `needs_you_tasks`, git branch and PR, `board` and `board_active_sprint` (cards per column, in atm's order), `stats` (start, task count, `agent_minutes`) |
| `api/search?q=` (`search_tasks`) | matching task IDs per project |
| `p/<id>/claims.json`, `p/<id>/worktrees.json` | the live claims; each worktree's copies as task views |
| `api/v1/projects/<p>/documents`, `…/files` | the document viewer's files |

The rest of §6.2's v1 routes (`projects/<p>` with the vocabularies and settings, `status`, `sprints`, `releases`,
`components`, `next`, `claims`) are designed but not built. The dashboard still reads `meta` from the task file for
everything they would carry.

## 2. Module by module

**Moves**: domain rules computed again in the dashboard. **Uses atm's**: takes the server's values already (keep).
**View**: presentation, state of the page, or the client layer (stays).

### Rules atm-lib already has, computed a second time

| Module | What it computes | atm-lib has it | Proposal |
|---|---|---|---|
| `stages.ts` `stageOf` | a done task's stage from `pr` and `merged_at` | `derived.stage` (`task_stage`) | **Moves**: read `derived.stage`; keep `doneSections` (grouping cards by PR: view) |
| `vocab.ts` `kindOf` on a task's status, `work.ts` `isDone` | a status's kind | `derived.kind` | **Moves** for tasks: read `derived.kind`. `kindOf` stays for statuses that aren't a task's (a filter value, a column) until the vocabularies come from the server (step 2) |
| `vocab.ts` `priorityOf` | a task's priority or the default | `derived.priority` | **Moves**: read `derived.priority` |
| `waiting.ts` `needsYouReason`, `needingUser`, `needsYouItems` | why a task needs the user (review, blocked on the user, together now) | `needs_you_tasks` in the summary (`project_summary`) | **Moves**: a `derived.needs_you` per task (the reason, or none), so the project page, the chips and the summary agree by construction. `reviewWait`, `blockWait` (how long, what was said: display of stored fields) stay |
| `vocab.ts` `resolveVocabulary` | the statuses, types, priorities and default from `meta`, with atm's defaults | `vocabulary()` (`common/model.py`) | **Moves** with the `projects/<p>` route (step 2): the dashboard gets the vocabularies as atm resolved them, including atm's refusal of an unusable one (today the API sends the fallback) |
| `sprints.ts` `sprintModel`, `readSprints`, `sprintsFromPhases` | the sprints from `meta.sprints`, or made from `phase` for a file never migrated | `vocab["sprints"]`; `atm migrate` turns phases into sprints | **Moves** with `projects/<p>` (step 2); reading phases goes (`atm migrate` has run for every project the server lists) |
| `releases.ts` `readReleases`, `components.ts` | `meta.releases`, `meta.components`, read leniently | `vocab["releases"]`, `vocab["components"]` | **Moves** with `projects/<p>` (step 2) |
| `stages.ts` `workflowType`, `projectinfo.ts` (how it pushes) | `meta.workflow` | `get_workflow` | **Moves** with `projects/<p>` (step 2) |
| `actors.ts` `isAgent` | an actor starting with `claude:` is an agent | `is_agent` (`common/model.py`) | **Moves** where it decides counts (agent work, stats); a `derived` flag per actor field is overkill. Proposal: `derived.started_by_agent` and `completed_by_agent`; the chip text stays |
| `timeline.ts` `completedAt` | a done task's finish: `completed_at`, else `updated_at` for old files | the same fallback in `project_stats` | **Moves**: `derived.completed_at` |
| `work.ts` `workSpan` | a task's agent work: started until review or done, or to now | `work_span` (`lib/query.py`) | **Moves**: `derived.work` (`start`, `end`, `live`), computed with the server's clock; `taskTimes` (the details panel's rows) stays as view |

### Rules with no atm-lib counterpart yet

| Module | What it computes | Proposal |
|---|---|---|
| `sprints.ts` `sprintSummary`, `addedMidSprint`, `carriedCount`, `daysUntil` | a sprint's counts by kind, what was added mid-sprint or carried | **Moves** to the `sprints` route (step 3), which `list_sprints` already answers for `atm sprints`; `dueText` stays (wording) |
| `releases.ts` `releaseSummaries`, `releaseNotes`, `splitReleases` | a release's tasks by kind and stage; its notes | **Moves** to `releases` and `releases/<r>/notes` (step 3): `list_releases` and `release_notes` exist for the CLI |
| `component-progress.ts` | each component's progress | **Moves** to `components` (step 3): `list_components` counts already |
| `velocity.ts` | tasks done per closed sprint, from its report | **Moves** to `sprints` (step 3): the reports are atm's |
| `list-groups.ts` | the List tab's groups: the active sprint, the planned ones in order, the backlog | Order: **moves** with `sprints` (atm's loop order, SR-7.4); grouping: view |
| `scope.ts`, `sprint-hash.ts` | which sprint the Kanban, List and Graph show, and its URL | **View** (the page's selection), on the sprints the server lists |
| `filters.ts` `filterValues`, `filterTasks`, `facetCounts`, `sortTasks`, `searchTasks` | what each filter value means for a task (its sprint or the backlog, its release, its components), which tasks match, the counts beside each value, the List's sort, the search box | **Decide in review.** The guide puts queries and search in atm-lib, and the v1 tasks route filters as `atm list` does. But filtering the loaded tasks is instant, and a round trip per click isn't. Proposal: the **meaning** moves (each task's `derived` carries what a filter compares: sprint id or backlog, effective release, components, priority), so the client only compares values; matching, counting and sorting the loaded list stay client-side; the search box uses `api/search` (atm's matching) and keeps the loaded order |
| `kanban.ts` | a column per status of the vocabulary, cards in atm's order (`nextUp`), readiness from `derived` | Columns follow the vocabulary (step 2); the order is atm's already. **View** otherwise |
| `forecast.ts` | completion estimates: pace from recent work time, remaining work in loop order | **Moves** (step 4): an estimate is a query over atm's data with atm's loop order; one answer for the dashboard and a future `atm status --forecast` |
| `burndown.ts` | remaining tasks and scope over time, with the forecast | **Moves** (step 4): the series; the chart's geometry stays |
| `insights.ts` | completion shares (overall, by phase, by type), status counts, gaps between completions, medians | Shares and counts **move** (step 4: `status_report` has `by_status`, `by_type` already); by phase goes with phases |
| `timeuse.ts` | time split into agents working, waiting on you, idle | **Moves** (step 4): built on work spans and blocks, which atm records |
| `agents.ts` | one lane per agent, a bar per task's work | Spans **move** with `derived.work` (step 1); lanes and bars are view |
| `projectstats.ts` | the project's numbers over its history | **Moves** (step 4): `project_stats` computes part of it already (start, task count, agent minutes); the rest (duration, commits, PRs, sprints closed) joins it, so the summary and the Project tab show one set of numbers |
| `projectinfo.ts` | who worked on the project, how it pushes, where its files are | Who worked **moves** with the stats (step 4); the rest with `projects/<p>` (step 2) |
| `overview-data.ts` | the overview's Agents, Activity and Progress across projects | Follows step 4: each project's series from the server, combined here (view) |
| `cardfacts.ts` | what a List card shows: who, when, how long | **View**, on `derived.work` (step 1) |
| `relations.ts` | how a finished task ended (`resolution`), what it relates to | **View** of stored fields; `atm check` validates them |
| `graph/layout.ts` (`dependencyChain`) | every task a task waits for, directly or through others, for highlighting | **View**: the graph's own traversal over the dependencies it draws; `tasks/<t>/deps` (direct only) wouldn't save a request |

### Uses atm's values already (keep as is)

`kanban.ts` readiness (`derived.ready`, `waiting_for`), `waiting.ts` together-now (`derived.ready`), `review.ts`
(`derived.effective_review`), `sprints.ts` `effectiveRelease` (`derived.effective_release`), `allboard.ts` (the
summaries' boards), `archive.ts` (the server's archive hint), `worktrees.ts` (each worktree's views), `load.ts` problems
(`derived.problems`).

### View, page state and the client layer (stay)

`api/http.ts`, `load.ts`, `claims.ts` (load the claims), `state.svelte.ts`, `view-store.ts`, `prefs.ts`, `routes.ts`,
`hash.ts`, `tabs.ts`, `dialogs.ts`, `appearance.ts`, `theme.ts`, `icons.ts`, `scroll.ts`, `chart.ts`, `format.ts`,
`time-window.ts`, `plain-text.ts`, `url.ts` (link safety, I-series invariants in
[security-architecture.md](security-architecture.md)), `compmd.ts` and `documents.ts` (rendering is CompMD's; the
viewer's routes are the server's), `notes.ts`, `graph/view.ts`, `changelog.ts`, `release-notes.ts`, `plugin-info.ts`
(build time), `types.ts` and `generated/`.

## 3. Proposed order

Each step leaves the dashboard working, with the unit tests of the moved rules moving to `tests/lib/` (atm-lib) and
the e2e fixture (`tasksServer`) serving the new fields.

1. **Use the derived values that exist; add the per-task ones.** Read `derived.stage`, `kind` and `priority`; add
   `derived.needs_you`, `completed_at`, `work`, `started_by_agent`/`completed_by_agent` to `task_views`. No new route.
   Removes `stageOf`, task `kindOf`/`isDone`, `priorityOf`, `needsYouReason`, `completedAt`, `workSpan`, `isAgent` from
   the counting paths.
2. **`api/v1/projects/<p>`**: the project's settings and vocabularies as atm resolves them (statuses with kinds, types,
   priorities and default, sprints, releases, components, workflow). Removes `resolveVocabulary`, `sprintModel`'s
   reading of `meta` (and of phases), `readReleases`, `components.ts`'s reading, `workflowType`.
3. **`sprints`, `releases` (and `notes`), `components`**: the routes `atm sprints`, `atm releases` and
   `atm components` already answer for the CLI. Removes `sprintSummary`, velocity, `releaseSummaries`, `releaseNotes`,
   component progress, and the List's sprint order.
4. **Activity and progress**: an `api/v1/projects/<p>/activity` (or a richer `stats`) with the series the charts draw:
   time use, burndown, forecast, completion shares, the project's stats. The largest step, and the one with the most
   tests to move; it can be split by chart.
5. **Filters and search**, as decided in review: `derived` carries what each filter compares, the search box asks
   `api/search`.

Every new route or field is an entry point or a field the dashboard acts on: each step updates
[security-architecture.md](security-architecture.md) and [multi-project-api.md](multi-project-api.md) in the same
change.
