+++
id = "01a1f255-18f1-77ce-baa0-187b8e51ad9f"
number = 49
title = "A long N need you list scrolls in a small popup"
status = "approved"
tasks = ["ATM-640"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T05:43:45Z
updated = 2026-11-19T06:00:25Z
+++

The bug: the *N need you* popup grew without bound as more tasks and reviews needed you. Now it stays small and scrolls without a scroll bar, like the dashboard's other lists.

## Change
- **`NeedsYouMenu.svelte`:** the rows are in a `HintScroll` (the scroller the Kanban columns and dialogs use). *▲ N more* / *▼ N more* show at the cut edges, and a click on one scrolls a page.
- **Height:** the popup is capped at `min(320px, 60vh)`, about eight rows (`app.css`, `.needs-you-scroll`).
- **Keyboard:** the arrows, Home and End still reach every row, and the focused row scrolls into view.
- **`HintScroll.svelte`, two new options:**
  - `tag="span"`: the list can sit in an overview card's paragraph, so it is built from phrasing elements only, as before;
  - `tabbable={false}`: keeps the hints out of the tab order. Without it, Bits UI's menu focused the *▼ more* hint on opening instead of the first row.

  The defaults leave the existing uses unchanged.

## Tests (`needs-you.spec.ts`)
- **Long list (23 entries):**
  - the popup is under 400 px and inside the window;
  - the list is cut off and scrolls, with no scroll bar;
  - *▼ N more* shows and there is no *▲*;
  - End focuses the last row, in view, after which *▲ N more* shows and *▼* goes;
  - Enter opens that task.
- **Short list:** no hints.

The needs-you, projects, Kanban and dialog specs pass, as do the unit tests, the checks and the full functional run (except the two failures below).

## Also in this range, on its own commit (ATM-638's test)
The full run failed *edit your own reply in place* once. That test checked that c1 has no Edit button, but c1's replies are inside its element. So it found your reply's Edit, and it had passed only by checking before the page knew who you are. It now checks c1's own header, after your reply's Edit has shown. 5 of 5 pass.

## Not fixed here
The long review's `afterSettled` measured 107 ms against its 100 ms budget in the full run, while the WebKit real-review test was blocking for 3.5 s alongside it. Alone it measures 25 to 34 ms. ATM-641 holds it.
