+++
id = "01a20b29-9a1a-70a2-9997-88d5e9b34a5f"
number = 141
title = "Sidebar items: the item of an address, the new pages, a project opens on its Overview"
status = "approved"
tasks = ["ATM-751"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T01:26:45Z
updated = 2026-11-24T02:39:08Z
+++

The sidebar overhaul's item model and addresses (`docs/sidebar-overhaul-design.md` §4).

**What changed:**
- **`itemOf(address)`** (`lib/address.ts`) gives the item a page belongs to: `overview`, `inbox`, `tasks`, `reviews`, `documents` or `analysis`. `ITEMS` lists all six and `ALL_ITEMS` the four for All projects. The landing page and `/new` belong to no item. `ITEM_NAMES` in `places.ts` gives each its name.
- **New addresses:** All's `/list`, a project's `inbox` and `documents` tabs, and the app's `/new` (with `namesNewTab`). The dashboard's `PAGE_PATH`, the route files, the route matcher, the server's `pages.rs` and the app's deep-link `PAGES` all know them. Their pages come later: `/list` and the two tabs show the default for now (ATM-752 builds them), and `/new` shows the landing page (ATM-755).
- **A project opens on its Overview:** `DEFAULT_VIEW` is now `project`. A project's address without a tab no longer brings back the tab you saw last. The rest of the saved view (filters, search, each tab's options) still comes back.
- **Labels:** Kanban is now **Board** and Graph is **Dependencies**, on the tab buttons and in Back's labels. The addresses keep `kanban` and `graph`, so old links still work.
- `docs/multi-project-api.md` §2.7 lists the new addresses.

**Please check:**
- The saved tab no longer winning over a bare `/projects/<id>`: I read the design's "an old link lands on the Overview" that way. It changed `saved-view.spec.ts` and `state.test.ts`.
- The e2e specs that opened a project without a tab and expected the board: they now name `kanban` (sprints, performance, app, tabs).
- The tests: `address.test.ts` covers the item of every kind of address and the new paths read, written and refused. Also `tabs`, `places`, `pages.rs`, the app's `links.rs` and `serve.test.mjs`'s page paths.
