+++
id = "01a1fb3d-f6b6-7532-b3d9-8b56be138b25"
number = 101
title = "A tab's own history in the app: Back and Forward walk the active tab, a switch adds nothing"
status = "approved"
tasks = ["ATM-694"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T23:15:04Z
updated = 2026-11-20T23:32:52Z
+++

Task 1 of the tabs design (§4.1, §4.3): each of the app's tabs has its own history.

- **`AppTabs`:** each tab keeps `history: { entries, at }`. An entry is an address, its state (where it was reached from) and its page's scroll. It does `push` (dropping what was ahead), `replace`, `go(±n)`, `canGo` and `entry`, keeps the last 50 entries, and saves them over a reload. Tabs saved by an older page start with a one-entry history.
- **`navigation.ts`:** in the app (`useTabHistory`), `navigate` steps in the active tab's history, and the page's own history is always replaced, so it stays one entry deep. `back(n)` and `forward()` walk the tab's history and restore each entry's scroll. In the browser nothing changes: a browser tab is the tab.
- **A switch of tab** saves the shown entry's scroll, then shows the other tab as it was left (`showTab`), adding nothing to either history. A new tab's Back is disabled until it moves.
- **Back and Forward** in the menu (⌘[ ⌘]), the document viewer's Back and Close, the review page's back button (`goBack`) and the mouse's back and forward buttons all use the tab's history.
- **Links:** an `<a>` click the router would push itself becomes a step in the tab. The link the app starts with is where its first tab starts, so there's no Back to the app's root.
- **Scroll:** each scrolled element with an `id` (at most 20), restored after arrival and again two frames later, for a view that draws late.

Tests:
- unit: the steps, Back and Forward, a step from the middle, replace, a switch adding nothing, scroll restored on Back and on a switch back, old saved tabs, the 50-entry limit;
- e2e (app): steps through a view and a task's details, the menu's Back and Forward, a new tab's dead Back, the first tab as it was left, the mouse buttons, and the page's `history.length` never growing;
- the browser traces stay as the existing back-button tests in `reviews.spec.ts`.

Not done:
- **A trackpad swipe.** The window's webview has no back and forward gestures on, and telling a swipe from scrolling the board sideways needs more than a wheel event. I'd do it with the title bar's ‹ › arrows (§4.5), which this task doesn't build.
- **The three review traces in the app.** A plain click still opens a review in a new tab there; that changes in a later task (§4.5's in-place click). Until then the in-app test uses views and task details.
