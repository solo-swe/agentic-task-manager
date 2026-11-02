# Changelog

All notable changes to the atm plugin and its task dashboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).
The dashboard comes with the plugin: updating the plugin updates it everywhere.

## [0.26.0] - 2026-11-01

### Added
- **Graph auto-fit** (T-341): *Auto* (`#graph-auto-fit`, on by default, saved as `graphAutoFit`) refits the graph
  when the filters, data, Hide/Dim or panel size change; panning or zooming turns it off.
- **Fit All or Matches** (T-341): `#graph-fit-all` (F) and `#graph-fit-matches` (Shift+F, without the greyed-out
  tasks; disabled when the same). The last one pressed is what auto-fit fits (`graphFitTarget`). `fitBox`, `nodesBox`.
- **`atm commit <id> --record <rev>`** (T-343): records an existing commit on the task without committing, for a
  task whose commit a rebase or cherry-pick replaced; the revision must be a commit on the current branch. A hint
  when its message doesn't mention the task. `--file` is now checked by atm (needed unless `--record`).
- **Animated fits** (T-341): fits, zoom buttons and filter changes glide (280 ms, `interpolateView`); not with
  reduced motion.
- **`atm sprints delete <name> [--move-to S|backlog] [--force]`** (T-339): deletes a sprint created by mistake. An
  empty one goes at once, closed too; an open one's unfinished tasks move (backlog by default). Refused for a closed
  sprint that had tasks, while a task in it is in progress, and without `--force` for an open one with history.
  `/atm:plan-sprint` mentions it, for when the user asks.
- **Open tasks left in closed sprints** (T-330): `atm status` warns about them (`open_in_closed_sprints` in
  `--json`), `atm check` warns about each, and `atm reopen` of a task in a closed sprint suggests `atm move <id> --to
  backlog` or `--to active` (`hint` in `--json`), moving nothing. `/atm:plan-sprint` shows them first as candidates.
- **An Agents tab on the Projects Overview** (T-333, `#overview&view=agents`, remembered as `overviewTab`): every shown
  project's agents on one timeline, a lane per agent or person across projects naming the projects it worked in, a
  bar opening its task in its project. It reads each project's `tasks.json` and `claims.json` while shown
  (`lib/overview-agents.ts`; conditional on refresh), with its own zoom and Live. `AgentsTab` takes a timeline, view
  state and click handler. A project's own `#view=agents` no longer changes the overview's remembered tab.
  The overview's *Active only* is now *Hide inactive projects (N)*, the same setting, shown only when a project would
  be hidden; the separate "N inactive projects hidden" note is gone.
- **Activity on the Projects Overview** (T-328): the title bar's Activity chart and popup for every shown project
  together (statuses read by kind, task IDs as "Project: T-1"), with each project's count for the chosen period
  (opening the project), its own period, and a task opening in its project. The overview now loads every project's
  `tasks.json` and `claims.json` while shown (`lib/overview-data.ts`, which `lib/overview-agents.ts` became), rather
  than the summary growing; `ActivityWidget` takes the tasks, period state, a click handler and the breakdown.
- **A Progress tab on the Projects Overview** (T-329, `#overview&view=progress`): a row per shown project with its
  done of total, its active sprint (done of total, due date, goal on hover) and open releases, opening the project's
  own Progress tab; then the burndown of every project's tasks together (last 14 days or all time, with the forecast)
  and the tasks done per week over the last ten (the projects' sprints don't line up). `projectProgress`,
  `weeklyVelocity` in `lib/overview-data.ts`.

### Changed
- **The Omniboard shows the active sprints by default** (T-340): each project's active sprint's tasks, with their own
  counts and *+ N more*, from the new `board_active_sprint` in `api/projects` summaries (`null` without an active
  sprint; `atm summary` too). *All tasks* (`#board-scope`, remembered as `boardScope`) shows every task, as before; a
  note names the projects without an active sprint. An older server's summary falls back to the whole board.
- **Graph toolbar** (T-341, `.graph-toolbar`): grouped, with segmented switches and a compact zoom; under 920 px the
  hint hides and the toolbar wraps. e2e runs with `reducedMotion: 'reduce'`; `openDashboard(…, { graphAutoFit:
  false })` opens a project without auto-fit. The shipped file's size guardrail goes from 500 to 525 KiB (T-342 looks
  for what makes the file grow).
- **The tabs sit right under the title bar** (T-334), 12 px below the subtitle instead of 38 px (`.topbar`'s bottom
  padding and margin), on the overview and on a project's page alike; `.landing` lost its 24 px top padding.
- **One look for the tabs** (T-346): a project's view tabs look like the overview's, on a line with the selected tab
  open onto the page; the views no longer sit in a framed, filled panel (`.view-panels`), and their content lines up
  with the page's edges.
- **Richer Task List cards** (T-338, `TaskCard`, `lib/cardfacts.ts`): the description's first two lines and the
  latest outcome or handoff under the chips, and a column on the right with who created it, worked on it and (when
  someone else) finished it, when it was done, started or created, and its work time. One column under 760 px.
- **A new project card on the overview** (T-335, `ProjectCard`): some depth (a soft shadow, a light top edge, a lift on
  hover); a title band with the name on one line, a hairline under it and the 14-day sparkline at its right; what needs
  you as a badge on the card's top-right corner; the active sprint's ring marked; quiet labels (*Sprint*, *Now*,
  *Next*) and the last finished task led by its time; the stats line as a caption under the sprint line. The task at
  work (or next up) is a mini task card on a stack of tilted cards, one per other task in progress or ready (at most
  three, from the summary's board counts), with *+ N more*. A card with nothing to show centres its note.

### Fixed
- **The “needs you” list on a project card stays inside the window** (T-332): lined up with the chip's right edge, it
  ran off the left of the window on cards in the first column; `NeedsYouMenu` now shifts it back in when it opens.

## [0.25.0] - 2026-11-01

### Fixed
- **The sprint table lists the active sprint first** (T-323): the open sprints follow the loop's order (the active
  one, the planned ones, the backlog), as the table says, not the order they were added in; a sprint started after
  others were planned was listed after them. The Progress panel's sprint rings follow the same order.

### Added
- **`atm components`** (T-324): lists the project's components (`meta.components`) with their label, globs, review
  mode and how many tasks have them, archived ones too. **`atm components add <id> [--label T] [--path GLOB]...
  [--review MODE] [--from-label L] [--assign]`** defines one in a single locked write: `--from-label` turns a label
  into the component on every task, in `tasks.json` and `tasks-archive.json`, and drops it from `meta.labels` (its
  text becomes the component's label); `--assign` also gives it to every task whose files its globs match. For
  projects that marked their parts with labels before components existed. It refuses a component already there, a
  bad ID or glob, and a label no task has (a typo), writing nothing. `plugin/CLAUDE.md` lists it among the *Define*
  commands.
- **`atm components edit <id> [--label T] [--add-path GLOB]... [--review MODE] [--from-label L] [--assign]`**
  (T-326): changes an existing component the same way, so a second label can move onto it (`--label` and `--review`
  empty remove them); it refuses an unknown component, naming them.

### Changed
- **The scope bar replaces the sprint header** (T-323, SR-7.2): the Kanban, List and Graph open on the active sprint,
  and the bar above them (`#scope-bar`) says which tasks they show, kept in step with the Sprint filter. Its menu
  (`#scope-trigger`) picks one: the active sprint, a planned one, the backlog, all tasks, or one of the last five
  closed; several, picked in the filters, show as *Filtered: N selected*. It carries the one progress figure for its
  tasks, and *showing N, filtered* (with ✕ to clear the other filters and the search) when they narrow the board
  further. Hints name the skill to run next: close the sprint (`/atm:close-sprint`) once its work is done, with its
  release ready to ship and the backlog to pull from; settle what's in review or blocked first; start a planned
  sprint, or one at all when none is active (`/atm:plan-sprint`). A project without sprints has no bar. The Sprint
  filter now scopes only the Kanban, List and Graph: the Progress tab shows every sprint (it focuses one itself, by
  its rows), and the Agents tab greys out by the other filters alone.
- **The active sprint, whichever it is**: the Sprint filter's value for the active sprint is `:active`, so a saved
  view follows the project to its next active sprint; while none is active it selects nothing. New views and views
  saved before this version without a Sprint filter open on it (the saved view's `version` is 2).
- **Markers for sprint states**: ▶ the active sprint, ✓ a closed one, in the scope bar and its menu, the Sprint
  filter and the Kanban's sprint lanes.
- **The List groups by sprint: the planning view** (T-323): where it shows more than one sprint (All tasks, or
  several picked in the filters), a section per sprint in the order of the work: the active sprint, the planned ones
  in order, the backlog (with how to plan it, `/atm:plan-sprint`), then the closed ones folded into one row
  (`#list-closed-toggle`; open while a search or filter narrows the list, and in a project not migrated yet, whose
  finished phases are read as closed sprints). Each section's header has its tasks, how
  many are done and the sprint's goal. *Group* (`#list-group`) also groups by label or by component, as
  `/atm:plan-sprint` groups the backlog, or not at all; it is part of the saved view (`listGroupBy`,
  `listShowClosed`). Group and Order sit at the scope bar's end, like the Kanban's options.
- **The Progress tab opens the burndown on the active sprint** (T-323): its tasks, since it started, its due date
  marked; *Show all sprints* and the range buttons still widen it, and with no active sprint it shows them all. The
  focus is `:active` in the saved view (`progressSprint`), so it follows the next sprint; views saved before this
  version without a focus take it too.
- **Velocity** (T-323, `#velocity`): under the sprint table, a bar per closed sprint, the last ten in order, with the
  tasks it got done (from its report, else its tasks done), the average as a dashed line, and what it carried over or
  had added mid-sprint on hover.
- **Released releases fold into one row** (T-323, `#release-released-toggle`): the open releases first, then
  "✓ 19 released · last 0.24.0", newest first when opened, each with its release date. A release recorded without
  tasks (recovered from git history) shows its name, date and PR only, no empty counts or notes draft.
- **List cards name their sprint**, instead of the phase badge left from before sprints, when the list shows several
  sprints and doesn't group by them (`.task-card-sprint`, as on the Kanban).
- **Search and filters move into the title bar** (T-323): the filter row under the title is gone. A compact search
  box (`#task-search`; `/` focuses it, and it widens while in use) and a **Filters** button (`#filters-toggle`) sit
  with the charts; the button's badge (`#filters-badge`) counts the groups that narrow the board. It opens a popup
  (`#filter-panel`): how many tasks match (`#match-count`) and what is selected (`.filter-chip`), one line per group
  with its most used values first and the rest behind "+N" (Labels with a find box once there are more than ten), and
  *Clear filters* at its top (`#clear-filters`: the other filters and the search; the sprint stays, it is the scope
  bar's), and Include archived at its foot. *Reset view* (`#reset-view`), which forgets the whole saved view (filters,
  tab, each tab's options and zoom), moved to the settings menu as *Reset this project's view*. Values are quiet text
  chips with a muted count, a selected one tinted; there
  is no "All" chip any more: nothing selected is all, and a group with a selection has *clear* by its name. The search
  is a magnifier until used. Escape or a click outside closes it. The scope bar's note names what narrows
  the board: "showing 12, filtered: Type Feature · Labels ux ✕".
- **A ⋮ settings menu** (T-323, `#settings-btn`, `#settings-menu`) at the title bar's right end holds auto-refresh
  (on/off, the interval, when it last loaded), mode, theme and, on a project's page, *Reset this project's view*. The
  reload button stays beside it, showing the auto-refresh state; the separate refresh and appearance menus are gone.
- **The Kanban's toolbar row is gone**: its completion ring of the tasks shown repeated the scope bar's progress, and
  its options (*Group by sprint*, only when the board shows more than one sprint, and *Show skipped*) moved to the
  scope bar's end. A project without sprints keeps *Show skipped* above the board.
- **The sprint is in the URL** (T-323): `#sprint=active`, `#sprint=Foundations,backlog` or `#sprint=all` opens the
  board on those sprints (names in any case; unknown ones, like a renamed sprint in an old link, are dropped). Picking
  a sprint in the scope bar or the filters writes it, so the address shares what is shown and Back returns to the
  previous sprint; a link with it wins over the saved Sprint filter, and opening another project drops it.
- **Components by their labels** (T-323): cards (now List cards too), the detail panel, the Component filter and the
  List's groups show a component's label ("REST API") rather than its ID, in the project's order. The Progress tab
  has a **By component** table (`#component-table`) for the focused sprint, or all: done, progress and what is left
  per component, tasks in none last; a row filters by its component. The Project tab lists the components with their
  task counts and globs.
- **The Project tab shows sprints, not phases**: its Vocabulary has *Sprints* (the active one ▶, the planned ones,
  the backlog, the closed ones folded) and its Stats *N sprints · M closed*.
- **Planned and closed sprints** (T-323): a planned sprint's hint says it starts once the active one is closed
  (`/atm:close-sprint`, then `/atm:plan-sprint`); a closed sprint's board folds its empty columns into slim strips so
  Done gets the width, its cards in a grid, and its release reads "released" once shipped.
- **Open tasks left in closed sprints** (T-323): a task reopened after its sprint closed (or merged back open) is
  work no loop takes. The scope bar warns on All tasks and Backlog ("N tasks open in closed sprints", a link to those
  sprints) and on the closed sprint itself; the List keeps such a sprint unfolded; their cards' sprint badge is marked
  ✓ with a dashed outline. atm's side is T-330.
- **The title bar**: the pills on the right line up with the title, and a hairline separates the title row from the
  line under it.

## [0.24.0] - 2026-10-31

**Before the first run in a shared project, update every agent's plugin:** this version turns the project's phases
into sprints, and atm 0.23 or older reads a migrated `tasks.json` as tasks without phases (`atm next` would order by
priority alone, `--phase` would select nothing; nothing is lost or rewritten).

### Changed
- **The skills end tasks by their review mode** (T-315, SR-5.4, SR-4.4): `/atm:do-task` and `/atm:loop-tasks` run
  `atm done` and follow the mode instead of reading "a review is asked for" from descriptions: `none` and `batch`
  are done (a `batch` note is what the end-of-sprint review checks); `blocker` and `async` go to review, `blocker`
  first (the user reviews it now, or an unattended loop stops), `async` while the loop goes on. Reviewing a task
  can approve it or ask for changes (`atm approve`, `atm request-changes`). `/atm:close-sprint` starts with the
  end-of-sprint review of the tasks waiting for it (fixes live, follow-ups `related` in the next sprint), then
  closes with `--reviewed`.
- **`/atm:create-tasks` proposes sprints** (T-303, SR-5.5): for new tasks, the active sprint, a planned one, a new
  one (added with `atm sprints add`, planned, never started) or the backlog, instead of phases and phase names; tasks
  go in with `"sprint"` (or none, the backlog). It pre-approves `atm sprints` and `atm backlog` instead of
  `atm phases`. `atm add` no longer needs a phase.
- **Sprints in the detail panel, the header and the overview** (T-298, SR-7.6, SR-7.7, SR-4.5): the detail panel's
  phase badge is the task's sprint (or *Backlog*, `#detail-sprint`), with its effective release
  (`#detail-release`), and a *Sprint history* when it was in more than one, flagged once carried twice. The header
  counts sprints and the backlog ("12 tasks across 3 sprints · 2 in the backlog"). `atm serve`'s project summary adds
  `active_sprint` (with progress, due date and goal) and `recent_sprints` (up to four: the open ones, then the latest
  closed), and its `next` follows the loop's default selection (the active sprint); a project card with sprints
  shows rings for them and the active sprint's line ("Ada Lovelace — 1 / 3 · due in 3 days", or "No active
  sprint"). Projects without sprints lost their phase rings with T-320 (Removed, below).
- **Kanban: Group by sprint** (T-296, SR-7.3) replaces *Group by phase* (`#kanban-group-by-sprint`): one lane per
  sprint with tasks, in the project's order, the backlog last (`buildLanes` in `lib/kanban.ts`, lanes
  `data-sprint`). A card names its sprint (`.kanban-card-sprint`, coloured by the sprint's place in the list) only
  when the ungrouped board shows more than one; the "Phase N" badge is gone. The saved view's `kanban.groupByPhase`
  is `groupBySprint` (an old view's value is kept).
- **The loop and do-task work by sprint** (T-302, SR-5.1, SR-5.2): `/atm:loop-tasks` takes a selection, passed to
  every `atm next`: `--sprint S` (repeatable; replaces `--phase`), `--tasks T-1,T-5`, `--all`, `--backlog`; none
  means the active sprint, or every task in a project without sprints. It stops when its selection has nothing
  ready and says why; for a finished sprint, what it got done and what's left, and that `/atm:close-sprint` closes
  it. `--continue` goes on to the next planned sprint (`--sprint` of it); the loop never closes or starts a sprint.
  It pre-approves `atm sprints` to read them. `/atm:do-task` without an ID takes the next task of the active sprint.
  The Kanban's **Next up** follows the same selection (since T-321 it is `atm next`'s own answer, below).
- **Phases become sprints** (T-292, `docs/sprints-and-releases-design.md` SR-8.1): `atm migrate`, which every skill
  runs first, converts a project with phases once, in `tasks.json` and `tasks-archive.json`. Each phase with tasks or
  a name becomes a sprint named after it ("Phase n" when unnamed, " (phase n)" added to a taken name), in phase order;
  one whose tasks are all finished is `closed` (start, end and a report from its tasks), the others `planned`, none
  active (it says to plan one with `/atm:plan-sprint`). A task's `phase` becomes `sprint` with a `sprint_history`
  entry from its `created_at`; `phase` and `meta.phases` are removed. The files are staged, not committed, and
  `atm migrate --json` lists the new sprints as `sprints_from_phases`. A task with a phase merged into a migrated
  project joins its phase's sprint at the next `atm migrate`. `atm serve` migrates the projects it serves too.
- **Progress tab by sprint** (T-297, SR-7.4): the phase table is a sprint table (`SprintTable.svelte`, `#sprint-table`,
  rows `data-sprint`): one row per sprint with tasks, in the project's order, then the backlog; a sprint's state and
  due date next to its name; a closed sprint shows when it closed and its report (carried to, added mid-sprint), and
  two or more fold into one row. Estimates add up in the loop's order: the active sprint, the planned ones, the
  backlog (`forecastSprints`, `nextInLoop` in `lib/forecast.ts`; `forecastPhases`, `phaseRanges` and `activePhases`
  are gone). The burndown focuses on a sprint; its default range is since that sprint (or the one the loop works on)
  started; a closed sprint's chart ends where it closed, and the due date of an open one is marked. The saved view
  keeps the focused sprint (`progressSprint`; a saved `progressPhase` is dropped, a `phase` range reads as
  `sprint`). The Progress panel's *By phase* is *By sprint*: a ring per open sprint and the backlog, one chip for
  the closed sprints.
- **The Kanban's Next up comes from atm** (T-321): the project's Kanban marks the first task of its summary's `next`
  in `api/projects` instead of applying the loop's rule itself, so it is always the task `atm next` hands out, also
  when the browser can't see why another isn't (a claim, a task done or waiting in another worktree). The summary's
  `next` now follows `atm next` exactly, tasks back from review first (`loop_order` in `claims.py`, shared by both).
  The dashboard's `nextUp()` and `next-up-parity.test.ts` are gone; the To Do column orders ready tasks by priority,
  then ID, for reading only.
- **Releases recovered from past PRs** (T-310, SR-8.2): `atm releases add` takes `--pr` more than once (a release
  shipped in several PRs gets all their tasks, the release records the last PR) and `--released-at TIME`, which adds
  it as released at that time. This repository's releases 0.5.0–0.23.0 were recovered that way, each released at its
  PR's merge; 0.5.0–0.12.0 have no tasks, as their PRs weren't recorded on tasks yet.

### Added
- **Review states, components and work together on the dashboard** (T-316, SR-6.6, SR-7.6): cards show a task back
  from review (*Changes requested*, *Approved*), *with you* for `with: user`, and its components; the detail panel
  also its effective review mode (`lib/review.ts`) and whether it waits for the sprint review. *Needs you* counts
  `with: user` tasks to do ("to do with you") and no longer a task back from review (an agent's work again), in
  the dashboard (`needsYouReason`) and in `atm serve`'s summary (`needs_you.together`). The sprint header says how
  many done tasks wait for the end-of-sprint review.
- **`with: user`** (T-314, SR-3.5, SR-5.3, SR-6.5, SR-6.6): a task done together with the user (`atm add` / `edit
  --with user`, removed with `--with ''`). `atm next` skips it unless `--with-user`, and says when only such work is
  left; `atm start <id>` takes it; `atm list --with-user` lists them. `/atm:loop-tasks --with-user` takes them;
  without it they are listed at the end as waiting for you. `atm migrate` gives unfinished tasks labelled
  `interactive` `with: user` once (recorded in `meta.migrations`; the label stays; `--json`: `with_user_from_label`).
  `/atm:create-tasks` sets it for work the user wants to do together.
- **`atm approve` and `atm request-changes`** (T-313, SR-3.9, SR-6.1, SR-6.2, SR-6.4): a task in review carries
  `review_state` (`waiting` after `atm review`, `approved`, `changes_requested`) and `reviewed_at`; both stay in the
  review kind (dependents keep waiting) and go when the task leaves it, the round recorded in `reviews` with its
  `outcome`. `request-changes --reason` adds a rejection note. `atm next --claim` takes such tasks back before new
  ones, giving them to the agent that worked on them for one lease length after the review, then to any agent;
  `atm start` takes one back too, but not one still waiting for its review. `atm reject` stays (back to to-do).
  `/atm:do-task` and `/atm:loop-tasks` record an approval with `atm approve` instead of a rejection. The merge
  driver merges the new fields with the progress fields.
- **Release view** (T-309, SR-7.5, `ReleaseView.svelte`, `lib/releases.ts`): on the Progress tab, each release not
  archived with its state, PR, target, the sprints that deliver to it, its tasks by stage (done, in PR, merged), and
  the notes draft (`atm releases notes`' grouping) to copy. The dashboard file's size limit (`guardrails.spec.ts`)
  goes from 450 to 500 KiB for the sprint and release views.
- **Review modes** (T-312, SR-3.8, SR-4.4): a task's effective mode (its own `review`, else its first component's,
  else `meta.review`) decides how `atm done` ends it. An agent's `atm done` on an active `blocker` or `async` task is
  refused (`atm review` instead); a `batch` task is done with its note as a `review` note and `sprint_review:
  pending`; `none` is done as before. A person (an actor not starting with `claude:`) may always `atm done`.
  `atm sprints close` refuses while a task in the sprint waits for the sprint review; `--reviewed` marks them
  reviewed. `atm status` and `atm sprints` list them; `atm reopen` clears `sprint_review`. `/atm:create-tasks`
  proposes a mode where one is needed.
- **Components** (T-311, SR-2.7): `meta.components` (`[{id, label, paths, review}]`, `paths` globs over the
  project's files, checked with `meta`). In a project with components a task's components must name them. `atm add`
  and `atm edit --add-file` print the components a task's files suggest (with the `atm edit … --add-component`
  that sets them); `--components auto` (and `"components": "auto"` in `--json-file`) sets them. `atm check` warns
  about a task whose files match no component. `/atm:create-tasks` proposes them.
- **`atm merged` releases a release** (T-308, SR-3.7, SR-5.6): when the PR it records completes a planned release
  (all its tasks in `tasks.json`, by their own release or their sprint's, skipped ones aside, merged in that PR), the
  release becomes `released` with its `pr` and `released_at`; otherwise atm says which tasks keep it planned
  (`--json`: `released`, `pending`). `/atm:push` relays it, and on the base branch of a release-branch project
  suggests `dev/<release>` from the active sprint's release, never renaming a branch.
- **`atm releases notes <R>`** (T-307, §3.3): a Markdown draft of the release's changes from its tasks (by effective
  release, `tasks.json` and the archive): the done tasks grouped by type in the project's order, each with its ID,
  name and last outcome note, then what isn't done yet; `--json` gives the same as data. For the CHANGELOG and
  WHATS-NEW entries a person writes.
- **Releases** (T-306, SR-2.2, §3.3): `meta.releases` (`[{id, name, state, target, pr, released_at}]`, states
  `planned`, `released`, `archived`, checked with `meta`) and `atm releases [--all]` (each release's tasks by kind and
  stage, by their own release or their sprint's), `atm releases add <R> [--name] [--target DATE] [--pr N]` (`--pr`
  gives it every task of that PR, in `tasks.json` and the archive), `atm releases edit`, `atm releases archive`. A
  sprint's and a task's `--release`, and the `--release` filters of `list` and `status`, must name a release
  (`no release '2.0'; the releases: …`); `atm check` reports one that doesn't exist. `atm check` no longer asks
  for a `phase`: migrated tasks have none (a `phase` that is there must still be a number).
- **`/atm:close-sprint`** (T-301, `skills/close-sprint/SKILL.md`, §4.3): closes a sprint with the user: reviews the
  done tasks with their outcome notes (follow-ups through `/atm:create-tasks`); decides each unfinished task (carried
  over by default, back to the backlog with `atm move`, or skipped with `atm skip`); chooses where carried tasks go
  (the next planned sprint, one of several, or a new one); closes it with `atm sprints close … --carry-to …` and
  offers `/atm:plan-sprint`. The user decides each step; it pre-approves only `atm migrate`, `status`, `sprints`,
  `list`, `show`, `deps`, `move` and `skip`.
- **`/atm:plan-sprint`** (T-300, `skills/plan-sprint/SKILL.md`, SR-4.1, SR-4.2): plans a sprint with the user: shows
  the active sprint and what's left, the planned sprints, the backlog by label and component, and the last closed
  sprint's carried tasks; agrees on the sprint (`atm sprints add`/`edit`); goes through candidates (`atm move`,
  dependencies flagged with `atm deps`), comparing its size with the last closed sprints; starts it only on the
  user's word (`atm sprints start`). It never starts, moves or dates anything without the user agreeing in the
  session, adds tasks only through `/atm:create-tasks`, and pre-approves only `atm migrate`, `status`, `sprints`,
  `backlog`, `list`, `show`, `deps` and `move`.
- **`atm sprints close <S> [--carry-to <S2> | new [--name N] | backlog]`** (T-299, SR-4.3, SR-4.5): refused while a
  task in the sprint is in progress; its unfinished tasks (to do, blocked, in review) go to `--carry-to`, needed when
  there are any, keeping their status and `sprint_history` (`new` adds a planned sprint right after it, named or
  generated); the sprint becomes `closed` with `end` and `report: {done, carried, added, to}` (`done` counts archived
  tasks too; `added`: joined after the sprint's `start`). `atm sprints` takes `--agent`.
- **Sprint header** (T-295, SR-7.2, `SprintHeader.svelte`): above the Kanban, List and Graph, the sprint the Sprint
  filter selects, or the active one when it selects none: its name (and state, when not active), goal, due date
  ("due in 3 days", "2 days overdue" in red), release, progress (done of the tasks not skipped) and, from
  `sprint_history`, how many tasks were added mid-sprint and carried in (`headerSprint`, `sprintSummary` in
  `lib/sprints.ts`).
- **Resolution and relations on the dashboard** (T-319, SR-7.6): the detail panel shows how a finished task ended
  when it wasn't done (`#detail-resolution`: won't do, obsolete, duplicate), what it duplicates and what duplicates
  it (`#detail-duplicate-of`, `#detail-duplicated-by`) and its related tasks both ways (`#detail-related`), each
  linked (a task that isn't in `tasks.json` is shown, not linked), in the Dependencies group. The Progress panel
  counts the finished tasks that weren't done apart from done, by resolution ("Not done: 2 won't do · 1 obsolete",
  `#progress-not-done`). The rules are in `lib/relations.ts` (`resolutionOf`, `relationsOf`, `notDone`).
- **Resolution and relations** (T-318, SR-2.8): `atm done --resolution done|wont_do|duplicate|obsolete` and
  `atm skip --resolution wont_do|obsolete|duplicate [--duplicate-of ID]` record how a task ended; only a resolution
  other than the default (`done`, or `wont_do` for a skipped task) is stored. `--duplicate-of` needs `duplicate` and
  a task that exists in either file; `atm reopen` clears both. `atm show` prints the resolution, the task's
  `related` tasks on both sides ("related: T-4, T-1") and "duplicated by: T-2". `atm check` checks them; the merge
  driver merges `related` as a set. None of it affects readiness.
- **`atm summary`** (T-321): the project's summary as `atm serve` lists it in `api/projects`, JSON, for the project it
  runs in. The Vite dev server serves it (its projects now have their numbers too), and the e2e fixture with
  `withAtmSummary()`.

### Removed
- **Phases leave atm-cli** (T-320, SR-8.3): `atm phases` (and `phases name`/`unname`), `--phase` on `next`, `list`,
  `archive`, `add` and `edit`, the phase column of `atm list` (now ID, status, priority, labels, name), the
  `by phase` line of `atm status` (a `by sprint` line instead, in a project with sprints) and the checks of
  `meta.phases`. `atm next` orders by priority, then ID; its line names the task's sprint ("Next: T-2 Two (Themes,
  feature)", or "backlog"), as `atm show` does. `atm serve`'s summary drops `current_phase` and `recent_phases`, its
  board cards carry `sprint` instead of `phase`, and its board's to-do column holds the loop's default selection; a
  project card without sprints shows one ring for all its tasks and "No sprints yet", and the All projects board's
  cards lose their phase badge. A `phase` left on a task is kept but ignored; `atm migrate` (run by every skill and
  by `atm serve`) turns phases into sprints first. `/atm:push` groups the PR body by sprint.

### Fixed
- `atm unblock` refused to record the answers when the questions and answers together were longer than 500
  characters; the limit now applies to each question and answer.
- `atm check` warned about every finished task's missing files without `--files` (since components, T-311): the
  option was hidden by a variable of the same name (T-318).

## [0.23.0] - 2026-10-30

### Changed
- **Kanban:** every status column stays on the board whatever the filters; a status filter empties the columns it
  leaves out instead of removing them (`buildBoard` in `lib/kanban.ts`). A skipped-kind column still shows only with
  *Show skipped* or when the status filter names it.
- **List tab** (T-281): the *Order* dropdown is drawn by the page (`.select-box` in `app.css`, the theme picker's
  style and chevron) instead of the platform's native control, whose font, padding and arrow changed between macOS
  and Chromium versions and kept failing the List screenshot tests on developer Macs.
- **Dependency graph layout:** laid out by dagre (`@dagrejs/dagre`, bundled into the dashboard) instead of columns
  by dependency depth: a task sits close to what it feeds instead of in the first column, a chain runs on one line,
  arrows cross less and follow dagre's route (`layoutGraph` in `lib/graph/layout.ts`; `topologicalLayers` and the
  layout's `layers` are gone). The layout is computed again only when the graph's shape changes (which tasks, their
  dependencies), so a reload that only changes statuses reuses it. The dashboard file's size limit
  (`guardrails.spec.ts`) goes from 400 to 450 KiB for dagre (about 48 kB).

### Added
- **Sticky notes theme** (T-283, `themes/sticky-notes.css`): the Kanban on a blackboard (dark) or a whiteboard
  (light), in a wooden or aluminium frame, with a chalk-dust or satin texture. The columns have no boxes: their
  headings and the lines under and between them are drawn in chalk or blue marker, by hand (SVG strokes that wobble,
  double up and break). The cards are paper sticky notes coloured by type (feature yellow, bugfix pink, test purple,
  infra blue, docs green, refactor and polish orange), tilted a little, with the name handwritten; inside a note the
  colour tokens are dark inks that read on every note colour in both modes (`theme-contrast.test.ts` checks the
  board and the notes). Card and board hooks other themes can set, defaulting to the old look: `--card-bg`,
  `--card-border`, `--card-radius`, `--card-shadow`, `--card-tilt`, `--card-font`, `--card-font-size`,
  `--board-surface`, `--board-frame`, `--board-radius`, `--board-shadow`, `--board-padding`, `--board-gap`,
  `--board-align`, `--board-font`, `--board-ink`, `--board-title-transform`, `--column-bg`, `--column-border`,
  `--column-radius`, `--column-divider`, `--column-header-line`, `--column-divider-image`, `--column-header-image`,
  `--column-header-overhang` (`app.css`). Cards carry `data-type` and `data-priority`; a column heading's status
  colour is `--column-color`.
- **Hand-drawn theme** (T-283, `themes/hand-drawn.css`): the Kanban sketched on a notebook page, cream paper in
  pencil (light) or a black notebook in white gel pen (dark), ruled, with a red margin line and punched binding
  holes. The columns, the lines under their headings and the cards' boxes are drawn by hand (the shared
  `--column-*-image` hooks; the boxes are a mask, `--card-sketch`, painted in the colour of the card's column), the
  page shows through the cards, and the names are handwritten in bold (`--card-font-weight`, new). *Next up* is
  circled by hand in the pen's blue instead of the straight frame.
- **Theme menu:** a separator line sets the themes that draw the board (Sticky notes, Hand-drawn) apart from the
  colour themes (`group: 'board'` in `themes/index.ts`).
- **Dependency graph hover:** hovering a task highlights it and every task it waits for, directly or not (a bolder
  border and a glow in the task's colour, the arrows between them bold in the accent colour); the rest fades
  (`dependencyChain` in `lib/graph/layout.ts`). A greyed-out task on the chain lights up too.
- **Dependency graph:** a *Dim done* button among the graph's controls greys out done tasks and the arrows touching
  them, whatever the filters, alongside *Filtered out: Hide / Dim*. Saved with the project's view (`graphDimDone`).

## [0.22.0] - 2026-10-27

### Changed
- **Task details** (T-280): the title, with the × on its line, and the badges stay at the top; the sections scroll
  below them without a scrollbar, like the Kanban columns, with *▲ N more* / *▼ N more* hints (sections cut off) that
  scroll a page. Less padding around the panel.
- **Section rail:** on the left of the sections, one entry per group the task has: *Info* (in review or blocked, the
  description), *Dependencies*, *Notes*, *Activity* (claimed by, who, timestamps, commit) and *Ref* (use cases,
  requirements, files), written bottom to top with an icon. A click brings the group to the top, adding room below
  the last section when the content is too short; scrolling by hand never adds any. The entry of the group in view
  is highlighted. The sections are in that order now (the Activity group was spread over the panel before).
- **Dependencies** in the details take one line each: a long name ends in "…" and shows in full on hover.
- **What's new** (T-282): the title, with the × on its line, stays at the top; the releases scroll below it without
  a scrollbar, with *▲ N more* / *▼ N more* hints (releases cut off). One scroller with these hints
  (`HintScroll.svelte`) now serves What's new and the Kanban columns, and one count of what is cut off
  (`cutOff` in `lib/scroll.ts`) serves those and the task details.

## [0.21.0] - 2026-10-27

**The repository moved to the `solo-swe` organisation.**

### Fixed
- **Activity popup heading:** it said *Activity per 3 hours* on *All time* and *per day* on *Week*: the bar size,
  which read as the period. It is now just *Activity*; the period label beside it names the period, and every chart
  heading names the period and its bar size (*Time per task* now names the period too). T-144 had fixed the chart
  headings only, and an e2e test pinned the wrong heading.
- **All projects board (Omniboard):** the page no longer scrolls on it: the board takes the window's height and each
  column scrolls on its own with *▲ N more* / *▼ N more* hints, like a project's Kanban (T-262). Grouped by project,
  the lanes scroll as one.

### Changed
- **Omniboard:** the All projects page's *Board* tab is called *Omniboard* (`#view=board` links and the remembered
  tab still work), with an ⓘ on what it shows and that each project's details are on its own Kanban.
- **Project tab:** the header is the logo and the name, without `meta.description` under it; side by side, the
  *Information* and *Stats* panels end at the same height.
- **Repository:** `github.com/solo-swe/agentic-task-manager` (GitHub redirects the old address). `plugin.json`'s
  `homepage` and `repository`, the dashboard's GitHub and Source links, the README and the docs point there.
- **Marketplace renamed** from `hakanserce-plugins` to `solo-swe-plugins`: the plugin installs as
  `atm@solo-swe-plugins`. An existing install doesn't follow the rename: `/plugin marketplace remove hakanserce-plugins`,
  `/plugin marketplace add solo-swe/agentic-task-manager`, `/plugin install atm@solo-swe-plugins`, `/reload-plugins`.
- **Titles:** the All projects page is titled *Projects Overview*; a project's page by its name alone, without
  *— Task Dashboard* (the title bar and the browser tab). *Task Dashboard* stays while loading and for a project
  without a name. The project selector's first option is *Projects Overview* too (typing its name finds it).
- **`atm repository set URL [--from OLD]`** also moves the tasks' recorded PR links (`pr.url`) from the old address
  (`--from`, else the one it replaces) to the new one, in `tasks.json` and the archive, without touching
  `merged_at`, timestamps or actors; `--json` adds `moved_links`.
- **The atm logo on GitHub:** the README is headed by the logo (`.github/assets/atm-logo.svg`, the dashboard's in the
  accent blue) instead of an emoji, and `.github/assets/social-preview.png` is the repository's preview card.
  `.github/assets/atm-logo.png` (512×512) is the logo as a PNG, for avatars.

## [0.20.0] - 2026-10-27

**Security hardening (phase 20) and progress and insights (phase 21).** Phase 20 implements the security review
(`docs/security-review.md`, `docs/security-architecture.md`); phase 21 the reviews of the Progress tab, the Activity
popup, project stats and the All projects Kanban.

### Security
- **Skills:** task text, notes and CI logs are data, not commands (`plugin/CLAUDE.md`, every skill); agent text is
  quoted in commands; skills pre-approve only the `atm` subcommands they need, never `atm run`.
- **atm-cli:** task IDs and the base branch are validated; `atm commit --file` takes exactly one literal file per
  flag; no reads or writes through symlinks in the task folder; robust against hostile task files (deep nesting,
  huge values, wrong types); `atm pr` accepts only http(s) URLs.
- **Server:** binds to loopback only, with Host checks, a Content-Security-Policy, `nosniff` and CORP headers, request
  limits; one broken project can't break `api/projects`.
- **Dashboard:** every link goes through one `safeUrl` check.
- **CI:** read-only token, no persisted credentials, actions pinned by SHA, Dependabot.
- **Verification:** a hostile-repository test suite; the agent sandbox (`tests/sandbox/run.mjs`) runs red-team tasks
  against the skills in a container; a verification schedule (every PR, every release, every quarter); `SECURITY.md`.
- **Loop:** `/atm:loop-tasks` confirms `--push` in the direct workflow and stops when a hook rejects a commit.

### Added
- **All projects: Projects and Board tabs** (remembered; `#overview&view=board` links to the board). The board shows
  To Do (ready), In progress, In review, Blocked and Done (last 24 h) across projects, from a new `board` summary
  field (`docs/multi-project-api.md` §3.3), sorted per column, 30 cards per column with *+ N more in <project>*, a
  project chip on each card; *Group by project* lanes; a search box over every project's tasks.
- **`GET api/search?q=`**: the task IDs per project matching every word (ID, name, description, type, labels, notes,
  project name); read-only, capped (§2.4).
- **Project tab** (last tab): Information (repository, workflow, people, agents, vocabulary) and Stats (started,
  duration, tasks, phases, commits, PRs, agent time alone and with you, parallelism, a time-per-task histogram with
  the slowest tasks).
- **Shared time use** (`timeuse.ts`): agents working, waiting on you, idle; the *Where the time went* bar on the
  Activity popup and the Progress tab.
- **atm records block and review intervals** (`blocks`, `reviews` on a task), merged by the merge driver, so waiting
  time can be measured.
- **Project summary `stats`**: started, tasks and agent minutes, archived tasks included; the overview cards show one
  line (*started 12 Aug · 236 tasks · 74 h agent time*).

### Changed
- **Tab order:** Kanban, List, Graph | Progress, Agents, Project; the Kanban is the default.
- **Activity popup:** three KPI cards (tasks done; time per task, agents alone and with you; where the time went);
  the agents chart stacked by kind with a waiting-on-you band; the time-per-task chart capped at the 90th percentile
  with outliers on the top edge and interactive tasks hollow with their own median; ⓘ notes on the heading and every
  chart. The sparkline draws the unfinished last bucket lighter and its tooltip explains the line. The buckets lose
  the unused `cumulativeDone`, `scope` and `wip`.
- **Progress tab:** agent time, waiting on you and idle replace utilisation; a filter banner (status and stage
  filters don't apply); the burndown has a title with ⓘ, a time range and what's waiting on people; the phase table
  shows finished phases in one row.
- **Header Progress popover:** finished phases as chips, types as one stacked bar, a Go to section.
- **About dialog:** statuses, phases and types only, with a *Project details* link to the Project tab.
- **Board cards** in progress carry `since` (`started_at`).
- **Detail panel:** a linked commit says *The link works once the commit is pushed*, unless its task is in a PR or
  merged (the dashboard doesn't know what was pushed).
- The dashboard's size budget is 400 KiB.

### Fixed
- **Kanban:** no page scrollbar; each column scrolls on its own, with *▲ N more* / *▼ N more* hints instead of a
  scrollbar (the wheel and trackpad still scroll).
- **Back to the overview** no longer leaves a task's detail panel open over it.

## [0.19.0] - 2026-10-26

**Insights reviewed, a security review, and dashboard polish.** Phase 19 reviewed the Progress tab, the Activity
popup, project stats and an All Projects Kanban with the user (`docs/progress-tab-review.md`,
`docs/activity-popup-review.md`, `docs/project-stats-proposal.md`, `docs/all-projects-kanban-proposal.md`); their
implementation is phase 21. The security review (`docs/security-review.md`: threat model, 19 findings) added phase 20.

### Added
- **Markdown in notes, questions and answers.** Notes, the review note and block questions in the detail panel render
  the same Markdown subset as descriptions (headings, lists, code, emphasis, safe links; HTML stays text). Card
  tooltips show the plain text.
- **Skills write Markdown.** `/atm:create-tasks` writes task descriptions in Markdown when it helps; `/atm:do-task`
  and `/atm:loop-tasks` mention `atm edit --description-file` and Markdown in notes.
- **Phase filter with many phases.** With more than 6 phases, the filter panel's Phase group lists *All*, the active
  phases (unfinished, and the one finished last) and any selected phase; *+ N finished* / *Hide finished* shows the
  rest, saved with the view (`filterFinishedPhases`).
- **Labels on Kanban cards** stay on one line: the first two, cut at about 14 characters, then a *+N* chip whose
  popover lists every label (hover or focus; a click pins it without opening the task).

### Fixed
- **Activity popup:** the Idle number fits on one line, and its colour bar became a swatch of the charts' idle
  hatching, with a tooltip.
- **About dialog:** on All projects it no longer shows the last project's section; in a project it names the project,
  with its logo.
- **Kanban columns** reach down to the bottom of the window instead of stopping at 640 px; each scrolls on its own.

## [0.18.0] - 2026-10-26

**Project cards that say what's going on.** The layout reviewed in `docs/project-cards-proposal.md`.

### Added
- **Server summary** (`/api/projects`, `docs/multi-project-api.md` §3.2):
  - `working`: the live claims, oldest first, as `{task, name, agent, branch}`;
  - `next`: up to 2 tasks `atm next` would hand out, in its order (one shared `ready_in_order`);
  - `last_done`: the task completed last, `{id, name, completed_at}`;
  - `git`: `{branch, pr}`, the main worktree's branch (from `.git/HEAD`, no git process) and the latest PR recorded on
    the tasks, `open` or `merged`. No forge call, no CI state. `.git/HEAD` is part of the summary's ETag;
  - `description`: `meta.description`, trimmed and capped at 200 characters.

  Task names in the summary are capped at 80 characters.
- **Project cards** show the tasks at work by ID and name, with agent and branch on hover, or "Next:" with what the
  loop takes next. They also show the branch and the latest PR (linked), and the last finished task with how long ago.

### Changed
- **Project card layout:**
  - Order: head (logo, name, "needs you"), recent-phase rings, working/next, git, last finished with the 14-day
    sparkline.
  - Every section keeps its height, so the cards of a row line up.
  - The project-wide done / total moved into the rings' tooltip.
  - The current-phase line shows only for servers before 0.17.
  - `meta.description` shows on hover over the name.
  - Servers before 0.18 keep the agent count and the last activity time.

## [0.17.0] - 2026-10-26

**Overview polish, project identity, and parallel agents that don't trip over each other.** Phase 17 was built by
three agents at once in separate git worktrees; what that turned up is fixed here too.

### Added
- **Project logos.** `atm logo [set PATH | unset]` (`meta.logo`: an SVG, PNG, WebP or ICO file inside the project, at
  most 512 KB); without one, a `logo.svg`, `.github/logo.png` and similar are found by themselves. The server serves
  it at `/p/<id>/logo` and lists `logo` in `/api/projects`. The dashboard shows it on project cards, in the project
  selector and next to the title, or the project's initials on a colour from its id.
- **The ATM logo** as the browser tab icon (an inline `data:` favicon), in About, and with "Agentic Task Manager
  (atm)" at the start of the status bar.
- **Phase rings on project cards.** The server summary has `recent_phases` (the lowest 4 phases with open work, filled
  up with the most recently finished ones; phases of only skipped tasks left out); each card shows up to 4 rings on
  one line, the project-wide count in their tooltip.
- **"Needs you" opens the task.** The chip in the title bar and on project cards opens the one task waiting for you,
  or a list of them (`NeedsYouMenu`); the summary has `needs_you_tasks` (`[{id, name, reason}]`). An older server
  still gets the filtered Kanban.
- **Kanban completion ring** for the tasks shown, with "67% · 24 of 36 done · 2 skipped" beside it.
- **`atm run <id> [--every SECONDS] -- <command>`** runs a long command (a test suite, a build, a wait on CI) while
  keeping the claim on `<id>` alive: renewed at the start (refused before the command runs when it isn't yours), then
  three times per lease. Output and exit code pass through. The do-task, loop-tasks and push skills use it for long
  checks; the PostToolUse hook renews only between tool calls.
- `docs/project-cards-proposal.md`: what project cards should show, reviewed; phase 18 builds it.

### Changed
- **One title bar for every page** (`TitleBar.svelte`): the overview has the same controls, at the same place, as a
  project page. The status bar sits at the bottom of the window on All projects too.
- **"Active phases only"** keeps every unfinished phase and the one that finished last (a phase of only skipped tasks
  counts as finished), instead of those finished within N days; the days field is gone (an old saved `days` is
  ignored). It is **on by default**.
- **Project cards** are taller (more padding, 220 px at least), the name gets two lines on every card (longer names
  end in "…", the full name and folder on hover), and a long phase name stays on one line, so the cards of a row line
  up.
- **Agent identity per worktree:** an agent identified by its Claude Code session is `claude:<session>@<folder>` in a
  linked git worktree (subagents share their parent's session ID); the main worktree keeps `claude:<session>`. The
  hook computes the same from its `cwd`, so it renews only that agent's claims.
- **Blocked and in-review tasks are shared across worktrees:** the claims store records `waiting` tasks, so `atm next`
  doesn't hand them out in another worktree; `unblock`, `skip`, `done` and `reject` work from any worktree and are
  applied where the task waits with its next atm command (`resolved`). `atm status` lists them, and the server leaves
  a worktree's stale copy out of `worktrees.json`.
- The update notice is a toast at the bottom right, never under the title bar; it shows only once per new build, and
  not at all when the page already is that build (a same-version server restarts only for a rebuild in its own plugin
  folder, and whoami's `build` comes from the bytes it serves).

### Fixed
- Each checkout's e2e server gets its own port (from its folder), so tests in parallel worktrees don't test each
  other's build.
- Section screenshots wait for the title bar to settle after scrolling and leave the sticky status bar out, which
  made them flaky on a busy machine.

## [0.16.0] - 2026-10-26

**All of a project's git worktrees on one board, links to commits and PRs, and clearer progress.**

### Added
- **Git worktrees on the dashboard.** `/api/projects` lists each project's worktrees (`worktrees: [{path, branch,
  main}]`, main first, read from the git directory without running git). A new route, `/p/<id>/worktrees.json`, lists
  them with, for each linked worktree, the tasks whose copy there differs from the main worktree's. The dashboard
  shows all of them on one board: each task from the worktree where it got furthest (by status kind; the main
  worktree's copy on a tie), with a branch chip on its Kanban and list card and in its details. Tasks only a branch
  has are shown too. Format: `docs/multi-project-api.md` §2.2.
- `atm repository [set URL]` records the repository's web address in `meta.repository` (git remotes are turned into
  `https` addresses, without credentials or `.git`); `atm init` fills it from `origin`. `/atm:create-tasks` asks for it
  when it's missing.
- **Commit and PR links** in the task details and on the In PR and Merged chips, built from `meta.repository` for
  GitHub, GitLab (also nested groups), Bitbucket and Gitea/Forgejo/Codeberg; a PR's recorded URL wins. A self-hosted
  server is recognised by its name (`gitlab.*`, `github.*`, ...) or by `meta.repository_type`. Other hosts show plain
  text; without an address, a click explains `atm repository set`.
- Task details show Started, Sent for review, Finished (or Skipped) and Duration (work time, "so far" while active),
  besides Created and Updated: only the times a task has.
- Progress panel: **Active phases only** on the By phase card, sharing the Progress tab's saved setting.

### Changed
- **Completion leaves out skipped tasks**: done / (total − skipped), so 7 done and 3 skipped is 100%. The rings draw
  the skipped share as a thin muted arc outside the ring, screen readers hear the count, and the phase table says
  "· N skipped". Forecasts and insights count skipped tasks as neither done nor left.
- `claims.json` is served from the clone's shared claims store, so agents at work in any worktree show on the board.
- The Progress panel has one card for By phase and one for By type.
- The theme picker in the Appearance menu is a drop-down list (the menu is 136 px tall instead of 456 px).
- The Status filter lists every status in the project's vocabulary, also those no task has yet (showing 0).
- The task description in the details panel sits in its own box, with smaller headings.
- `atm done`, `release`, `fail`, `review`, `block` and `heartbeat` on a task whose lease of the same agent expired
  now say so, when it expired and what to run next (`atm start <id>`, then the command again), with exit code 4. The
  expiry is saved before the error. An expired lease and `atm release` keep the task's commit; `fail`, `reject` and
  `reopen` still drop it.

### Fixed
- Opening another task scrolls the details panel back to the top; an auto-refresh of the same task keeps the scroll.

## [0.15.0] - 2026-10-25

**One dashboard for all your projects.** The dashboard is no longer copied into each project and served per project:
one server per user serves it, straight from the installed plugin, for every project atm runs in.

### Added
- **Project registry** in `~/.agentic-task-manager/projects.json` (`ATM_HOME` overrides the folder). `atm init`,
  `atm migrate` and `atm serve` add the project they run in, and the PostToolUse hook adds it on the first tool call
  (one small file read per call; it never waits for the registry's lock). A linked git worktree adds its main
  worktree. `atm projects` lists them; `atm projects add [PATH]`, `remove <id|PATH>` and `prune` manage them. A
  project opts out with `meta.dashboard.listed: false`. An unreadable registry is moved aside and started again.
- **`atm serve`, one server per user** (`ATM_HOME/server.json`), on port 8765 or the next free one. It holds the
  dashboard in memory and serves `/api/projects` (each project with a summary: done / (total − skipped), current
  phase, agents working, what needs you, last activity, completions over 14 days; computed by atm-cli's own logic,
  cached until a project's files change) and each registered project's `tasks.json`, `claims.json` and
  `tasks-archive.json` under `/p/<id>/` (ETag and 304), nothing else. It binds to 127.0.0.1 and refuses requests whose
  Host isn't `localhost`/`127.0.0.1` on its port (DNS rebinding). `atm serve` prints the project's URL
  (`#project=<id>`), `--all` the overview's. Routes and formats: `docs/multi-project-api.md`.
- **`atm serve --refresh`** restarts a running server whose dashboard is older than the plugin's (never a newer one),
  on the same port. A SessionStart hook runs it, and every atm command (so the PostToolUse hook too) makes the same
  cheap check, so the server follows a plugin update by itself.
- **Dashboard: project selector** next to the title: All projects, then each project with a progress bar and a dot
  when something needs you; type to filter, arrows, Enter, Escape. A badge counts what needs you in the other
  projects.
- **Dashboard: All projects overview**, a card per project: progress (skipped tasks shown apart), current phase,
  agents at work, what needs you (opens that project's Kanban filtered to review and blocked), last activity and a
  14-day sparkline. Projects that need you come first; **Active only** hides projects with nothing open and nothing
  done in 14 days. The dashboard opens on it with two or more projects, on the project with one, and explains how to
  add a project with none. `#project=<id>` and `#overview` links; an unknown project has its own page.
- **Dashboard: "A new version of the dashboard is ready"**, with Reload, when the server restarted on a newer build.
- `atm phases` lists the phases with their names and task counts by kind; `atm phases name <n> "<name>"` and
  `atm phases unname <n>` change one name at a time. `atm status` shows the names; `atm check` warns about a named
  phase without tasks. `/atm:create-tasks` names phases this way instead of rewriting `meta.phases`.
- `atm skip` records `skipped_at`; `atm migrate` fills in `completed_at` and `skipped_at` from `updated_at` for
  finished tasks without them (in the archive too), so a later edit doesn't move them in time.

### Changed
- `atm migrate` no longer copies the dashboard into the project. It deletes a project's old copy, stops its old
  per-project server (only when that server confirms it serves that folder) and removes `.server.json`/`.server.log`.
- The dashboard loads everything from the server's API (`api/projects`, `p/<id>/…`); opening the HTML file another
  way is no longer supported. The dev server and the e2e fixture play the same API.
- Saved views are kept under the project id the server gives (`meta.id`, so existing views are found again).
- Archiving dates a skipped task by `skipped_at`.

### Removed
- `/atm:update-dashboard`: updating the plugin updates the dashboard, and What's new is in the dashboard.

## [0.14.0] - 2026-10-25

### Added
- **Five more themes**, each in light and dark and checked for readable contrast: Ayu, Night Owl, Gruvbox, Noctis
  (with Noctis Lux) and Sublime Material.
- **Appearance menu** for 11 themes: it opens on the current theme, the arrow keys and Home/End move through them, and
  the list scrolls on small windows.
- **Progress tab: "Active phases only"**, which hides phases finished more than N days ago (7 by default, editable;
  remembered per project). The burndown still covers every phase.
- **Markdown in task descriptions** in the details panel (headings, lists, code, links).
- **What's new for users**: the dashboard's What's new and `/atm:update-dashboard` show `WHATS-NEW.md`, a short list
  per release; this changelog stays the detailed record.
- `atm skip <id> --reason TEXT`: a to-do or blocked task that isn't needed becomes skipped, with the reason as a note.
- `atm reopen <id> --reason TEXT`: a done or skipped task goes back to to-do (done by mistake, or not finished).

### Changed
- The Progress panel's rings (by phase, by type) are separate cards.
- `/atm:do-task` and `/atm:loop-tasks` commit a task's work while it is still in progress, and only then mark it done.

### Fixed
- `atm commit` accepts files removed with `git rm` (they were refused as unknown).
- Activity titles name the period you picked ("this week, per day"); choosing Week showed "per 6 hours".

## [0.13.0] - 2026-10-24

**Update the plugin for every agent before your first `atm archive`:** older versions see a dependency on an
archived task as missing, so its dependents never become ready.

### Added
- **atm-cli** (the `atm` command) does everything the skills need; they never touch `tasks.json` directly. New:
  `status`, `show`, `list`, `deps`, `add`, `edit`, `check`, `note`, `review`/`reject`, `block`/`unblock`, `commit`,
  `workflow`, `pr`, `merged`, `archive`/`unarchive`, all with `--json`. It is on Claude Code's PATH, so one rule,
  `Bash(atm *)`, approves it across upgrades.
- **Priorities** (`P0`–`P3`): agents take the most urgent first within a phase; cards, filters and List sort show
  them.
- **Labels** such as `spike-2` or `ux`: chips on cards, a filter, and search.
- **Your own vocabularies**: statuses (each of a kind), phases, types, priorities and labels, confirmed by
  `/atm:create-tasks` and listed in **About**.
- **In Review**: `atm review` hands work over for approval; `atm done` passes it, `atm reject` sends it back. Cards
  say how long it has waited.
- **Blocked on a decision**: agents ask instead of guessing (`atm block --on user|agent --question`); cards say
  "Waiting on you" or "on an agent", the details list the questions, `atm unblock --answer` replies. The header counts
  what needs you.
- **Notes** (handoffs, outcomes, reviews, rejections, answers, failures) and **who** created, started and completed a
  task, in its details; the Agents tab marks people, agents and approvals; a Started by filter.
- **`/atm:push`** for both workflows, direct or pull requests: pushes, opens the PR, watches CI and fixes failures
  without weakening tests or raising budgets. Your `CLAUDE.md` comes first.
- **Stages of Done**: In PR (linked) and Merged chips; in pull-request projects Done is split by stage. A Stage filter.
- **The archive**: `atm archive` moves old finished tasks to `tasks-archive.json`; atm suggests it past 1 MB or 500
  archivable tasks. The dashboard loads it only for Progress, activity details and **Include archived**.
- **Help** and **About** in the footer.
- A plugin hook renews claims while an agent works.

### Changed
- Unchanged `tasks.json` polls are a `304 Not Modified` (exact ETags, even for two changes in one second).
- The skills commit with `atm commit`: exactly the files named, plus `tasks.json`.
- `started_by` replaces `agent` (still read); `atm migrate` also updates a rebuilt dashboard of the same version.

## [0.12.0] - 2026-10-24

### Added
- Light and dark mode: the dashboard follows your system setting, or choose System, Light or Dark in the top right.
  Your choice is remembered per browser and applied before the page appears, so there's no flash of the other mode.
- Themes: Default, Nord, Solarized, Catppuccin (Latte/Mocha), Tokyo Night (Day/Night) and High contrast, each with a
  light and a dark variant. All colours meet contrast targets for readable text (see docs/theme-validation.md in the
  repository).
- Filter buttons show how many tasks each would show, given the other filters; values with no tasks are dimmed.
- A new top bar that stays in view while you scroll: a progress ring and an activity sparkline next to the title.
  Click the ring for completion overall, by phase and by type. Click the sparkline for activity: tasks completed by
  type, how many agents were working, time per task, and idle stretches when no agent worked, for a period you pick
  (hour, day, week, month, all time or custom; ‹ › step back to yesterday, last week, last month, …). Reload,
  auto-refresh, mode and theme are compact menus in the top right.
- Keyword search over task ID, name, description and type. The filters fold into one line under the title that shows
  only what's selected, with the number of matching tasks; **Filters** opens Phase, Type and Status.
- **Agents** tab: a timeline with a lane per agent and a bar per task, from start to completion, coloured by type;
  work in progress runs up to now. Zoom with the buttons, Ctrl/⌘ + scroll or the "last hour" presets, drag to move
  in time, or pick a range on the overview below the lanes. **Live** keeps it at the current time. Tasks outside the
  search and filters are greyed out.
- Agent utilisation on the Progress tab: agent work time against calendar time (24 hours a day) and against working
  time (8 hours a day, Monday to Friday), overall and per agent.
- `atm start` (and `atm next --claim`) record which agent took a task in its new `agent` field. It stays when the
  task is done and is removed if the task goes back to todo. Tasks finished earlier show as "Unattributed".
- **Task List** tab: the task cards, which used to sit below the views. The tabs have icons and sit on the view,
  which fills the rest of the window (the dependency graph uses all of it).
- The dependency graph can hide tasks outside the search and filters (the default) or show them greyed out.
- The dashboard remembers your view per project: filters and search, the selected tab, the options in each tab
  (Kanban, Progress focus, graph hide/grey out, Agents Live, the Activity period), and the zoom of the graph and the
  Agents timeline. **Reset view** forgets it.
- `/atm:create-tasks` gives new task files a `meta.id`, and `atm init` adds one to existing files, so dashboards of
  different projects on the same port keep separate views.

### Changed
- Kanban "Group by phase" is now remembered per project instead of per browser (your previous choice is kept as the
  starting point).
- The count boxes and the small completion chart above the filters are replaced by the top-bar widgets, and the
  views panel no longer collapses.

### Fixed
- Phase badges have their tinted background again (lost in 0.5.0).

## [0.11.0] - 2026-10-23

### Added
- Several agents can now work on one project at once. Run `/atm:loop-tasks` in more than one session, ideally each
  in its own git worktree: they never take the same task or overwrite each other's changes to `tasks.json`. See
  "Running agents in parallel" in the README.
- Claims: the skills take each task with `atm next --claim`, which records the agent, branch and worktree. All
  worktrees of a clone share the claims. A claim lasts 30 minutes and is renewed during long tasks; if an agent stops,
  its task becomes available again when the claim runs out.
- The dashboard shows who works on each in-progress task, on which branch and since when (cards, Kanban board and
  task details).
- `tasks.json` merges task by task: status changes from different branches combine, and tasks added on both
  branches are both kept. A git merge driver for this is registered automatically.

### Changed
- `/atm:do-task` and `/atm:loop-tasks` change task status only through the plugin's helper, which locks
  `tasks.json` and writes it atomically, so the dashboard never reads a half-written file.
- The skills commit only the files a task changed plus `tasks.json`, instead of everything in the working tree.
- A failed task records why in `last_failure`.

### Fixed
- `/atm:serve stop` no longer waits several seconds in containers (dev containers, CI images) before reporting the
  server stopped.

## [0.10.0] - 2026-10-23

### Added
- `/atm:serve` starts the dashboard in a background server (`--open` opens the browser, `stop` and `status` manage
  it). It uses port 8000 or the next free one, keeps running after the session ends, and only ever stops the server
  it started for this project.
- Everything atm needs now lives in one folder, `.agentic-task-manager/`, at the root of your project.

### Changed
- Projects that keep `tasks.json` in `docs/` or the project root are moved into `.agentic-task-manager/` the first
  time a skill runs, with `git mv` so history is kept. The move is staged for you to commit; `/atm:loop-tasks`
  commits it on its own before its first task.
- The dashboard HTML is no longer committed to your repository. It's a copy of the plugin's dashboard, listed in
  `.agentic-task-manager/.gitignore`, and the skills bring it up to date automatically when they run.
- The skills use a small helper script that needs **Python 3.9 or newer** (standard library only, nothing to
  install). On macOS it comes with the Command Line Tools, like `git`.
- Updating the plugin: run `/plugin marketplace update hakanserce-plugins`, then `/reload-plugins`. No restart.

## [0.9.0] - 2026-10-23

### Added
- **Progress** tab: a burndown chart of remaining work and total scope over time, with a forecast to the estimated
  finish. Click a phase to see only that phase.
- Per-phase table with progress, remaining work and a completion estimate, e.g. "≈ 1 h 10 m of agent work
  (50 m – 1 h 40 m)", plus the clock time if the loop keeps running. Estimates use the median time per task over
  the last 10 completed tasks and follow the loop's phase order; blocked work is flagged, not estimated.
- Estimates use `started_at`/`completed_at` when present (written by the skills since 0.7.0). Older task files
  still work: the time between completions is used instead, with idle gaps capped at 30 minutes.

## [0.8.0] - 2026-10-23

### Added
- Kanban board: the dependency graph now shares its panel with a **Kanban** tab (and a Progress tab, coming next).
  The selected tab is part of the URL, so a reload or a shared link opens the same view.
- Kanban columns for Todo, In Progress, Blocked and Done (Skipped on request). Todo cards say whether they are
  ready or which tasks they are waiting on, and **Next up** marks the task `/atm:loop-tasks` will pick next.
- Optional "Group by phase" swimlanes on the Kanban board, remembered per browser.

### Changed
- The graph keeps its zoom and position while you look at another tab.

## [0.7.0] - 2026-10-22

### Added
- Release notes in the dashboard: the version next to the title opens a "What's new" panel, and shows a "new" marker
  after an upgrade until you've opened it.

### Changed
- Auto-refresh is now on by default (every 5 seconds), and the dashboard remembers whether you switched it off and
  which interval you chose.
- The dependency graph keeps its zoom level when filters change or a reload changes the visible tasks. It only pans
  when no task would be left on screen.
- `/atm:do-task` and `/atm:loop-tasks` record when each task started (`started_at`) and was completed
  (`completed_at`). The activity timeline uses the completion time.

### Fixed
- `/atm:update-dashboard` looked for the new dashboard one directory too high inside the installed plugin, and now
  prints the actual release notes between your old and new version instead of a summary it made up.

## [0.6.0] - 2026-10-22

### Added
- Dependency graph zoom and pan: drag to pan, Ctrl/⌘ + scroll or pinch to zoom, zoom/fit/1:1 buttons, keyboard
  shortcuts when the graph has focus, and a clickable minimap when the graph doesn't fit.
- Large graphs open at a readable scale, centered on the task in progress; small graphs open at 100% instead of
  being stretched to the page width.

## [0.5.0] - 2026-10-22

### Changed
- The dashboard was rebuilt (Svelte 5) with an automated test suite. It looks and behaves the same as before.
- Filter buttons and the browser tab title now update when tasks.json is reloaded.

### Fixed
- Task names and descriptions are shown as plain text. Previously, HTML in tasks.json was rendered as markup.
- Installation instructions in the README use the correct plugin name (`atm@hakanserce-plugins`).

## [0.4.0] - 2026-04-05

### Added
- Task timestamps: `created_at` and `updated_at`.
- Activity timeline on the dashboard showing when tasks were completed.
- `/atm:update-dashboard` to copy the latest dashboard into a project.

### Changed
- `/atm:loop-tasks` commits after each task by default (`--no-commit` to opt out).

## [0.3.0] - 2026-04-04

### Changed
- The plugin lives in its own `plugin/` directory, separate from the marketplace configuration.

## [0.2.0] - 2026-04-04

### Changed
- The plugin is now called `atm`, so skills are `/atm:create-tasks`, `/atm:do-task`, `/atm:loop-tasks`.
- Repository restructured to follow Claude Code plugin conventions; installable from the `hakanserce-plugins`
  marketplace.

## [0.1.0] - 2026-04-03

### Added
- Initial release: skills to decompose requirements into `tasks.json`, execute one task, and loop through all tasks,
  plus a self-contained HTML dashboard with stats, filters, task cards and a dependency graph.
