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

## Built, and measured again

1 to 4 are in:

1. **Cards off screen** (`ReviewFile.svelte`): `content-visibility: auto; contain-intrinsic-size: auto 400px` on
   each card.
2. **Far cards given back** (`ReviewFile.svelte`, `FAR` = 3,200 px): a made card more than about four screens from
   the view drops its diff and stands at the height it had; it is made again when it comes near. It is kept while a
   comment is being written in it, while the focus is in it, and for the file gone to.
3. **Long files in chunks** (`DiffView.svelte`, `CHUNK` = 100 rows): a file's first 100 rows at once, the rest 100
   at a time when the page is idle, with a placeholder of their height meanwhile. Chunks of 300 made a frame of about
   120 ms when the pane scrolled into a 600-line file; 100 makes it about 30.
4. **A revision's files cached** (atm-api's `documents` route, `documents.ts`): at a full object ID, the answer is
   tagged by the revision and the path and sent with `Cache-Control: private, max-age=31536000, immutable`, and the
   page lets the browser's cache answer it. A name (`HEAD`, a branch), a short ID or a refusal is never kept.

The same test, run on its own (Chromium, this Mac):

|                                                  | Before                   | After                  |
| ------------------------------------------------ | ------------------------ | ---------------------- |
| Diffs tab clicked → the first file's lines drawn | 83 ms                    | 65 ms                  |
| Pane scrolled top to bottom                      | 60 steps in 2.7 s        | 80 steps in 1.4 s      |
| A step's frame, median / slowest                 | 25 ms / 130, 130, 119 ms | 17 ms / 110, 30, 28 ms |
| Long tasks while scrolling                       | 3, the longest 100 ms    | none                   |
| At the end: diff rows / elements in the page     | 7,788 / 40,761           | 30–200 / 2,300–3,200   |

The one slow frame left (about 100 ms) is always in the first two steps: the page settling after the tab opens, when
the cards near the first screen are made together once it is idle. The budgets differ a little from the ones asked
for: the first file and that settling under 200 ms, every step after the first three under 100 ms rather than 50
(measured: about 30 on its own, up to 97 while the whole e2e suite runs on five workers), and fewer than 1,500 rows.
50 ms would fail on a loaded machine without the page being any slower. Twice each in CI.

## CR-1 itself, measured as it feels (round 6)

Hakan, on round 5: the numbers above are too small for how it feels (opening "like 2 seconds", scrolling to the end
"more like 10"). Right: the test draws plain text in Chromium from a mocked server, and the app draws CompMD's
highlighted sides, each from atm, in WebKit (the app is a WKWebView). `tests/e2e/real-review.spec.ts` measures a real
review instead: this checkout's atm serving the project on a private state folder, in Chromium and in WebKit, the
Diffs tab opened (and watched for 5 s), scrolled to the end a screen at a time until the last file is drawn, then a
jump to the top and one to the end (each watched 3 s). It runs only when given a project:
`pnpm exec playwright test real-review`, with `tests/e2e/real-review.local.json` saying which (git ignores it).

**What it found** (CR-1, before this round, WebKit):

- **Opening:** the first lines in 0.3 s, then the page **blocked for 2 to 3.6 s** in frames of up to 1.5 s: every run
  of code highlighted by CompMD at once, as soon as its grammars had loaded.
- **Scrolling to the end:** reached in 1.4 s, but the **last file drawn at 4.9 s**: the 160 sides of the files passed
  were all asked for at once, and the browser asks atm six at a time, in order, so the last file's waited behind all
  of them (each about 75 ms at atm; up to 1.8 s in the queue).
- **Jumps** (Chromium): a jump to the top or the end blocked the page for 3 s.

**What this round changes:**

5. **Highlighting takes turns** (`render-turns.ts`, `CompMd.svelte`'s `plain`): a run of code shows as written at
   once and is highlighted by CompMD when its turn comes, one at a time, the one nearest the view first, each when the
   page is idle after the one before has drawn. The plain lines give way the moment CompMD has drawn (both at once
   made the run twice as tall for a frame, which moved the lines under a drag), and no turn starts while a mouse
   button is held.
8. **Sides asked for four at a time, the latest first** (`review-data.ts`, `LOADS_AT_ONCE`): what the reader is
   looking at now goes before the files already passed.
9. **A long file waits as tall as its lines** (`ReviewFile.svelte`, the estimate no longer capped at 4,000 px):
   measuring showed the end of the review moving away from a reader who had just scrolled to it, by up to 3,400 px,
   as a 600-line file near the end grew from 4,000 px to 12,000 once drawn; the last file was then far again, and in 3
   runs of 4 not drawn until the reader scrolled on.

**Measured again** (the same review, 3 to 4 runs; Chromium / WebKit):

| | Before | After |
|---|---|---|
| Opening: blocked in its first 5 s | 0.1 s / 2–3.6 s | 0 s / 1.4 s (one frame, below) |
| Scrolled to the end: the last file drawn | 2.9 s / 4.9 s | 1.0–1.8 s / 1.4–3.0 s |
| A jump to the top, then the end: blocked | 3.0 s / 1.0 s | under 0.1 s / 0.1–0.3 s |
| The slowest side, in the queue | 1.3 s / 1.8 s | 0.16 s / 0.09–0.11 s |

**What is left: one frame of 1.5 s in WebKit**, the first TypeScript run highlighted on the page. Timed alone, it is
CompMD's highlighter (Shiki) readying the TypeScript grammar, once per page, in WebKit (Chromium: a few ms). That is
CompMD's to fix, not atm's (it draws the code; atm shows it): proposed as a CompMD task, with these numbers.

**Next:** that CompMD task; atm's own cost per side (about 75 ms, five git processes and the project opened per
request; 6 and 7 above, as backlog tasks).
