+++
id = "01a20809-2e06-7334-b030-a432b232e0bf"
number = 127
title = "Stats, progress and the forecast from atm"
status = "approved"
tasks = ["ATM-722"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T10:52:29Z
updated = 2026-11-23T15:14:09Z
+++

The project's stats, progress and forecast are atm's: one route for a slice of the tasks and a period, and the overview's Progress tab from the summaries. The dashboard draws them and works none of them out (docs/dashboard-derived-state.md group F).

## What changed
- **atm-lib `stats`** (`stats.rs`, with `intervals`, `forecast`, `progress`, `time`, `people`):
  - `project_stats(project, query, now)`: of the slice, progress by status, type and sprint, the forecast (the pace, each sprint's counts and estimate in the loop's order, the row the loop works on next) and time use; of the whole project, its numbers, the time per done task, people and agents.
  - `overview(project, now)`: done and total, the open releases, the days tasks were done, the burndown and the forecast, for the summaries' `progress`.
- **The front ends:**
  - the route `…/projects/<p>/stats?tasks=&from=&to=`, in v1 and v2, through `ProjectSource::stats`;
  - the app's `project_stats`, in the main window's capability;
  - the CLI's hidden `atm project stats`, which the e2e fixture runs; `atm project status` prints the forecast.
- **The schema:** `Stats` and its parts, and `ProjectSummary.progress`.
- **The dashboard:**
  - `lib/stats.ts` loads them; `forecast.ts`, `projectstats.ts` and `projectinfo.ts` only reshape atm's answer;
  - the Progress widget, the Progress tab (its slice and period fetched as they change), the Project tab and the overview's Progress tab read them;
  - their TypeScript rules are gone.
- **Docs:** `docs/multi-project-api.md` §2.5a and §3.2, `docs/security-architecture.md` (the route and the command), CHANGELOG.

## Tests
- atm-lib's `stats` tests, the route's tests with the fake, the app's integration test;
- `tests/scripts/scenarios/stats.json` (the status line, no pace, a slice), and `serve.test.mjs` (the summary's `progress`);
- the dashboard's unit tests, rewritten for atm's answers (`stats.test.ts` new); the e2e fixture serves the route from the binary;
- clippy, `cargo test`, the CLI suite, the functional e2e.

## Round 2 (`8c1b5a1`)
The timed e2e test of a reload on the graph tab took 230 ms against its 200 ms budget. The cause was the fixture: it runs atm with `spawnSync` inside its route handler, so a request for a new body's stats held up the tasks request. The fixture now works the stats out when the data is set, as it does the task views: 60 ms. And the dashboard no longer waits for the stats before showing the tasks: they come in after, if the page still shows the project (a unit test).

> [!NOTE]
> The range also holds ATM-681's, ATM-682's and ATM-683's commits since round 1, each with its own review. ATM-722's are `3a88ca2` and `8c1b5a1`.

## To look at
- An unmigrated project's phases are its sprints in the stats, as in the sprint views (`planned_vocabulary`).
- The e2e fixture runs atm at the page's fixed clock (`page.clock.setFixedTime` sets atm's time too), as atm now works the times out.
- An archived skipped task now counts as skipped, not done. The TS counted the archive as done until the archive was loaded; the widget's count is now exact from the start, and its title says "(3 archived)", not "(3 archived, done)".
- The overview's burndown and forecast add up each project's figures, as work goes one task after another.
- Velocity from day counts takes each day's middle as the time.
- Not moved, left for later: the agents' lanes, and the TS `timeUse` and `agentTimeline` of the Activity widget, the Agents tab and the overview's timeline. The overview still loads every project's tasks for those, but no longer for its Progress tab.
