+++
id = "01a1f86c-74ad-71b0-ad10-bfa792277388"
number = 75
title = "A review’s source view draws no line between two lines"
status = "approved"
tasks = ["ATM-690"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T10:06:59Z
updated = 2026-11-20T11:45:54Z
+++

You saw a line between every two lines of a review's source view. Each line's row (its tint and its number) sat at the code's measured line height, a fraction of a pixel (20.8 px). Where two rows met, both half covered the same pixel, and their tints blended into a line.

`rowOf` in `DiffCode.svelte` now puts each row's edges on whole pixels, so the rows tile with no seam. The text doesn't move, and every line number is still level with its text, in Chromium and WebKit, wrapped or not.

**Test:** the Split Wrap test checks that every row is on whole pixels and starts where the one before it ends, wrapped and not (the test is in ATM-651's commit, which shares `reviews.spec.ts`).
