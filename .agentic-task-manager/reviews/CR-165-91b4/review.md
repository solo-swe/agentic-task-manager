+++
id = "01a20e74-cee1-7230-aaef-3184ec3891b4"
number = 165
title = "The new tab's chooser of scope and item (/new)"
status = "approved"
tasks = ["ATM-755"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T16:47:45Z
updated = 2026-11-24T17:34:40Z
+++

The app's + (and File ▸ New Tab, ⌘T) now opens a new tab on `/new`, a chooser of scope and item, as a browser's new tab page is (ATM-755, [the design §3](docs/sidebar-overhaul-design.md#3-tabs-and-navigation)).

**What it shows** (`shell/NewTabPage.svelte`): *Open*, and two columns.
- **The scopes:** All projects, then the workspace's projects there, with their logos and what needs you (All's sums them).
- **The chosen scope's items:** four for All, six for a project, each with its icon and the Code Reviews badge.
- It opens on the sidebar's scope (the one the preferences keep).
- Each item is a link to its page, so a click opens it **in this tab**, and ⌘-click opens a tab of its own as every link in the app does. The chooser stays the tab's first entry: Back returns to it.
- It is titled *New Tab* (ATM-754), under the window's sidebar and title bar, with no page header.

**The keys** (`lib/new-tab.ts`, pure, so they're unit-tested): the chooser has the focus when it opens.
- ↑↓ move in a column, ←→ between the columns, and Enter opens the chosen item.
- Typing filters the projects by name or ID; All stays only while what's typed matches "all projects". The first project kept is chosen. Backspace and Escape undo the filter.
- Keys with ⌘, Ctrl or Alt are left to the app.

**Routing:** `/new` is now a page of its own (`routeFor` gives `{ page: 'new' }`), whatever the projects:
- Before, it was the landing page, which with one project became that project's page.
- The sidebar keeps its last scope on it, its rule for a page of no scope.
- Nothing loads All's data for it.
- `tab-actions`'s default for a new tab is `NEW_TAB_ADDRESS` (`/new`); it was `/`.

**Not done here:** the browser has no + and no chooser of its own (the design says the webapp doesn't need it). `/new` typed in a browser shows the chooser anyway.

**Tests:**
- `new-tab.test.ts`: the scopes listed with and without a filter (name, ID, case, All, none); ↑↓ at the ends and the item kept within a scope's items; → and ← and moving within the items; Enter (and nothing to open with no match); typing, Backspace, Escape; other keys.
- `NewTabPage.component.test.ts`: All then the projects there (a missing one left out), their badges, All's four items as links; the sidebar's scope chosen when it opens; a scope clicked giving its six item links; the keys working at once with Enter navigating; typing filtering and saying so, and no match.
- `routes.test.ts`: `/new` is its own page with none, one or two projects, in another workspace too. `app-tabs.test.ts`: a new tab goes to `/new`.
- `app.spec.ts`: + opens *New Tab* at `/new` with its three scopes; choosing Second Project's Tasks opens it in that tab; the menu's Back returns to the chooser; the keys (↑↑ Enter) open All's Overview. The two tab-history tests expect the new tab's chooser.
- `pnpm run check`, the unit suite, the functional e2e suite (618 passed).
