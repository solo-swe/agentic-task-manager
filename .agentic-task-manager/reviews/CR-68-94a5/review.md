+++
id = "01a1f7de-350c-71fb-98e6-3dc04b2394a5"
number = 68
title = "A review goes back to where it was opened from"
status = "approved"
tasks = ["ATM-686"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T07:31:37Z
updated = 2026-11-20T09:50:03Z
+++

A review's back button goes back to where the review was opened from, and says where: "← Kanban", "← T-004" (a task's details), "← All reviews", "← Reviews". It goes up to the project's reviews list only when the review was opened on its own: a new tab, its address typed, a link from outside.

## How
- **Every history entry remembers where it was reached from** (`lib/navigation.ts`): a push records the address shown before it (`atmFrom`). A replace keeps what the entry had.
  - **The same page** (the same path, with a task's details opened over it, another file or another round) keeps the page's own origin, one step further back (`atmSteps`). So opening a task's details over a review and closing them, or moving between files, doesn't lose the board the review came from.
  - **A switch of the app's tabs** pushes a `fresh` entry, with no way back in that tab, since the entry before belongs to another tab.
  - **An anchor click that SvelteKit follows itself** gets the same record, in the layout (`beforeNavigate`/`afterNavigate`).
- **The back button** (`ReviewsTab.svelte`) goes Back through the history (`goBack`: `history.go(-steps)`), so the place returns as it was (the details open, the filters) and Forward comes back to the review. Its label is `placeName(address)` (`lib/places.ts`): a task's key, Document, a review's reference, or the tab's name.
- **A plain click opens a review in place in the app too**, as `docs/tabs-and-navigation-design.md` (ATM-653) proposes. Before, every click opened a new tab, which has no history to go back through. ⌘-click (Ctrl, middle click) still opens a tab of its own. This covers the review links on cards and in the details (`ReviewLink.svelte`), the Reviews list, all projects' reviews (`OverviewReviews.svelte`) and the needs-you chip (`reviews/open.ts`).

> [!NOTE]
> The in-place click in the app comes from ATM-653's design, which is still waiting on your review (CR-63). Without it, the fix would do nothing in the app. If you'd rather keep new tabs on a plain click, the back button there would always go to the reviews list.

## Tests
- **Unit** (`navigation.test.ts`, `places.test.ts`):
  - a push remembers the page before;
  - details over the board are a place to come back to;
  - on the same page the origin is kept, a step further each time, across a replace;
  - a page opened on its own has none; a fresh entry has none;
  - the back button goes that many entries back;
  - each place's name.
- **E2e:**
  - from a Kanban card, back to the board, and Forward to the review again;
  - from a task's details, back to them, open over the List;
  - details opened over the review and closed, still back to the board;
  - typed address, up to the list (`reviews.spec.ts`);
  - from all projects' reviews, back there, and Forward (`overview-reviews.spec.ts`);
  - in the app, a plain click opens in place and comes back to the list, and ⌘-click opens a tab whose back goes to the list (`app.spec.ts`).
- **Checks:** `pnpm run check`, the unit tests (1,811), the functional e2e (526 passed), and the WebKit project (7).
