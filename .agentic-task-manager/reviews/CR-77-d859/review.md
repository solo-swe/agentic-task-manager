+++
id = "01a1f8a1-8965-77b5-bf02-2cc5675dd859"
number = 77
title = "Text size in Settings, the ⋯ on a review’s tab, and where Rendered’s look is set"
status = "approved"
tasks = ["ATM-691"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T11:04:58Z
updated = 2026-11-20T11:16:25Z
+++

The three things you asked for:

- **Text size** in Settings, under Document style: Tiny, Small, Medium (the default), Large, Huge (0.8× to 1.3×). It scales a Markdown document's text, headings with it, in the document viewer and on a review's rendered sides, in every document style and on the theme's own page. Line numbers follow the text. The setting is kept like the others, and the sample beside the select shows an "A" at the size chosen.
- **⋯ on a review's tab:** in the app, a review's tab now has the ⋯ at the title bar's right end, opening Settings as on every other tab.
- **Rendered's tooltip:** "Rendered, as the document viewer shows it. Its document style and text size are in Settings (⋯)".

**Also in this commit, test fixes only:** two Wrap tests read the line numbers once, right after a switch, before CompMD's layout had placed the rows. They failed about 1 run in 5. They now wait for the rows to settle: 32 of 32 passed in Chromium and WebKit.

**Tests:**
- unit: the sizes, an unknown one falling back to Medium, the setting kept;
- e2e: the size scales the viewer's text on the theme's page and in GitHub style, numbers beside it, kept after a reload; the rendered side's text scales; the ⋯ on a review's tab opens Settings; Rendered's tooltip.

The check, 1,822 unit tests and the whole functional e2e suite (566) pass.
