+++
id = "01a1f146-5a7e-7004-bccb-464c471bcdb7"
number = 39
title = "Table columns as wide as their words, never a letter a line"
status = "approved"
tasks = ["ATM-618"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:48:02Z
updated = 2026-11-19T06:08:49Z
+++

Table columns in the document viewer (and task text) are no longer squeezed to a letter a line while one column takes the space.

## Cause
`.compmd` sets `overflow-wrap: anywhere`, so a word may break between any two letters. That made a cell's least width one letter. The browser's table layout then gave a column with long text all the room and shrank the others to that.

## Change
- `.compmd :is(th, td) { overflow-wrap: break-word }`: each column is at least as wide as its longest word, and the rest of the width is shared by how much text each column holds.
- A table wider than the text still scrolls sideways inside it, as before (`display: block; overflow-x: auto`).
- It applies to every document style (GitHub, Docs, Magazine) and to task text, which share `.compmd`.
- **One choice to check:** a very long unbreakable value (a URL, a hash) now widens its column, and the table scrolls, rather than breaking mid-value. Say if you'd rather such values break.

## Tests
- e2e (`documents.spec.ts`, "the document viewer: tables"): a three-column table.
  - At the viewer's width, the ID and word columns stay on one line each, and the sentence column wraps at its words.
  - In a 360 px window, the words stay whole and the table scrolls sideways; the viewer stays inside the window.
- Without the fix, both tests fail: the words broke over 2 to 5 lines.
- The documents specs (37) and the checks pass.
