# The review's diff viewer: memory and rendering, profiled

Hakan, 2026-11-18: before choosing how to make the Diffs tab lighter, understand what it costs: how many CompMD
renderers it makes, what each holds, and what that adds up to on a large review. This note collects what is measured
so far. The options are agreed with Hakan from it, then built. The earlier work on the same tab, what it built and its
numbers, is in [review-performance.md](review-performance.md) (ATM-587).

**Status** (2026-11-18): memory measured, and what holds the destroyed renderers found: two causes, one a bug of
ours (_What holds them_, below). Still to measure: CompMD's own share of a renderer's cost, one renderer's latency
from mount to drawn, the rendered Markdown view, and WebKit (the app). The options at the end wait for Hakan.

## How the tab uses CompMD

There is no renderer for the whole review, and none for a whole file. There is one per run of lines, per side.

- **Source view** (`DiffView.svelte`). A file's diff is cut into runs: the rows between the *Show n unchanged lines*
  folds and the comment chains. Each run is a `DiffCode`, which is one `<CompMd>` holding that run's lines as a fenced
  code block (`DiffCode.svelte`). In split view, a run has two: the old side and the new. Unified view has one.
- **Rendered view** (`RenderedSide.svelte`, Markdown). Each side has one `<CompMd>` per part shown, a part being the
  blocks near a change, with folds between parts. Every part's renderer gets **the whole document** and
  `only={[from, to]}`, the blocks it shows (ATM-591), so headings, footnotes and frontmatter stay right. A document
  shown in five parts is processed five times a side.
- **Which files have renderers.** Only cards near the view are made (`MARGIN`), and a card more than `FAR` (3,200 px)
  away gives back its diff and keeps only its height (ATM-587). A card coming back is made again from nothing.
- **Each `<CompMd>` is a CompMD sandbox view of its own** (`createSandbox(…).open(…)` in `CompMd.svelte`), with its
  listeners (`rendered`, `error`, `layout`) and a `MutationObserver` on `<html>` for the theme.

## The case

CompMD's CR-1, round 2: 94 files, 2,995 lines added and 304 removed, the same review as ATM-587's.

## Method

This checkout's atm (`target/debug/atm _serve`) served CompMD's project on a private state folder, with this branch's
dashboard (ATM-615's fixes in). Playwright's Chromium drove it in a 1440 × 900 window, with the diff in its default
layout and the source view. The scripts were one-offs, not committed:

- **Renderers:** each mounted `<CompMd>` is one `.compmd` element. A `MutationObserver` counted every one added (made),
  and the page was asked how many were in it (alive).
- **Memory:** the DevTools protocol, after three forced garbage collections (`HeapProfiler.collectGarbage`):
  `Performance.getMetrics`' `JSHeapUsedSize`, `Nodes` (every DOM node alive, in the page or not) and
  `JSEventListeners`. These were compared with the elements in the page (`getElementsByTagName('*')`).
- **Moves:** the pane scrolled to the end a screen a frame (as fast as the page can draw), then back to the top. Jumps
  were clicks on files in the tree.

## How many renderers

| What was done | Alive after | Most at once | Made by it | Diff rows |
| --- | --- | --- | --- | --- |
| Diffs tab opened, first screen settled | 26 | 26 | 27 | 320 |
| Scrolled to the end, a screen a frame | 0 at once, 22 settled | 40 | 192 | 0, then 362 |
| A jump to file 1 | 26 | 26 | 26 | 320 |
| A jump to file 48 | 22 | 40 | 22 | 210 |
| A jump to file 94 | 22 | 24 | 22 | 558 |
| A jump to file 2 | 26 | 26 | 26 | 320 |
| A jump to file 24 | 16 | 28 | 16 | 216 |

- **What is alive is bounded by the view:** 16 to 32 renderers wherever the pane is, 40 at most.
- **Moving makes many.** A pass to the end and back makes about **410** renderers, about 15 times what is on screen.
  Scrolling up makes more than scrolling down: about 366 against 146 to 204. Cards pass through the margin, are made,
  and are given back.
- **Nothing is reused.** A jump back to a file seen a moment ago makes its renderers again.

## Memory

After forced garbage collections:

| State | JS heap | DOM nodes alive | Elements in the page | Listeners | Renderers alive / made |
| --- | --- | --- | --- | --- | --- |
| Overview tab | 5.9 MB | 1,314 | 266 | 72 | 1 / 1 |
| Diffs tab, first screen | 20.4 MB | 10,932 | 3,693 | 312 | 26 / 27 |
| Scrolled to the end, pass 1 | 58 MB | 60,508 | 5,143 | 839 | 22 / 231 |
| Back to the top, pass 1 | 82.6 MB | 107,377 | 3,693 | 1,429 | 26 / 597 |
| Scrolled to the end, pass 2 | 98.6 MB | 116,871 | 5,143 | 1,533 | 22 / 1,013 |
| Back to the top, pass 2 | 110.7 MB | 117,233 | 4,510 | 1,597 | 32 / 1,429 |
| 15 s later, nothing done | 110.6 MB | 117,234 | 3,693 | 1,553 | 26 / 1,429 |
| After 20 jumps between files | 98.5 MB | 64,291 | 4,868 | 994 | 26 / 1,535 |

A run of five passes, measured the same way, levelled off at 104 to 106 MB and 113,000 to 117,000 nodes.

- **A renderer with its rows** costs about **0.55 MB of heap and 370 DOM nodes**. Going from the Overview to the first
  screen adds 14.5 MB and 9,600 nodes for 26 renderers and 320 rows. This includes the rows' highlighted markup;
  CompMD's own share isn't separated yet.
- **What is needed is small:** 16 to 32 renderers alive, about 9 to 18 MB.
- **What is held is not.** Renderers that are gone stay in memory: the page has 3,700 to 5,100 elements while about
  117,000 nodes are alive. Listeners grow from 312 to about 1,550 the same way. Forced garbage collection and 15 s of
  idle free none of it, so it isn't garbage waiting to be collected: something holds it.
- **It levels off.** It doesn't grow with every pass: it stops around 105 to 111 MB and 117,000 nodes, and 20 jumps
  between files brought it down to 64,000 nodes. So a bounded or rotating set of old renderers is held, not every one
  forever.
## What holds them

**How it was found.** Every `.compmd` element got a marker object of a class of its own as it was added. After two
passes to the end and back, a heap snapshot was taken, and each marker followed back to a GC root by its shortest
strong path. 544 markers, none of them garbage. 512 were on detached elements, and only the 26 to 32 in the page
should be alive.

**1. A render after its renderer was destroyed (ours, the most of it).** Every detached renderer was held the same
way:
1. a `MediaQueryList` `change` listener;
2. a closure;
3. CompMD's colour-scheme watcher (minified `_n`);
4. its target, `#t`, the `.compmd-page` element;
5. the whole card's detached diff, through it.

CompMD's renderers remove that listener in `destroy()`, so the views holding it were never destroyed. That is our
doing. `render-turns.ts` takes the next turn off the queue, then waits up to 150 ms for the page to be idle before
rendering. A card given back in that time destroys its `CompMd`, whose cancel then finds nothing in the queue, and
the render runs anyway. `draw()` makes a CompMD view in the detached element, after `onDestroy` had found no view to
destroy. The markers show it: the held elements' outer `.compmd` says `data-compmd="loading"` (Svelte's last state,
before it was destroyed), and their `.compmd-page` inside it says `data-compmd-settled` (CompMD drew it afterwards).

A fix was tried, and taken out again until the options are agreed: a cancelled turn is skipped even once picked, and a
destroyed `CompMd` never draws. Measured the same way:

| | Before | With the fix |
| --- | --- | --- |
| Two passes to the end and back: heap | 110.7 MB | 74.9 MB |
| The same: DOM nodes alive | 117,233 | 83,214 |
| The same: listeners | 1,597 | 1,156 |
| After 20 jumps between files: heap, nodes | 98.5 MB, 64,291 | 52.4 MB, 27,411 |

**2. Chromium's cached layout of skipped cards (not ours, bounded).** With the fix in, 400 detached renderers were
still held, by a different path: the frame's view, its scrollable areas, the pane's `PaintLayerScrollableArea`, then
a `LayoutResult` and its tree of `PhysicalBoxFragment`s, down to the layout objects of the removed diff. A card off
screen is skipped (`content-visibility: auto`), so its layout isn't redone when its diff is given back. Its last
layout, cached, still points at the removed rows until the card is laid out again. That is why the memory levels off
(about one old diff a card) and why jumps, which lay cards out again, free some of it. It is Chromium's; WebKit, the
app's engine, may keep less, or more: to measure.

## Rendering latency

Measured so far (Chromium), from ATM-587, ATM-615 and the long-review spec (`reviews.spec.ts`: 94 files shaped as
CR-1, plain code, a mocked server):

| | On its own | CPU slowed 4× |
| --- | --- | --- |
| Diffs tab clicked → the first file's lines drawn | 54–65 ms | 184–198 ms |
| A scroll step's frame, median / slowest after settling | 17 / about 30 ms | 17 / 46–50 ms |
| Long tasks while scrolling | none | none, or one of about 64 ms |

On CR-1 itself, in WebKit (the app), [review-performance.md](review-performance.md) (*CR-1 itself*) measured the
last file drawn 1.4 to 3.0 s after scrolling to the end, and one frame of 1.5 s, CompMD's highlighter readying the
TypeScript grammar once per page.

Still to measure: the cost of one renderer from mount to drawn (processing, the sandbox view, highlighting), the
rendered view's repeated processing per part, and the same numbers in WebKit.

## Options

To be agreed with Hakan once the measurements above are complete. The ones the data points at so far:

- **Fix the render after destroy (recommended, small).** The fix tried above: a cancelled turn never renders, and a
  destroyed `CompMd` never draws. About 36 of 110 MB, and half the nodes after jumps, for a few lines and their tests.
- **Free a skipped card's old layout.** Give a card's diff back only once it is laid out again (or lay it out once
  then, cheaply, since it is small without its diff), so Chromium drops the cached fragments of the removed rows.
  About 50 MB more on CR-1; to weigh against the layout it costs while scrolling, and to check in WebKit.
- **Fewer renderers per file:** one per side per file instead of one per run. CompMD would draw a file's runs with
  gaps between them, which is close to CMD-250 (the lines' boxes).
- **Reuse instead of remake:** keep a few recently given-back cards' renderers, or reuse sandbox views from a pool,
  so a fast pass or a jump back doesn't build them again.
- **Make fewer while moving:** don't make cards the pane is only passing through quickly (a card made only once the
  scroll slows near it).
- **The rendered view: process the document once a side,** not once a part, and give each part its blocks.

## Upstream (CompMD)

What only CompMD can change goes to it as tasks, with these numbers. The snapshot found no leak of CompMD's: its
views free their listeners when destroyed. What is left for it: drawing one text in ranges with gaps (for one
renderer a side per file), and a processed tree several views can share. A smaller one: a view's colour-scheme watcher
could listen once a page, not once a view (a page of 30 views holds 30 `matchMedia` listeners).
