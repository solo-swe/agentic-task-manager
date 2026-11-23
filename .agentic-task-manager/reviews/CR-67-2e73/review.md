+++
id = "01a1f7d2-b15e-72a0-a213-4bf6f7862e73"
number = 67
title = "The chevron in a tab opens the project list again"
status = "approved"
tasks = ["ATM-684"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T07:19:02Z
updated = 2026-11-20T09:49:29Z
+++

The chevron in a tab opens the project list again.

## The cause
Since ATM-605 and ATM-639 the tabs scroll in a strip of their own (`overflow-x: auto`). The project list was drawn inside the chevron's box (Bits UI's `ContentStatic`, placed by CSS under its button), so the strip clipped it: it opened, but nothing of it showed. The e2e test said "visible" all along, because Playwright's `toBeVisible` doesn't look at clipping.

## Change
- `ProjectSelector.svelte`: a `floating` option. With it the list is drawn over the page (a Bits UI portal and `Popover.Content`), placed beside the button at its bottom left, 6 px below. The list's content is now one snippet, shared by both ways.
- `AppTitleBar.svelte`: the tab's chooser is `floating`. The browser's title bar keeps the list as it was, so its screenshots don't change.
- `app.css`: a floating list isn't offset by its own CSS (`.project-menu.floating { position: static }`).

## Tests
- `app.spec.ts`, the title bar test: once the list is placed, what is at its middle is the list itself, and its filter takes a real click. It fails without the change (Chromium: the list is clipped). It is tagged `@webkit` now, so it runs in the app's engine too. In WebKit the list is placed a moment after it opens, so the check waits for it (`expect.poll`). It passes 3 of 3 in each engine.
- The app and projects specs (57), the selector's and title bar's unit tests, `pnpm run check`.
- I didn't get a clean full e2e run: a filter mistake started the whole suite, and its output was cut off. It ended with exit 1, most likely the WebKit timing failure fixed above, but I can't show that the rest passed.
