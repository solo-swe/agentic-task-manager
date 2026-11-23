+++
id = "01a1f84f-8962-76ad-b48b-9c6c85ce705b"
number = 72
title = "Wrap in Split, its two sides kept level"
status = "approved"
tasks = ["ATM-616"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T09:35:24Z
updated = 2026-11-20T09:51:57Z
+++

Wrap works in Split too. A long line wrapped on one side makes its counterpart on the other side as tall, so the lines after it stay level across the two sides. Each line's number stays on its first visual line.

This uses CompMD 0.8.0's line API, which ATM-616 was waiting for: `view.lines()` gives each line's boxes, and `view.lineHeights()` sizes its lines.

## How
- **Measuring, `DiffCode.svelte`:** a wrapped run measures its lines from CompMD's boxes of each line (`lines()`, one box per piece of text on each visual line). A line's own height is its visual lines times the line height, read from the text's boxes, so the heights given to it never feed back into it. The run reports those heights, and its gutter rows follow each line's top. This replaces DiffCode's own measuring with a range at each line's start.
- **Levelling, `DiffView.svelte`:** in Split, both sides of a run get each line's taller height. `DiffCode` gives the lines taller than one line to CompMD as `lineHeights`, by their line in the text: the frontmatter's lines, then the fence's, then the code.
- **Plumbing, `CompMd.svelte` and `compmd.ts`:**
  - A `lineHeights` prop, given to the view as it changes. A text no one sizes lines of never calls it, and heights no longer asked for are cleared once.
  - A text whose lines are read (`onlines`) is now processed with origins. Without them `lines()` gave a code run nothing; documents already had them.
  - Reading lines is part of the render key, so turning Wrap on processes a run again with them.
- **The button, `ReviewPage.svelte`:** the Wrap button no longer waits in Split.

## Tests
- **E2e (`reviews.spec.ts`, `@webkit`), new.** In Split, a changed line is long on the new side and short on the old. Wrapped:
  - the lines after it (their text, found where it is drawn) are level across the sides within 2 px;
  - the changed line's rows are several lines tall and equal on both sides;
  - each side's next row starts where the changed line's ends;
  - unwrapped, one line height again.

  Passes in Chromium and WebKit. With the heights not passed, it fails (the sides 134 px apart). A screenshot shows the short line as tall as the wrapped one, lines 3 and 4 level.
- **E2e, existing:** the Unified Wrap test, ATM-652's line-number test (wrapped and not), and ATM-650's `+` on a wrapped line pass on the new measuring, in both engines.
- **Unit:** `CompMd.component.test.ts` checks that heights reach the view as they change, are cleared once, and aren't sent when there are none. `ReviewPage.component.test.ts` checks that Wrap is enabled and wraps in Split.
- **Checks:** `pnpm run check`, the unit tests (1,816), the functional e2e (535), and the WebKit project (16).
