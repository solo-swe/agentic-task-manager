+++
id = "01a1fa31-bec6-73f6-9c17-0be813b6bbcb"
number = 90
title = "The regular themes board scrolls as a whole in the app: columns capped at the board (100cqh)"
status = "approved"
tasks = ["ATM-707"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T18:22:06Z
updated = 2026-11-21T20:36:57Z
+++

Your report: in the app, in every regular theme, the Kanban and the Omniboard scroll as a whole with a scrollbar. The sticky-notes and hand-drawn themes are fine.

**Cause.** A regular theme keeps a column to the board's height with `max-height: 100%`, a percentage of the board's height, and that height comes from flexing. Your screenshot shows the app's WebKit not resolving that percentage: no column's bottom is visible, so the columns grew to their cards. The board has `overflow-x: auto`, which lets it scroll vertically too, and it did, with its scrollbar showing. The drawn themes stretch their columns (`--board-align: stretch`), which sizes them without any percentage.

**Fix (`app.css`).** The two full-height boards, the project's and the Omniboard's, are now size containers, and their columns are capped at `100cqh`, measured from the board's own box. The look doesn't change: columns are as tall as their cards, up to the board's height.

**Why we didn't catch it**
- The board's scroll tests ran in Chromium only.
- The screenshots are Linux Chromium, which draws no scrollbars.
- No test looked at a long column inside the app's frame.

I've added those tests:
- `app.spec.ts`, tagged `@webkit`: in the app's frame, in the default and sticky-notes themes, in a 1280×720 and a 1000×615 window. The board doesn't scroll as a whole, a long column scrolls inside it with no scrollbar taking room, and in the large window neither does the tab nor the page.
- `kanban.spec.ts`'s columns test now runs in WebKit too, and checks that the board doesn't scroll.

**Please check it in the app.** Playwright's WebKit resolves the old percentage, so these tests passed before the fix as well. They guard the layout, but they never showed your bug. Run `cargo tauri dev` and open the Kanban and the Omniboard in Default.

The full e2e suite passes in both engines: 549 tests.
