+++
id = "01a1ec1e-490c-7662-b019-c555b41cacfe"
number = 1
title = "Large reviews: where the Diffs tab spends its time, and what to change"
status = "changes_requested"
tasks = ["ATM-587"]
author = "claude:0f56a916"
reviewers = ["Hakan Serce"]
created = 2026-11-18T00:46:10Z
updated = 2026-11-18T01:00:14Z
+++

Where a large review's Diffs tab spends its time, measured on CompMD's CR-1 (94 files, ~3,300 changed lines) and a test shaped like it, and what to change.

**Found:** opening is fast (the first file in about 0.1 s; atm's answer for CR-1 in 46 ms). **Scrolling is the slow part** and gets slower the further you go: every file you pass stays drawn, so by the end the page holds every row (7,788 rows and 40,761 elements in the test), with frames of up to 130 ms.

## Asked: build these four in this task

1. **Skip drawing files off screen:** the browser lays out and paints only the files near the view (CSS `content-visibility: auto` on each file's card, sized by its last height). A few lines; it stops the scroll getting slower as the review grows.
2. **Give back far files' rows:** a file more than a few screens from the view drops its drawn diff and keeps only its height; when you scroll back it is drawn again at once (its two sides are already loaded). The page then holds a few hundred rows wherever you are, not every row of the review.
3. **Draw a long file in chunks:** a file of more than about 1,000 rows draws a few hundred at a time as they near the view.
4. **Cache a file at a revision:** each side of a file the page fetches (a file at a commit never changes) is kept by the browser (`ETag` and an immutable cache header on `documents?…&at=<sha>`), so reopening the review, or comparing the next round, fetches nothing again.

## Later, or only if needed

5. **Highlight only what is shown:** only if the app is still slow after 1–4.
6. **Page the files from atm** and 7. **speed up atm's own answer:** not needed for CR-1; backlog tasks.

## Budgets after 1–4
The first file drawn under 200 ms; a scroll step's slowest frame under 50 ms after the first screen; no more than ~1,500 rows in the page wherever it is scrolled.

The details and the numbers: `docs/review-performance.md` (the file of this review).
