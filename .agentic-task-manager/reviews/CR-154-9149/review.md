+++
id = "01a20dfc-543e-713c-8d83-7587101a9149"
number = 154
title = "All-projects activity pill inside the tab"
status = "approved"
tasks = ["ATM-736"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T14:36:10Z
updated = 2026-11-24T16:20:15Z
+++

In the app, the All-projects activity pill moves from the window's title bar into the tab, at the right end of its subtitle line, where a project's Progress and Activity pills are (ATM-736, [the design §6](docs/sidebar-overhaul-design.md#6-moved-inside-the-tab-atm-736-atm-737-atm-738)). It belongs to the All scope's pages, not to the window.

**The change (`shell/Header.svelte`):**
- The overview's `TitleBar` gets the pill as its `trail` in the app, on every page of the All scope (Overview, Tasks, Code Reviews, Analysis).
- The window title bar's end (`appEnd`) keeps only the workspace switcher and the ⋯.
- In the browser nothing changes: the pill stays among the title bar's charts.

It shares the project pills' narrow-tab rule (`.in-tab-trail` hides below the same width), so on a narrow window it gives way the same way theirs do.

**Tests (`app.spec.ts`, the T-530 test):** on each of the All scope's four items, the activity pill is in the tab's trail and the window title bar has none. It sits on the subtitle's line, there is no Progress pill, and a click opens its panel.

`pnpm run check`, the unit suite, and the e2e specs (583 passed).
