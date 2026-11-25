+++
id = "01a20b57-5449-7129-a6c6-36dc6d03d5dc"
number = 143
title = "The contents box sits in the document viewer’s corner"
status = "approved"
tasks = ["ATM-729"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T02:16:42Z
updated = 2026-11-24T02:37:05Z
+++

The document viewer's contents box now sits in the viewer's top-left corner.

**The cause:** the box sat in an 820 px column centered in the document's body, the page's old width. Since ATM-706, every document style's page is 980 px wide, GitHub's width, so the box ended up about 80 px inside the page, over its text.

**The fix** (`app.css`): the box is anchored to the body's corner, 6 px in, whatever the page's width. The body's padding is a variable (`--doc-body-pad`, smaller in a narrow viewer) that the box reaches out across. It stays sticky at `top: 0`, since a sticky box's `top` counts from inside its scroller's padding, so it stays put as the document scrolls.

**Please check:**
- The new e2e test in `documents.spec.ts`, which also runs in WebKit: the box 6 px from the corner in each style (GitHub, Docs, Magazine, the theme's own), left of where the text starts, after scrolling, and when open. The existing contents test checks the corner too.
- The look: the screenshots of the document viewer will show the box in its new place once the release task regenerates them.
