+++
id = "01a20e02-bfdd-757a-a820-4b8fb6c82deb"
number = 155
title = "All-projects search moves up in the tab"
status = "approved"
tasks = ["ATM-737"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T14:43:10Z
updated = 2026-11-24T16:20:29Z
+++

The All-projects search moves up to where a project's search is (ATM-737, [the design §6](docs/sidebar-overhaul-design.md#6-moved-inside-the-tab-atm-736-atm-737-atm-738)): the same field in the same place on every page that searches.

**The change:**
- `overview/BoardSearch.svelte`: the field, taken out of the overview's toolbar unchanged (same ids, the same `task-search` look as a project's: a magnifier until used).
- `shell/Header.svelte` shows it on All's Board and List, the pages it searches:
  - in the app, as the tab line's `lead`, at its left before the subtitle, as a project's search and Filters are;
  - in the browser, in the title bar's charts, before the activity pill, as a project's search is before its pills.
- `ProjectsOverview.svelte`: the toolbar keeps the rest (the ⓘ, Active sprints / All tasks, Group by project, Hide inactive projects). The row stays because those controls are still in it.

**Tests:**
- `all-projects-board.spec.ts`: the search is in the title bar, not the toolbar, and before the activity pill. It is on the Board and the List, and not on Projects, Progress, Agents or Code Reviews. The search tests themselves (T-261, All's List) are unchanged and pass.
- `app.spec.ts`: on the All scope's Tasks the search is at the left of the tab's line, before the subtitle. Overview, Code Reviews and Analysis have none.
- `pnpm run check`, the unit suite, the functional e2e suite (613 passed).
