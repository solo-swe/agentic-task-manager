+++
id = "01a1ed8b-e034-76ab-8b2b-164569f95f17"
number = 15
title = "Links in a review open the project documents"
status = "approved"
tasks = ["ATM-586"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:25:29Z
updated = 2026-11-18T08:14:28Z
+++

Links in a review's summary and comments to the project's documents now open, as they do in a task's text.

- **The decision the task left open:** a link to a file the round changed opens its diff on the Diffs tab, since the review is about that file. Anything else opens the document viewer over the review, at its heading, lines or text: another file, a document, or a changed Markdown file's heading or text fragment. A bare link to a changed file means its diff; a heading or text link means reading the document.
- **How it reaches every comment:** the review page decides (`openLink`) and hands it down through Svelte context (`reviews/links.ts`). The summary and every `ChainBox` get it, on the Overview, on the Comments tab and in place in a diff, without threading a prop through the diff viewer. `ReviewsTab` passes `ondocument` → `dashboard.openDocument`.
- **One rule in CompMd:** it made a relative link a viewer link only for task text or a document (`task` or `path`). A text given `onopen` is now the project's text too. Help and What's new pass no `onopen`, so their relative links stay text, as before.
- External links are unchanged: the browser, or `open_url` in the app.

Tests:
- CompMd: given `onopen`, relative links (a heading, a line range) call it with their reference; without it, they stay text.
- E2e (`reviews.spec.ts`): the summary's link to a document opens the viewer, as does a comment's. The summary's link to a changed file opens its diff on the Diffs tab, with no viewer.
