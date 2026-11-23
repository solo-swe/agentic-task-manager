+++
id = "01a1f2b1-8743-73e8-a41a-056baf0d1672"
number = 53
title = "One CompMD runtime per document; a long review restyles only for what it shows"
status = "approved"
tasks = ["ATM-646"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T07:24:43Z
updated = 2026-11-19T07:54:46Z
+++

The atm side of the restyle problem from your CR-7 recording. CompMD's side (one shared sheet, no forced style reads, per-document listeners) is the proposal on your clipboard. This commit does what atm can do without CompMD, and puts the one place CompMD's fix will plug into in position.

## Change
1. **One CompMD runtime per document** (`lib/compmd-runtime.ts`, new). Every `<CompMd>` used to make its own sandbox (`createSandbox`) and its own `MutationObserver` on `<html>` for the theme. Now there's one of each for the document:
   - `sandboxOf(compmd)`: the shared sandbox;
   - `followScheme(view)`: one observer telling every view, gone with the last.

   When CompMD takes a shared `stylesheet` / `scheme`, they're passed here, one change.
2. **A jump doesn't close CompMD pages while it's being made** (`ReviewFile.svelte`). A card a jump scrolled far from gives back its diff only once the page has settled, and keeps it if it came near again meanwhile. Each close is a full-page restyle in WKWebView. Outside a jump a card gives back at once, as before, so ATM-620's memory bound holds: 21,347 nodes alive after down and back, every run.
3. **A side with nothing to highlight opens no CompMD page** (`DiffCode.svelte`). That's a run of only padding rows and empty lines, the old side of added lines. It keeps the plain text a run shows while it waits for its turn, which is the same height, so it never opens and closes a CompMD page.
4. **A card not made yet is contained** (`.review-file-later { contain: strict }`): an empty box at its estimate, so restyles and repaints of the rest of the review skip it. A long review is mostly these. It's empty, so this isn't ATM-620's `content-visibility` leak.

## Measured (Chromium; WebKit counts the same triggers)
A new e2e uses a review shaped as CR-7: 15 files, one long, ~100,000 px of unmade cards. During a jump to the last file:

| | before ATM-644 | after ATM-644 | now |
|---|---|---|---|
| CompMD pages opened | 8 | 8 | 7 |
| `adoptedStyleSheets` set | 12 | 12 | 10 |
| `getComputedStyle` | 96 | 32 | 28 |

The 28 reads are exactly CompMD 0.7.1's own, four per page it opens and draws, so the review's code now forces none. The test asserts these bounds:
- pages ≤ 8, the cards in view and near it;
- sheet sets ≤ 2 per page;
- reads ≤ 4 per page.

The pointer sweep still forces 0 reads.

## What's left, and why
The 7 pages are what's on screen: a page per side of each run in view. Below that only CompMD helps. Until its fix, each of those opens still restyles the whole page once, which in your app is ~1 s on a review this long. That's much better than before (no hover restyles, no closes during the jump), but not instant.

## Tests
- **Unit:** `compmd-runtime.test.ts` (one sandbox, a new module makes its own; one observer tells every view, ignores other attributes, and goes with the last).
- **e2e:** the measured jump above. The review, document and details specs pass (98), as do the unit tests (1,775) and the checks.
- **The full functional run** (525) passes except ATM-641's long-review timing under load (107 against 100, with the WebKit real-review test blocking 3.4 s beside it). Alone it measures 26–61 ms. I made the new test smaller (5,000 lines, not 20,000) so it competes less: with it, the app tab-switch test that had also failed under load passes.
