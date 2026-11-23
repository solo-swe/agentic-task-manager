+++
id = "01a1f86c-76ca-7359-989a-45682d3ed2ff"
number = 76
title = "A comment on rendered Markdown shows on the page, under its block"
status = "approved"
tasks = ["ATM-651"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T10:07:00Z
updated = 2026-11-20T11:58:03Z
+++

A comment on rendered Markdown now shows on the page, under the block it is on, as the source view shows a chain under its lines.

- **Where:** the page is cut after the block where each chain ends, and the chain shows in the gap, in Split and Unified. Open chains are shown in full. Resolved ones are folded, with Reopen and Show, as in the source view. Chains on the same block are listed latest-starting first.
- **Writing:** the comment box opens in the same place, under its block, instead of over the page. Once the comment is sent, it shows there.
- **Line numbers and change marks** below a chain move down with the text. The page places them again whenever it changes size, because CompMD doesn't see a part move when nothing changed inside it.
- **Focus:** a comment box on the rendered page is focused without scrolling, then scrolled into view once the page is drawn. Before, it scrolled while the page above it still held the whole document, and the box ended up under the file's header.

**Tests:** a new e2e test, in Chromium and WebKit: chains under their paragraphs (open and resolved), Split and Unified, a new comment written and shown there, and the numbers and marks below beside their text. `renderedParts` has a unit test for the cuts. The existing rendered and comment tests pass.
