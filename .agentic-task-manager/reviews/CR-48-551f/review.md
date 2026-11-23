+++
id = "01a1f24f-0117-7449-be5e-6d34c7f6551f"
number = 48
title = "The tab strip keeps its room after tabs closed while scrolled (WebKit)"
status = "approved"
tasks = ["ATM-639"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T05:37:06Z
updated = 2026-11-19T06:12:09Z
+++

The bug: in the app, the tab strip got narrower when tabs were closed while it was scrolled right. This fixes it.

## Cause (WebKit only: the app's webview)
I measured it in Playwright's WebKit, which the app's engine matches. Two things were wrong:
1. **The scroller was sized by its tabs less what was scrolled off.** With 3 tabs left (660 px of tabs) and the strip scrolled 222 px, it came out 438 px wide instead of its 506 px room.
2. **The box around it was sized by the arrows' and the +'s glyphs, not their flex basis** (22 and 28 px). That left it 53 px short, so even 2 tabs that fit got ‹ › and scrolled. This happened after a fresh load too: the existing ATM-605 test's last step fails in WebKit.

Chromium uses the flex basis and not the scroll offset, which is why the e2e suite (Chromium only) never saw either.

## Change (`ui/TitleBarTabs.svelte`)
- The scroller is absolutely positioned inside a plain box of the tabs' own width (count × tab width). The box shrinks to its room, so the scroll offset plays no part in sizing.
- The arrows and the + have a `width` as well as their flex basis.

Nothing else changes; Chromium measures the same as before.

## Tests
The ATM-605 e2e now scrolls to the end, then closes the last tabs one by one:
- from 11 tabs down to 3, the strip keeps its room with the last tab flush at its end;
- at 2 tabs there are no arrows and nothing is scrolled.

It passes in Chromium, and in WebKit through a scratch config, which I didn't commit. Before the fix it failed in WebKit at 3 tabs (438 instead of 506) and at 2 tabs (arrows shown).

The app, tabs and title bar specs (37), the unit tests and the checks pass.

**A question for you:** running the app specs in WebKit in CI would catch this kind of bug. Do you want that as a task? It needs WebKit installed in CI's Playwright job.
