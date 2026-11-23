+++
id = "01a1f83f-ab2f-735c-beed-4aa1a81431d7"
number = 71
title = "The selected tab has no line under it again"
status = "approved"
tasks = ["ATM-688"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T09:18:04Z
updated = 2026-11-20T09:52:11Z
+++

The selected title bar tab has no line under it again. It joins the page under it, as a browser's selected tab does; the other tabs keep the line.

## The cause
- **How it was meant to work:** the bar draws a 1 px line at its bottom. Each tab reached 1 px below the bar (`margin-bottom: -1px`), and the selected one, in the page's background, covered the line.
- **What broke it:** since ATM-605 the tabs sit in a scroller of their own (`overflow-y: hidden`), sized to the bar's content box. It clipped that pixel, so the line ran under the selected tab too.

## Change (`ui/TitleBarTabs.svelte`)
- The scroller reaches 1 px below its room, over the bar's line.
- The tabs fill it, so the tab's own −1 px margin is gone.
- The selected tab, opaque, covers the line; the others are transparent and leave it showing.

## Test
`app.spec.ts`, tagged `@webkit`: three tabs, the middle one selected. The test checks that:
- the strip reaches the line's row;
- the selected tab reaches over it with an opaque background;
- the other tabs are transparent;
- what is drawn at the line's row under the selected tab is that tab.

It fails without the change (the strip stops 1 px short) and passes with it, in Chromium and WebKit. I also looked at a screenshot: the middle tab joins the page, the others keep the line.

## Checks
- **Suites:** `pnpm run check`, the strip's and title bar's unit tests, the app, projects and reviews specs (133), and the WebKit project.
- **Along the way:** the WebKit project showed ATM-656's jump test failing 1 run in 8. That came from a rule in ATM-687, now taken out in CR-70's round 2; the test passes 8 of 8 again.
