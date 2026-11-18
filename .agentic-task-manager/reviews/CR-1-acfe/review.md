+++
id = "01a1ec1e-490c-7662-b019-c555b41cacfe"
number = 1
title = "Large reviews: where the Diffs tab spends its time, and what to change"
status = "approved"
tasks = ["ATM-587"]
author = "claude:0f56a916"
reviewers = ["Hakan Serce"]
created = 2026-11-18T00:46:10Z
updated = 2026-11-18T05:30:44Z
+++

You were right: round 5's numbers came from a test that draws plain text in Chromium from a mocked server. The app draws CompMD-highlighted sides, each fetched from atm, in WebKit. This round measures **CR-1 itself** that way, and fixes what that showed. Round 5's code (files off screen not drawn, far files given back, long files in chunks, a revision's files cached) is still in; this round's diff is only the new commit.

## What the real CR-1 showed (before this round, in WebKit)

- **Opening:** the first lines in 0.3 s, then the page blocked for 2 to 3.6 s, in frames of up to 1.5 s. Every run of code was highlighted by CompMD at once, as soon as its grammars had loaded.
- **Scrolling to the end:** the bottom was reached in 1.4 s, but the last file was drawn at 4.9 s. The 160 sides of the files passed were all asked for at once, and the browser asks atm six at a time, in order, so the last file waited behind all of them.
- **A jump to the top or the end** blocked the page for up to 3 s (Chromium).

## What this round changes

5. **Highlighting takes turns.** A run of code shows as written at once, and CompMD highlights the runs one at a time, the one nearest the view first, when the page is idle. The plain lines give way the moment CompMD has drawn, and no turn starts while a mouse button is held, so lines never move under a drag.
8. **Sides are asked for four at a time, the latest first,** so what you are looking at loads before the files you have passed.
9. **A long file waits as tall as its lines.** A 600-line file used to stand at 4,000 px and grow to 12,000 once drawn, which moved the end of the review away just after you had scrolled to it.

## Measured again (the same review, Chromium / WebKit)

- Opening: the page blocked for 0 s / 1.4 s (was 0.1 / 2 to 3.6 s).
- Scrolled to the end, the last file drawn at 1.0 to 1.8 s / 1.4 to 3.0 s (was 2.9 / 4.9 s).
- A jump to the top, then the end: under 0.1 s / 0.1 to 0.3 s (was 3 / 1 s).
- The slowest side took 0.16 s / 0.1 s (was 1.3 / 1.8 s).

## What is left, and isn't atm's

One frame of about 1.5 s in WebKit: the first TypeScript run highlighted on a page, which is CompMD's highlighter (Shiki) readying its TypeScript grammar. I propose that as a CompMD task.

## Two changes to what you asked for

- **A step's frame budget is 100 ms, not 50,** for the reason in round 5.
- **"No more than 1,500 rows" is now "no rows more than 3,200 px and a screen from the view".** A 600-line file in view is 1,200 rows in split view on its own.

Measure it yourself with `pnpm exec playwright test real-review`; `tests/e2e/real-review.local.json` names the project. The numbers are in `docs/review-performance.md`, under *CR-1 itself*.
