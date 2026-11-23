+++
id = "01a204f8-7547-74b9-85a7-a575e641346a"
number = 119
title = "Document styles: GitHub’s width for every page, GitHub first and the default, Theme last"
status = "approved"
tasks = ["ATM-706"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T20:35:21Z
updated = 2026-11-23T00:39:57Z
+++

Document styles: every page GitHub's width; GitHub the default and first, Theme last (Hakan, 2026-11-20).

## What changed
- **`themes/doc-styles.css`:** one `--doc-page-width` (980px), every style's page `max-width` from it: Docs (was about 816px) and Magazine (760px) are as wide as GitHub's, their type and colours as they were. The diff viewer's rendered side (`.doc-sheet`) follows.
- **`appearance.ts`:** `DOC_STYLES` GitHub, Docs, Magazine, then the theme's page, now named "Theme (the theme’s page)"; `DEFAULT_DOC_STYLE` GitHub. **`state.svelte.ts`** reads the stored choice with that default. A preference is stored only when chosen (`prefs.ts`), so a reader who chose a style, the theme's page too, keeps it; one who never chose gets GitHub. No migration needed.

## Tests
`appearance.test.ts` (order, default, a chosen `auto` kept), `state.test.ts` (none chosen is GitHub; `auto` once chosen stays), `documents.spec.ts` (a fresh dashboard renders in GitHub; every style's page 980px). Three alignment checks measured from a heading's box, which GitHub's padding and rule moved about 4px below its text: now measured from the text, they show the numbers on their lines under GitHub too (no layout change). The Hand-drawn notebook spec chooses the theme's page; a review's rendered side is 980px.

## To look at
- Screenshots that change (Settings, the document viewer, the review's rendered side) are regenerated in the release task.
- The name "Theme (the theme’s page)": the task called it Theme; say if you want another label.
