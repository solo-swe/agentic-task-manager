+++
id = "01a20b61-258e-738b-b9eb-cc2c240c50a3"
number = 144
title = "Diff view: a Final layout, the file after the change"
status = "approved"
tasks = ["ATM-731"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T02:27:25Z
updated = 2026-11-24T02:37:45Z
+++

A third layout for the diff view, **Final**: the file as it is after the change, nothing highlighted.

**What changed:**
- **`diff.ts`:** `finalBlocks` is the unified layout with every fold opened, the removed lines left out, and the added lines drawn as plain lines. Each row keeps its numbers (an added line has no old one). `DiffMode` and `isDiffMode` name the three layouts.
- **The selector:** the review page's Layout group has a third button, Final (the `file-text` icon), stored with the other layout settings. `DiffView`'s own header has Split, Unified and Final.
- **In final mode:** one column numbered on the new side only, no change marks, nothing folded, and Prev/Next change find nothing. In the rendered Markdown view, the new side shows whole (`RenderedSide`'s new `whole` prop), unmarked, labelled "After the change". A deleted file shows "Deleted by this change: nothing after it."
- **Comments:**
  - A chain on a new-side line stays under that line.
  - A chain on an unchanged old-side line stays under it too, since that line keeps its old number.
  - A chain on a line the change removed has no line in the final file, so it shows after the file, as chains on folded lines already do.

**Please check:**
- Whether chains on removed lines belong after the file in this mode. The alternative would be to place them at the line where the removal happened.
- The tests: `diff.test.ts`, `DiffView.component.test.ts`, and an e2e test in `reviews.spec.ts` (Final pressed, 41 unmarked lines, the chain on its line, kept across a reload).
