# The review's diff viewer: memory and rendering, profiled (ATM-620)

Hakan, 2026-11-18: before choosing how to make the Diffs tab lighter, understand what it costs: how many CompMD
renderers it makes, what each holds, what that adds up to on a large review, and where the time goes. This document
is that analysis. The options at the end are for Hakan to decide; nothing is built yet. The earlier work on the same
tab, what it built and its numbers, is in [review-performance.md](review-performance.md) (ATM-587).

## Executive summary

**The case:** CompMD's CR-1 (94 files, about 3,000 lines changed). One scripted session was measured in Chromium and in
WebKit, the app's engine: opening the tab, scrolling fast, skimming, jumping between files, and the rendered Markdown
view. The page was measured as it is, and in eight variants, each changing one thing. Three runs each; medians below.

**Memory: the tab keeps what it has thrown away.** It needs about 21 MB for what is on screen, but after a skim from
the end to the top it holds **175 MB and 171,000 DOM nodes**, while the page has about 4,600 elements. Two causes, both
found and proven by experiment:

1. **A bug of ours:** a highlighting turn can run after its renderer was destroyed, and makes a CompMD view nothing ever
   destroys. That view's listener keeps the whole card's diff alive.
2. **`content-visibility: auto` on the file cards** (ATM-587): Chromium keeps a skipped card's last layout, which still
   points at the diff the card has given back. It is now redundant, since far cards give their diffs back anyway.

**Fixing both keeps the tab at 21–37 MB and about 13,000 nodes through the whole session** (from up to 175 MB and
171,000), with no measurable cost in speed in either engine. Each fix alone does not: the leak fix alone leaves 158 MB
after a skim, and removing `content-visibility` alone leaves 46–56 MB.

**Speed: Chromium is fine; WebKit, the app, is not, and that is CompMD's highlighting.** In WebKit a skim from the end to
the top is janky for **10–11 s of its 31 s** (130–170 frames over 50 ms, some over 1 s). Opening the tab blocks once
for **1.3 s**, and jumps between files for 1–2.5 s. With the same page and no highlighting of the code runs, WebKit
blocks for **0.25 s** in that skim, and for none on opening. So about 97% of the app's jank is highlighting in WebKit:
none of our memory fixes changes it, and only CompMD can make it fast.

**Recommendations:**

| | What | Gain (measured) | Cost | Who |
| --- | --- | --- | --- | --- |
| **1** | Fix the render after destroy | with 2: the whole session flat at 21–37 MB | about 15 lines, tests | atm |
| **2** | Drop `content-visibility` from the cards | with 1: as above | 2 lines of CSS, tests | atm |
| **3** | A memory bound in the long-review spec | catches either coming back | a test | atm |
| **4** | Ask CompMD for faster highlighting in WebKit | the app's jank: up to 97% of it | CompMD's work | CompMD |
| **5** | Ask CompMD for a highlight cache (a run seen again is free) | jumps and scrolling back | CompMD's work | CompMD |
| 6 | The rendered view processes a document once a side, not once a part | estimated, to measure first | small | atm |
| — | Not now: re-laying out given-back cards (no gain), holding cards while scrolling fast (no visible gain, needs work), one renderer per file (small gain, large cost) | | | |

1–3 are small and certain, and fix memory everywhere. 4 and 5 are what will make the app feel smooth; they are
CompMD's, so they go to it as tasks with these numbers. 6 is worth measuring when 1–3 are built.

## How the tab uses CompMD

There is no renderer for the whole review, and none for a whole file: there is one per run of lines, per side.

- **Source view** (`DiffView.svelte`). A file's diff is cut into runs: the rows between the *Show n unchanged lines*
  folds and the comment chains. Each run is a `DiffCode`, which is one `<CompMd>` holding that run's lines as a fenced
  code block (`DiffCode.svelte`). In split view a run has two, the old side and the new; unified has one.
- **Rendered view** (`RenderedSide.svelte`, Markdown). Each side has one `<CompMd>` per part shown (the blocks near a
  change), with folds between parts. Every part's renderer gets **the whole document** and `only={[from, to]}`, the
  blocks it shows (ATM-591), so headings and footnotes stay right: a document shown in 13 parts is processed 13 times a
  side.
- **Which cards have renderers.** Only cards near the view are made (`MARGIN`, 800 px); a card more than `FAR`
  (3,200 px) away gives back its diff and keeps only its height (ATM-587). A card coming back is made again from
  nothing.
- **Highlighting takes turns** (`render-turns.ts`, ATM-587): a run shows its text plain at once, and is highlighted by
  CompMD when its turn comes, one at a time, nearest the view first, when the page is idle.
- **Each `<CompMd>` is a CompMD sandbox view of its own** (`createSandbox(…).open(…)` in `CompMd.svelte`), with its
  listeners and a colour-scheme watcher.

## Method

**The page:** this checkout's atm (`target/debug/atm _serve`) serving CompMD's project on a private state folder,
CR-1 round 2, in a 1440 × 900 window, split view, Source.

**The engines:** Playwright's Chromium, and Playwright's WebKit (the engine of the app's WKWebView on macOS, not the
app itself). All runs one at a time, on this Mac, three per variant; tables give the medians.

**The session**, every phase watched frame by frame:

| Phase | What it does |
| --- | --- |
| open | the Diffs tab clicked, watched 6 s |
| fast | to the end, a screen a frame, then 3 s at the end |
| reader | back to the top, a screen every 250 ms (a skim), then 3 s |
| fast2 | to the end and back to the top, a screen a frame, then 3 s |
| jumps | six files gone to from the tree, each watched 2 s |
| rendered | the Rendered view, the spec (`compmd-spec-v1.md`, 394 KB) gone to, watched 6 s |

**What is measured:**

- **Frames:** every frame's length from `requestAnimationFrame`. *Blocked* is the time over budget in frames over
  50 ms, summed. *Max* is the longest frame.
- **Renderers:** every `.compmd` element added, when CompMD drew it (`data-compmd-settled`), and when it was removed.
  *Made*, *wasted* (removed before it was drawn), and the time from mounted to drawn.
- **When the reader sees it:** the first diff line, the first highlighted run, and, after a scroll stops, when the
  runs in view are all highlighted.
- **Memory (Chromium only):** the DevTools protocol after three forced garbage collections: the JS heap, DOM nodes alive
  (in the page or detached) and listeners. WebKit has no such protocol in Playwright.
- **Long animation frames (Chromium):** which scripts the long frames ran.
- **What holds memory:** heap snapshots. Every `.compmd` element got a marker object of a class of its own, and each
  marker was followed back to a GC root by its shortest strong path.

**The variants**, each one patch on the committed page, built, served and measured the same way:

| Variant | What it changes |
| --- | --- |
| baseline | nothing |
| leak fix | a cancelled highlighting turn never runs, even once picked; a destroyed `CompMd` never draws |
| leak fix + relayout | also: a card laid out once after its diff is given back |
| no-cv | the cards' `content-visibility: auto` removed |
| leak fix + no-cv | both |
| leak fix + no-cv + fast | also: while the pane moves half a screen or more per scroll event, cards wait (`holdNearCards(150)`) |
| plain | leak fix + no-cv, with the code runs drawn as plain text, no CompMD (CompMD's share) |

The harness (a Playwright script with the phases above, a heap-snapshot retainer finder) and the variant patches are
in the session's scratch folder, not in the repository. The implementation commits the harness as an extension of
`tests/e2e/real-review.spec.ts`, so the numbers can be taken again after each change; the patches are only these
experiments.

## Findings: renderers

| Phase (Chromium, baseline) | Made | Wasted | Alive after | Mounted → drawn, median |
| --- | --- | --- | --- | --- |
| open | 26 | 0 | 26 | 473 ms (a queue of 26, one at a time) |
| fast | 182 | 151 | 22 | 212 ms |
| reader | 430 | 1 | 32 | 166 ms |
| fast2 | 828 | 689 | 32 | 95 ms |
| jumps (six) | 130 | 0 | 38 | 311 ms |

- **What is alive is bounded by the view:** 16 to 38 renderers wherever the pane is.
- **Moving makes many, and most are wasted when moving fast.** A fast pass down and back makes about 830 renderers, of
  which about 690 are removed before they are drawn. A skim makes about 430: every run of every file, once.
- **Nothing is reused:** a file seen a moment ago is made, and highlighted, again.
- In WebKit, a fast scroll makes far fewer (about 28 against 182): its frames are slower, so the pane passes more cards
  between two of them.

## Findings: memory (Chromium)

**Medians, heap and DOM nodes alive, at the end of each phase:**

| Variant | open | fast | reader | fast2 | jumps | rendered |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 20.6 MB, 11k | 56 MB, 59k | **175 MB, 171k** | 84 MB, 115k | 95 MB, 117k | 106 MB, 119k |
| leak fix | 20.6, 11k | 50, 61k | 158, 142k | 63, 85k | 74, 87k | 84, 89k |
| leak fix + relayout | 20.6, 11k | 56, 58k | 165, 142k | 62, 82k | 74, 84k | 85, 86k |
| no-cv | 20.6, 11k | 44, 36k | 46, 35k | 53, 36k | 47, 37k | 56, 36k |
| **leak fix + no-cv** | 20.6, 11k | **32, 15k** | **29, 13k** | **37, 13k** | **26, 14k** | **36, 13k** |
| leak fix + no-cv + fast | 20.6, 11k | 21, 11k | 29, 13k | 27, 11k | 26, 14k | 36, 13k |
| plain (no CompMD in runs) | 12.6, 10k | 19, 10k | 19, 11k | 20, 11k | 21, 12k | 29, 13k |

The page itself has 3,700 to 5,300 elements throughout, in every variant.

**What a renderer costs.** Going from the Overview to the first Diffs screen adds 14.5 MB and 9,600 nodes for 26
renderers and their 320 rows: about 0.55 MB and 370 nodes each, rows included. CompMD's own share, from the plain
variant: **about 0.3 MB, 36 nodes and 8 listeners per renderer** (20.6 against 12.6 MB at open). The rest is our rows.
So what is needed on screen is about 21 MB, of which about 8 MB is CompMD's.

**What held the rest**, found by heap snapshots (544 markers, 512 of them on detached elements after two fast passes):

1. **A render after its renderer was destroyed (ours).** Every detached renderer was held the same way: a
   `MediaQueryList` `change` listener, its closure, CompMD's colour-scheme watcher (minified `_n`), its target `#t` (the
   `.compmd-page`), and the whole card's detached diff through it. CompMD's renderers remove that listener in
   `destroy()`, so these views were never destroyed. `render-turns.ts` takes the next turn off the queue, then waits
   up to 150 ms for the page to be idle before rendering. A card given back in that time destroys its `CompMd`, whose
   cancel then finds nothing in the queue, and the render runs anyway: `draw()` makes a view in the detached element,
   after `onDestroy` found none to destroy. The held elements show it: their outer `.compmd` says
   `data-compmd="loading"` (Svelte's last state), their `.compmd-page` says `data-compmd-settled` (CompMD drew it
   afterwards).
2. **Chromium's cached layout of skipped cards (`content-visibility`).** With the leak fix in, a snapshot after a skim
   still found 416 detached renderers. All were held by one path: the frame's view, its scrollable areas, a scroll
   container's `PaintLayerScrollableArea` and `LayoutResult`, then a tree of `PhysicalBoxFragment`s down to the layout
   objects of removed rows. A skipped card's layout isn't redone when its diff is given back, so its last fragments
   keep the removed rows alive. Laying the card out once more after the give-back (the relayout variant) did not
   release them; removing `content-visibility` did.

**Why both fixes are needed:** each holds a different set. The leak fix alone leaves the layout cache (158 MB after a
skim); no-cv alone leaves the leaked views (46–56 MB, 35,000 nodes); both together leave only what is on screen.

## Findings: speed

**Chromium** (medians; *blocked*: time over budget in frames over 50 ms):

| Variant | open | fast | reader (31 s) | fast2 | jumps | rendered |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 67 ms | 0 | 117 ms | 100 ms | 0 | 50 ms |
| leak fix + no-cv | 67 | 0 | 200 | 67 | 33 | 50 |
| leak fix + no-cv + fast | 67 | 50 | 150 | 67 | 0 | 50 |
| plain | 0 | 0 | 0 | 0 | 0 | 0 |

Chromium is smooth in every variant: a few frames of 80 to 150 ms, all of them CompMD highlighting a run (the long
animation frames name CompMD's `highlight.js`, 90% or more of their script time). Removing `content-visibility` costs
nothing measurable.

**WebKit**, the app's engine (medians; the range over three runs where it matters):

| Variant | open | fast | reader (31 s) | fast2 | jumps | rendered |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | **1,364 ms** (one frame of 1.3 s) | 49 | **10,713 ms** (141 long frames) | 307 | **2,490** | 1,779 |
| leak fix | 1,395 | 83 | 10,179 | 1,704 | 2,449 | 1,877 |
| leak fix + no-cv | 1,385 | 121 | 11,355 (8.9–12.5 s) | 1,520 | 2,583 | 1,649 |
| leak fix + no-cv + fast | 1,754 | 174 | 5,939 (see below) | 1,399 | 6,371 | 3,048 |
| plain (no CompMD in runs) | **35** | 0 | **249** | 0 | **49** | 219 |

- **The app's jank is CompMD's highlighting.** Without it, WebKit blocks 0.25 s in a skim instead of 10–11 s, and
  nothing on opening instead of 1.3 s. The 1.3 s frame on opening is CompMD's highlighter readying the TypeScript
  grammar, once a page, in WebKit only (Chromium: a few ms), as [review-performance.md](review-performance.md) found.
  During a skim, about 440 runs are highlighted at about 24 ms each on average, many of them in frames of 100 ms to
  1 s.
- **Our memory fixes don't change WebKit's speed** either way: the differences between baseline, leak fix and
  leak fix + no-cv are within the spread of the three runs (a skim: 8.9 to 12.5 s blocked with no-cv, 10.2 to 11.6 s
  without).
- **The fast-scroll hold, measured with this harness, misleads in WebKit.** The harness's skim moves a whole screen at
  each step, and the hold takes any move of half a screen or more for fast scrolling, so it held cards back while
  skimming too (108 renderers made instead of about 440: less to highlight, so less blocked). It also fires on a jump,
  which made jumps worse (2.1 to 6.8 s blocked). A reader's wheel moves in small steps, so the hold would need to tell
  speed from jumps before it can be judged.
- **Latency the reader sees** (Chromium / WebKit, baseline): the first diff line 80–140 ms; the first run highlighted
  150–280 / 200–250 ms; after a fast scroll stops, the runs in view highlighted about 0.2 s / 0.4–0.7 s later; a
  jump's file drawn in 5–17 ms and highlighted in 110–720 ms (Chromium). A run waits a median of 470 ms (Chromium) or
  1.6 s (WebKit) from mounted to highlighted when 26 are queued at once on opening.

**The rendered view:** the spec (394 KB) shown in 13 parts takes 740 ms to draw in Chromium and 1.3 to 1.4 s in WebKit,
with frames of up to 1.2 s in WebKit. In the plain variant (CompMD still renders the Markdown, but no code run competes
for turns) WebKit draws it in 1.09 s with no frame over 180 ms. Each part processes the whole document; how much of the
740 ms that is was not separated (see *Not measured yet*).

## Options compared

| # | Option | Memory | Speed | Complexity | Side effects and risks | Who |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **Fix the render after destroy:** a cancelled turn never runs, even once picked; a destroyed `CompMd` never draws | −17 to −21 MB at every phase's end alone; with 2, see 2 | none | about 15 lines in `render-turns.ts` and `CompMd.svelte`, unit tests | none: it is a correctness bug, and draws nothing a reader could see (its element is gone) | atm |
| 2 | **Drop `content-visibility: auto`** from the file cards | with 1: the session flat at 21–37 MB, 11–15k nodes (from up to 175 MB, 171k); alone: 44–56 MB | none measurable, either engine | 2 lines of CSS; the estimate's `min-height` for a loading card (ATM-615) stays useful, `contain-intrinsic-size` goes | the browser lays out the cards near the view rather than skipping those off screen: at most about 38 made cards on CR-1, whose rows are bounded by `FAR` and `CHUNK`. A review whose made cards are very tall (several 5,000-row files within 3,200 px) lays out more; the chunking caps a file's rows. The ATM-615 squeeze can't recur (it came from `content-visibility`) | atm |
| 3 | **A memory bound in the long-review spec:** DOM nodes alive after passes down and back, after a forced GC | keeps 1 and 2 | — | a test (CDP in Chromium) | none | atm |
| 4 | **Faster highlighting in WebKit** (CompMD): the TypeScript grammar's 1.3 s, and the per-run cost; for example off the main thread, grammars prepared ahead, the regex engine WebKit runs fastest, or tokenizing in slices that yield | none | the app: up to 97% of its jank (plain variant: 0.25 s against 10–11 s blocked in a skim) | CompMD's | — | CompMD |
| 5 | **A highlight cache** (CompMD): a run's highlighting kept by its text, language and theme, so a run seen again draws without tokenizing | a bounded cache (to size) | jumps and scrolling back: free for runs seen before; first views unchanged | CompMD's; atm's side unchanged | holding the cache; caching CompMD's output in atm instead would bypass its sandbox's lifecycle (the CompMD boundary) | CompMD |
| 6 | **The rendered view processes a document once a side:** `CompMd.svelte` keeps the processed tree of the last texts (by text, path and options) and gives each part its blocks | a processed tree kept while the card is made (a few MB for the spec) | the spec's 740 ms (Chromium) and 1.3 s (WebKit): the processing share is not measured yet, at most ×13 less | small: a cache in our own wrapper (`processText` is ours) | stale tree if the key is wrong (the text is the key) | atm |
| 7 | **Hold cards while scrolling fast** | none more after 1 + 2 | Chromium: 12 renderers made instead of 182 on a fast scroll, nothing visible; WebKit: not judged (above) | small, but it must tell speed from jumps and from programmatic scrolls (`showFile`) | cards appear up to 150 ms later after a fast scroll stops; jumps made worse as tried | atm |
| 8 | **Re-lay out a card after its diff is given back** | none (measured) | none | small | — | — |
| 9 | **One renderer per side per file** (CompMD drawing a text in ranges with gaps, close to CMD-250) | CompMD's share is about 0.3 MB a renderer, 7–12 MB for 22–38: perhaps half of that saved | fewer view setups; the tokenizing is the same text | high: `DiffView` is built per run (folds, comments between runs), and CompMD needs a new way to draw | large change for a small gain once 1 + 2 are in | CompMD and atm |
| 10 | **Reuse given-back renderers** (keep the last few cards' views) | more memory held on purpose | jumps back to recent files instant | medium: views kept detached, and re-attached | must not bring back the leak; overlaps with 5, which is the cleaner way | atm |

## Recommendations

1. **Build 1, 2 and 3 now**, together: the leak fix, `content-visibility` dropped, and the memory bound in the spec.
   They are small, measured, and take the tab from up to 175 MB to about 21–37 MB through any session, with no speed
   cost in either engine.
2. **Send 4 and 5 to CompMD** as tasks, with this document's WebKit numbers: they are what makes the app smooth, and
   only CompMD can do them. 4 first.
3. **Measure 6 when 1–3 are in**, and build it if the processing is a real share of the rendered view's time.
4. **Leave 7–10:** 8 gains nothing, 7 gains nothing a reader sees and needs more work, 9 and 10 cost much for what is
   left after 1 + 2 (and 5 covers 10's purpose better).

## Not measured yet, and the limits of these numbers

- **WebKit's memory.** Playwright's WebKit has no memory protocol, so the memory findings are Chromium's. The leak (1)
  is the page's own logic, so it holds in WebKit too; whether WebKit's layout keeps given-back diffs as Chromium's does
  (2) is not known. To check in the app: Safari's Web Inspector (Memory timeline) on the app's WKWebView, before and
  after 1 + 2.
- **The app itself.** Playwright's WebKit is the engine of the app's WKWebView, not the app: the app adds Tauri, its
  own window and the system's WebKit build.
- **One review, one machine.** CR-1 is large in files, not in one file: a review with a single 20,000-line file is not
  measured. All on this Mac, no CPU slowdown.
- **The skim is synthetic:** a whole screen every 250 ms, not a wheel's small steps (it misled the fast-scroll hold).
- **The rendered view's processing share** (6) and the TypeScript grammar's own cost were not separated by
  instrumentation; the estimates above say so where they are used.
- **Spread in WebKit** is large between runs (a skim: 8.9 to 12.5 s blocked); the comparisons above are of medians,
  with the spread given where it decides a conclusion.
