+++
id = "01a1f50b-4e82-7076-bd22-7d813b70a7d4"
number = 61
title = "Rendered Markdown: comment with the + on a line, or a drag over lines"
status = "approved"
tasks = ["ATM-650"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T18:22:01Z
updated = 2026-11-20T09:48:18Z
+++

The rendered view of a Markdown file now takes comments the way the source view does.

## Change
- **The +:** hovering a block shows a **+** beside its line number. A click opens the comment box right under the block. A press dragged over more blocks picks all their lines; Shift extends a pick. The block's number can be pressed as well. While the comment is written, the lines picked keep their + and their text is tinted in the accent colour.
- **What the comment is anchored to:** the source lines the block came from (the margin's rows, from CompMD's line map). The quote is those lines as written, as in the source view, so the comment also shows in the source view, on the same lines. Selecting text still works as before.
- **One button, not a copy:** the + is a new `ui/LinePlus.svelte`, used by both the source view (`DiffCode`) and the page (`DocumentPage`). The page takes the + only when its host passes `onpress`. The review does; the document viewer doesn't, so it is unchanged.
- **Where the comment box goes:** under the lines picked. `DiffView` now gets that position from the side, instead of reading the text selection.
- **Room for the + in a review:** a review's page leaves 64 px on the left, between the numbers and the text. Its numbers there are now at full strength, and turn the accent colour when hovered or picked.

## Tests
Two e2e tests on a rendered file:
- **Click:** no + until the pointer is over a block, then only that block's, between its number and its text. None on a blank line. A click opens the box on line 1, under the heading, and the + stays. Pressing a number picks its line.
- **Drag:** the + dragged over two blocks gives lines 3–5. The comment is sent with those lines and their source text as the quote, and it shows in the source view under line 5.

## Checks
- `pnpm run check` and the unit tests (1,785);
- the functional e2e (526) once.

Not in this change: showing comments on the rendered page itself (ATM-651), and the line numbers' alignment (ATM-652).
