# Large reviews: loading and scrolling the Diffs tab (ATM-587)

Hakan, 2026-11-18: on a large review (CompMD's CR-1) the Diffs tab takes long to show, and scrolling its pane is
painfully slow. This note measures where the time goes and proposes what to change, for a decision before building.

## In short

Opening the Diffs tab is fast (the first file in about 0.1 s, atm's answer in 46 ms); **scrolling is the slow part**,
and it gets slower the further you scroll, because every file you pass stays drawn in the page. The steps proposed,
each explained under _Proposals_ below:

1. **Skip drawing files off screen:** the browser lays out and paints only the files near the view (CSS
   `content-visibility`), a few lines.
2. **Give back far files' rows:** a file a few screens away from the view drops its diff and keeps only its height;
   it is drawn again when you come back.
3. **Draw a long file in chunks:** a file of more than about 1,000 rows draws a few hundred at a time as you scroll.
4. **Cache a file at a revision:** the browser keeps each side it fetched (a file at a commit never changes), so a
   review opened again, or the next round, doesn't fetch them again.
5. **Highlight only what is shown:** a file's syntax highlighting for the rows on screen, or after they are drawn
   plain; only if the app is still slow after 1–4.
6. **Page the files from atm:** atm sends the file list first and each file's diff as it is reached; not needed for
   CR-1, a follow-up for reviews of thousands of files.
7. **Speed up atm's own answer:** not needed now (46 ms).

**Asked:** build 1–4 in this task, 5 only if needed, 6 and 7 as backlog tasks, with the budgets at the end.

## The case

CompMD's CR-1, round 2: **94 files, 2,995 lines added and 304 removed**, among them the spec (`compmd-spec-v1.md`,
394 KB a side) and many small conformance files.

## Measurements

**atm** (the running `atm service`, this Mac):

| Request                                                                          | Size   | Time  |
| -------------------------------------------------------------------------------- | ------ | ----- |
| The review's page (`…/reviews/CR-1`: header, rounds, chains, every file's patch) | 324 KB | 46 ms |
| One side of one file (`…/documents?path=compmd-spec-v1.md&at=8c7f063`)           | 394 KB | 98 ms |

The page asks for both sides of a file when its card nears the view: for CR-1 that is up to 188 requests as you
scroll, the largest two about 0.4 MB and 100 ms each, every time the review is opened (no caching: a revision's file
never changes, but the route sends no validator for it).

**The page** (a test shaped as CR-1, `reviews.spec.ts` › _a long review_: 94 files, three of 600 lines, some of 120,
the rest 12, every fifth line 240 characters; Chromium, the test's mocked server, so atm's time isn't in it):

|                                                  |                          |
| ------------------------------------------------ | ------------------------ |
| Diffs tab clicked → the first file's lines drawn | 83 ms (settled 102 ms)   |
| Pane scrolled top to bottom, a screen a step     | 60 steps in 2.7 s        |
| A step's frame, median / slowest three           | 25 ms / 130, 130, 119 ms |
| Long tasks while scrolling                       | 3, the longest 100 ms    |
| At the end: diff rows / elements in the page     | 7,788 / 40,761           |

Opening is fast. **Scrolling is where it hurts, and it gets worse the further you go:** a card's diff is made when it
nears the view and never unmade, so by the end every file's rows are in the page (40,000 elements here), and each
step's layout, style and observer work grows with them. In the app the real files add CompMD's highlighting of each
side (and rendering, for Markdown), which this test doesn't do, so the real pane is slower than these numbers.

## Proposals

Ordered by what they buy for CR-1-sized reviews, cheapest first.

1. **Cards out of view stop costing (recommended).** `content-visibility: auto` with `contain-intrinsic-size` (the
   card's last measured height) on each file card: the browser skips the layout and painting of cards off screen,
   with no change to how they are made. A few lines of CSS; it addresses the step time growing with the review.
2. **Unmake far cards (recommended).** A card more than a few screens from the view gives back its diff (its rows and
   their effects), keeping its measured height as a placeholder, and makes it again when it comes back (its sides
   already loaded, so instantly). The page then holds about the same number of rows wherever you are (hundreds, not
   thousands). The scroll pinning that keeps the chosen file still (the ResizeObserver) already copes with heights
   changing.
3. **Long files in chunks (recommended, for files over ~1,000 rows).** A file's rows drawn in chunks of a few hundred
   as they near the view, the rest a placeholder of their height; the folds of unchanged lines already make most files
   short.
4. **Cache a revision's files (recommended).** A file at a revision never changes: the `documents?…&at=<sha>` route
   answers with `ETag` the blob's ID and `Cache-Control: private, max-age=31536000, immutable` (only for a full SHA,
   never a branch name), so a review opened again, or a round compared with the next, reads its sides from the
   browser's cache. In the app, the page's own cache (`review-data.ts`, `SOURCES_CAP`) does the same per session.
5. **Highlight what is shown.** A side's highlighting (CompMD) for the rows shown rather than the whole file, or in
   idle time after the rows are drawn plain. Worth it for the spec-sized files; measure after 1–3.
6. **Paging the files from atm (not yet).** The page asks for the file list first and each file's patch as its card
   nears. For CR-1 the whole answer is 324 KB in 46 ms, so this buys little now; it matters for reviews of thousands
   of files (lock files, generated code). Proposed as a follow-up task, with a threshold (the whole answer up to, say,
   2 MB or 500 files; past it, the list first).
7. **atm's own answer (not needed now).** Rounds' trees cached and chains placed once per file: CR-1 has no chains and
   answers in 46 ms; to look at again for a review with many comments.

## What is decided, then built

Build **1 to 4** in this task, with budgets in `reviews.spec.ts` for the long review: the first file drawn under
200 ms (twice in CI), a scroll step's slowest frame under 50 ms after the first screen, and the rows in the page
bounded (no more than ~1,500 wherever the pane is); **5** if the app is still slow after them, measured on CR-1 in the
app; **6** and **7** as backlog tasks.
