+++
id = "01a1f2bc-f2ed-75ca-93e2-b0056d742584"
number = 54
title = "Rendered Markdown in a review is the Document Viewer's own page"
status = "approved"
tasks = ["ATM-645"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T07:37:11Z
updated = 2026-11-19T07:44:38Z
+++

The rendered README on CompMD's CR-7: one page only, the last line outside it, narrower than the viewer's. And it's now the Document Viewer's own component, not a copy.

## What was wrong
`reviews/RenderedSide.svelte` re-created the viewer's page from its CSS classes:
- its own placement code and gutter;
- its own 52 px left margin;
- its own height rule: `aspect-ratio: 8.5 / 11` with `min-height: auto`, meaning "at least a letter page, taller with its content".

That rule is my lead suspect for your app: if the webview doesn't let the content grow the box (WebKit's handling of `aspect-ratio` with an automatic minimum height), the page stays one letter page tall and the rest of the text runs past it. That's exactly what you saw. I couldn't reproduce it in Playwright's WebKit, so this is the likely cause, not a proven one.

## Change
- **`documents/DocumentPage.svelte` (new):** the page, taken out of the viewer. It holds:
  - the paper (`.doc-page`, the theme's and the document style's);
  - the text as CompMD renders it, whole or in parts (a review's folds, `parts` and `onunfold`);
  - a Markdown document's line numbers in its margin, placed beside the text CompMD drew (`gutterRows`, one algorithm for both).

  The host draws its own marks over it (`overlay`), says which margin rows to mark (`marked`), and hears where lines were drawn (`onplaced`).
- **`DocumentViewer.svelte`:** uses it. Its own marks stay its own: a code file's line numbers, the lines a link names, the marks on a link's text. The contents list, scrolling and text fragments are unchanged.
- **`RenderedSide.svelte`:** uses it. It keeps only the review's parts: the desk and its label, the folds, the change marks, the selection. Its own margin and width rules are gone, so the page is the viewer's width (820 px where there's room) with the viewer's margin.
- **Height, one rule:** the page is as tall as its document. In the viewer it still fills its window when the document is short; in a review card, which has no window, it's the document's height.

  **A visible change:** a short document's page in a review is no longer a letter page tall (ATM-592's minimum); it ends with its text. Say if you want a minimum back, done in a way that doesn't rely on `aspect-ratio`.

The folds' class is now `doc-fold` (it was `rendered-fold`), as the page's own.

## Tests
- **`reviews.spec.ts`:**
  - the page's last text inside the page;
  - a document of several pages drawn whole, its last line inside and numbered;
  - in Unified, the page as wide as the viewer's (`max-width` 820 px, and `min(820, the desk)`).
  - The fold tests use the new class.
- **The Document Viewer's specs** (lines, Markdown numbering, `#L`, text fragments, contents, tables, themes) pass unchanged, as do the review specs (98 in all).
- **In WebKit** (Playwright, by hand: the suite is Chromium only), the rendered and viewer specs pass, 41 of 44. The 3 that don't are the fixture's: WebKit can't grant `clipboard-write` or answer a 304 in a test.
- **Unit tests** (1,778) and the checks pass.

To check in the app: CR-7's README, Rendered, Split and Unified, scrolled to its end. The screenshots that show a rendered review page are regenerated in the release task.
