+++
id = "01a20b9e-630e-7636-8eb8-c1b832d0ea07"
number = 147
title = "The page by item: items as tabs, sub-tabs, All's List"
status = "approved"
tasks = ["ATM-752"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T03:34:19Z
updated = 2026-11-24T03:36:39Z
+++

The page by item (ATM-752, sidebar design §2.3 and §5): the seven view tabs become the sidebar's items, with sub-tabs.

**A project's page:** Overview · Inbox · Tasks · Code Reviews · Documents · Analysis as its tabs (`ItemTabs.svelte`). Tasks has Board · List · Dependencies, and Analysis Progress · Agents, as sub-tabs under them (`SubTabs.svelte`). An item opens on its first view, and every item and sub-tab click is a step in the tab's history. The Code Reviews item carries the "waiting for you" badge. Inbox and Documents show a placeholder line until ATM-762 and ATM-766 build them.

**All projects:** Overview (the cards) · Tasks (Board · List) · Code Reviews · Analysis (Progress · Agents). Its board is now named **Board**, as the design calls it (it was the Omniboard); `/board` links are unchanged.

**All's List (new, `/list`):** the Board's tasks as rows grouped by column, each row with its project chip, with the same search, scope and "hide inactive" options. It uses the projects' summaries, not each project's full task list (so it doesn't add to the app's memory growth, ATM-734). The summaries carry a project's first cards of each column, so each group shows its full count and "+ N more in <project>", as the Board does. A row opens the task's details over the list. *Worth your look:* is the List of summaries enough for All, or do you want every task of every project there (a new route)?

**Please note:**
- **In the app the item row still shows.** The task says the sidebar takes its place, but the sidebar items are ATM-753; until then the app would have no way to switch items. ATM-753 removes it there.
- **A fix the e2e run caught:** `/?view=list` (the landing page, one project) opened the project's Overview, because `list` is now also one of All's tabs. `withProject` now keeps a landing-page view where the page gone to has it (`routes.test.ts`).
- The search and its debounce moved into `board-search.svelte.ts`, shared by the Board and the List.
- Back-button names: the Project tab is now "Overview", All's board "Board".

**Tests:** `ItemTabs.component.test.ts` (items per scope, keys, badges; sub-tabs), `tabs.test.ts` (each view's item, All's), `allboard.test.ts` (the board without a cap), `routes.test.ts`, places and history names. E2E: `tabs.spec.ts` rewritten for items and sub-tabs (Inbox and Documents, keys, Back across sub-tabs, deep links), All's List in `all-projects-board.spec.ts`, and the other specs reach views through a new `openView` helper. Unit tests 1,978 and the functional e2e suite (606) pass. The screenshots of the tab rows change; the release task regenerates them.
