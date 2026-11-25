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

## CompMD 0.8.0 (ATM-648)

CompMD 0.8.0 has both fixes the review's pages waited for:
- in WebKit, Oniguruma's WebAssembly highlights, where Shiki's JavaScript engine compiled each regex the first time it
  ran: the 1.5 s frame above;
- the CompMD pages of one document share one style sheet, and read the colour scheme only for a frame or a component
  (ATM-646's proposal): opening or closing a page no longer restyles the whole document.

Measured the same way on 0.7.1 and on 0.8.0, on this Mac, atm 0.41.0 otherwise unchanged:
- the timed e2e tests, one worker, three runs each, medians;
- the real review (CompMD's CR-1, 94 files), one run per engine.

**The real review** (`real-review.spec.ts`; 0.7.1 → 0.8.0):

| | Chromium | WebKit |
|---|---|---|
| Opening: blocked (its slowest frame) | 167 → 83 ms (167 → 83) | **1,712 → 92 ms (1,509 → 92)** |
| Opening: busy until | 781 → 454 ms | 1,953 → 286 ms |
| Opening: the first lines | 517 → 271 ms | 135 → 127 ms |
| Scrolled to the end: the last file drawn | 2,264 → 1,978 ms | 2,188 → 2,013 ms |
| Scrolling: blocked | 183 → 133 ms | 144 → 0 ms |
| A jump to the top: blocked | 0 → 0 ms | **1,594 → 78 ms** |
| A jump to the end: blocked (long frames) | 0 → 0 ms | 715 → 59 ms (10 → 1) |
| The sides: their time at atm in all (the slowest) | 26.7 → 13.4 s (492 → 240 ms) | 2.2 → 1.3 s (239 → 109 ms) |

**The timed e2e tests** (`reviews.spec.ts`, `performance.spec.ts`; Chromium, medians of three):

| | 0.7.1 | 0.8.0 |
|---|---|---|
| A jump to a long review's last file: sheets set / styles read | 10 / 28 | **0 / 0** |
| The long review: the first file drawn / settled | 54 / 101 ms | 38 / 68 ms |
| The long review: the slowest frame after it settled | 43 ms | 23 ms |
| The long review: DOM nodes alive after down and back | 21,347 | 22,051 |
| A file far down a large review: total | 62 ms | 55 ms |
| The + following the pointer: median / slowest frame | 33 / 34 ms | 33 / 34 ms |

The jump test now holds the restyles at none (`reviews.spec.ts`). The tab switches and reloads of
`performance.spec.ts` render no CompMD, and are within their run-to-run spread: a first 0.8.0 run measured them as
0.7.1 had (the agents tab 98 ms, the graph 110, the list 124), a later quieter one lower (67, 79, 78).

**Sizes:** CompMD's WebAssembly 2,812,614 → 2,960,440 bytes (+5%); `compmd.js` 82,744 → 98,690 bytes; the frames'
runtime 71,941 → 81,004 bytes; the dashboard's page unchanged (1,058,092 bytes).

## CompMD 0.9.0 (ATM-703)

CompMD 0.9.0 moves code highlighting out of the frames: the host's page makes Shiki once and answers each frame's
blocks, one block a task (CompMD measures ten frames blocking a WebKit page 0.4 s instead of 2.8 s). Its processor
lists only the components a document uses, so a document without a diagram loads no Mermaid. The review's diffs are
host-mode pages, not frames, so their highlighting was already the page's; the change reaches the task text and
documents that draw a frame (a diagram).

Measured the same way on 0.8.0 and on 0.9.0, on this Mac, atm 0.41.0 otherwise unchanged: the timed e2e tests, one
worker, three runs each, medians.

**The timed e2e tests** (`reviews.spec.ts`, `performance.spec.ts`; Chromium, medians of three; 0.8.0 → 0.9.0):

| | 0.8.0 | 0.9.0 |
|---|---|---|
| A jump to a long review's last file: sheets set / styles read | 0 / 0 | 0 / 0 |
| The long review: the first file drawn / settled | 36 / 70 ms | 37 / 71 ms |
| The long review: the slowest frame after it settled | 24 ms | 23 ms |
| The long review: DOM nodes alive after down and back | 22,116 | 22,116 |
| A file far down a large review: total | 85 ms | 87 ms |
| The + following the pointer: median / slowest frame | 33 / 34 ms | 33 / 34 ms |
| Tab switches (500 tasks): Kanban / Progress / Agents / Graph / List | 21 / 47 / 68 / 79 / 79 ms | 24 / 48 / 67 / 79 / 78 ms |
| A reload that changes the data: Graph / Kanban / Progress | 60 / 51 / 67 ms | 53 / 51 / 52 ms |

All within their run-to-run spread but the reload's Progress tab (66–68 ms on 0.8.0, 52–54 on 0.9.0, every run).

**The real review** (CompMD's CR-1, 94 files; `real-review.spec.ts`, one run per engine, in the full e2e runs; the
0.8.0 run was before ATM-693's jump fix, so the jumps compare both changes):

| | Chromium | WebKit |
|---|---|---|
| Opening: blocked | 83 → 83 ms | 102 → 92 ms |
| Opening: busy until | 251 → 272 ms | 272 → 249 ms |
| Scrolled to the end: the last file drawn | 2,048 → 1,929 ms | 2,022 → 2,017 ms |
| Scrolling: blocked | 133 → 117 ms | 0 → 0 ms |
| A jump to the top: blocked | 0 → 0 ms | 74 → 70 ms |
| A jump to the end: blocked (long frames) | 0 → 0 ms | 107 → 51 ms (2 → 1) |
| The sides: their time at atm in all (the slowest) | 15.3 → 10.4 s (159 → 110 ms) | 1.1 → 1.1 s (89 → 85 ms) |

**Sizes:** CompMD's WebAssembly 2,960,441 → 2,967,257 bytes (+0.2%); `compmd.js` 98,693 → 99,956 bytes; the frames'
runtime 81,004 → 82,429 bytes; the dashboard's page unchanged (1,084,296 bytes).

## CompMD 0.10.0 (ATM-780)

CompMD 0.10.0 (the spec's drafts 17 and 18) adds to what a document can write (type in a box, image boxes, a
document's own fonts, gradients, no-code components, the `lang` and `dir` a renderer sets), and nothing a host must
change: atm builds and its checks pass with no change but the pins. A document's own fonts a host can't serve show the
fallback, as atm's page policy does (no font source is allowed).

Measured on 0.9.0 and then on 0.10.0, on this Mac, atm 0.43.0 otherwise unchanged, medians of three: the timed e2e
tests on one worker, the real review (`--project measure`, its two engines side by side, three runs each). The real
review ran on three workers in both cases, so its numbers compare with each other, not with the section above (one run
each, in the full e2e runs).

**The timed e2e tests** (Chromium; 0.9.0 → 0.10.0):

| | 0.9.0 | 0.10.0 |
|---|---|---|
| A jump to a long review's last file: sheets set / styles read | 0 / 0 | 0 / 0 |
| The long review: the first file drawn / settled | 38 / 69 ms | 36 / 66 ms |
| The long review: the slowest frame after it settled | 29 ms | 28 ms |
| A file far down a large review: total | 90 ms | 74 ms |
| The + following the pointer: median / slowest frame | 33 / 34 ms | 33 / 35 ms |
| Tab switches (500 tasks): Kanban / Progress / Agents / Graph / List | 31 / 48 / 64 / 61 / 84 ms | 30 / 48 / 58 / 61 / 86 ms |
| A reload that changes the data: Graph / Kanban / Progress | 55 / 59 / 57 ms | 54 / 47 / 56 ms |

All within their run-to-run spread.

**The real review** (CompMD's CR-1, 94 files; 0.9.0 → 0.10.0):

| | Chromium | WebKit |
|---|---|---|
| Opening: the first file drawn | 400 → 201 ms | 401 → 235 ms |
| Opening: blocked | 183 → 100 ms | 413 → 217 ms |
| Opening: busy until | 715 → 366 ms | 794 → 486 ms |
| Scrolled to the end: the last file drawn | 2,292 → 2,070 ms | 2,081 → 2,080 ms |
| A jump to the top: blocked | 0 → 0 ms | 199 → 100 ms |
| A jump to the end: blocked | 0 → 0 ms | 239 → 119 ms |
| The sides: their time at atm in all (the slowest) | 36.1 → 27.1 s (528 → 305 ms) | 4.8 → 3.3 s (644 → 426 ms) |

A review opens about twice as fast in both engines, the app's WebKit included, and a jump in WebKit blocks half as
long.

**Sizes:** CompMD's WebAssembly 2,967,257 → 3,058,330 bytes (+3.1%); `compmd.js` 99,956 → 117,611 bytes (+17.7%);
the dashboard's page 1,165,584 → 1,165,586 bytes.

**A test made to wait for the page (`reviews.spec.ts` `openDiffs`):** 0.10.0 is still drawing a file's code a moment
after its line numbers show, so a test that measured a line and then pointed at it could point at another (the drag
over three lines, 3 runs in 5). It waits now for CompMD to say every block of the file is drawn (`data-compmd="ready"`).
