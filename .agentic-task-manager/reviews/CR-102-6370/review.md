+++
id = "01a1fb54-95b2-70b3-b899-55333e5c6370"
number = 102
title = "Clicks as in a browser: every card and row a real link; ⌘ and middle click in the background, ⇧⌘ forward"
status = "approved"
tasks = ["ATM-698"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T23:39:46Z
updated = 2026-11-20T23:51:59Z
+++

Task 4 of the tabs design (§4.5): clicks as in a browser, on every card and row.

- **Real links:**
  - A Kanban card's name, the Omniboard's cards and a list row's name are now `<a href>` with the task's address (`dashboard.taskAddress`: this page with `?task=`).
  - On a card the link covers the whole card (`.card-link::after`), with the card's own links and buttons above it. A ⌘-click or a middle click anywhere on the card is the link's, and *Copy Link* works.
  - The card is no longer a separate `role=button` with its own Enter key: the link is its one stop for the keyboard, and Enter on it opens the details.
- **One place for the clicks** (`new-tab-clicks.ts`):
  - `followInPlace` makes a plain click open in place and leaves every other click alone.
  - `newTabAddress` now also takes any link to one of the dashboard's pages (a review row, a card), so the app's one capture handler opens it.
  - `#…` links stay the text's own.
- **Background or forward:** in the app, ⌘- and middle clicks open a new tab in the background, right of this one (`AppTabs.open(address, true)`); ⇧⌘ brings it forward. In the browser, the browser does all of this from the real links.
- A middle click on a title-bar tab already closed it; that's unchanged.

Tests:
- unit: `followInPlace`, `plainClick`, `inBackground`, and page links in `newTabAddress`, with `#…` and other sites refused;
- e2e: a card's href and a plain click in place (Kanban); the Omniboard and the Kanban by the keyboard through the link;
- e2e, in the app: a middle click and a ⌘-click in the background, ⇧⌘ forward, a list row's link and its plain click.

Changed on purpose: two app tests expected a ⌘-click to show the new tab; it now stays in the background, as §4.5 says.
