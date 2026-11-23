+++
id = "01a1eda4-b0cb-7039-ab4b-4cf4d8fc381e"
number = 16
title = "Wrap long lines in the diff viewer"
status = "approved"
tasks = ["ATM-588"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:52:35Z
updated = 2026-11-18T08:16:18Z
+++

The Diffs tab gets a **Wrap** button (the wrap-text icon, in its own group after Split/Unified). It's kept in this browser with the layout.

**In Unified:** long lines wrap at the column's width. A line's row is as tall as its wrapped text, its number stays on its first visual line, and the next line's row starts right under it. Nothing scrolls sideways. The `+`, the pick, a chain's highlight and the comment box all go by the measured rows.

**In Split it waits, and says why** (the button is disabled with its reason on hover). Keeping the two sides level means making a wrapped line's counterpart as tall. CompMD draws a code block as token spans between newlines, with no element per line, so there's nothing to give that height to. And `lines()` gives a code block's lines no boxes; I found that while measuring (`drawn: []`). So:
- **CMD-250** in CompMD's backlog: each code line an element, and its boxes in `lines()`. Its `tasks.json` change is left uncommitted there for you, as with CMD-249.
- **ATM-616** in this backlog: Wrap in Split once CMD-250 is in.

**How a wrapped line is measured** (`DiffCode.svelte` `measureWrapped`): read-only, from a collapsed DOM range at each line's start in the code CompMD drew. Each row runs from its line's top to the next line's top. Empty lines at the end get one line height each. Where nothing is laid out (jsdom) it measures nothing. Unwrapped, nothing changes: the one line height, as before, so ATM-587's long-review numbers hold (measured this run: first drawn 64 ms, slowest step 32 ms).

Tests:
- Component: Wrap disabled in Split, with its reason; it wraps in Unified, isn't applied back in Split, and is kept for the next page.
- E2e: a 400-character line wraps in Unified, its row is more than twice another's, the next row starts where it ends, with no sideways scroll, and it's kept after a reload. In Split the button is disabled, saying why.
