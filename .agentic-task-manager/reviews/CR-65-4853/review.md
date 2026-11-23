+++
id = "01a1f5ad-db67-7610-962f-33fbf7a14853"
number = 65
title = "WebKit: the + on a wrapped line keeps the pane where it is; tests in WebKit"
status = "approved"
tasks = ["ATM-655"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T21:19:34Z
updated = 2026-11-20T09:49:03Z
+++

In WebKit (the app's engine), the **+** on a wrapped line no longer sends the review pane about 1,600 px away. The tests whose layout or scrolling can differ there now run in WebKit too.

## The cause, traced
- **What it wasn't:** the comment box takes the focus without scrolling the pane (traced: the pane was still at 1,231 after the click), and nothing in atm scrolls it.
- **What it was:** three frames later the pane jumped by exactly the height of the code block above the box (1,628 px), with no layout change. That was the moment the block's plain text was swapped for its highlighted text, at its turn.
- **Why only WebKit:** its scroll anchoring had picked a node inside that block, and when the node was replaced it moved the pane by the block's height. Chromium doesn't.

## Change
- **The fix (`DiffCode.svelte`):** `overflow-anchor: none` on a code block, so the pane never anchors inside one. With it, the pane stays at 1,231 in every frame.
- **A `webkit` project (`playwright.config.ts`):** reruns the tests tagged `@webkit` (`WEBKIT_TOO` in `reviews.spec.ts`) in WebKit, after `svelte`. `timed` now waits for `webkit` too, so no WebKit page runs beside a timed test. Tagged:
  - the two "+ on a line … the pane stays" tests;
  - the Wrap test;
  - the two line-number tests (ATM-652).

  They take 13 s. The whole reviews spec in WebKit passes too (2 min), but it doesn't need to run there every time.
- **`AGENTS.md`:** how to tag a test for WebKit.
- **A flaky test:** "edit your own reply in place" timed out once in WebKit, in the whole spec on five workers; its Edit press was lost while the page settled. It now presses until the box opens.
- **A new test:** a refresh of the review while you edit your reply keeps the edit. I checked this first, in case a refresh dropped the editor; it doesn't.

## Checks
- `pnpm run check` and the unit tests (1,785).
- **The full e2e:** one test outside this change failed once, Project tab's "Information and Stats end at the same height". It passes three times alone. The `webkit` and `timed` projects then passed (14).
- The fixed test fails in WebKit without the fix, the pane moving 1,231 → 2,859; with it, it passes.
