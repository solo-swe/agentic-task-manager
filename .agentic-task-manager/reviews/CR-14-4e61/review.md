+++
id = "01a1ed86-c83e-75b2-9f6b-cd6738b24e61"
number = 14
title = "A task links to its code reviews in its details"
status = "approved"
tasks = ["ATM-585"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:19:55Z
updated = 2026-11-18T08:11:37Z
+++

A task's details now link to its code reviews.

- **In review:** the *In review* section shows the latest review the task names, prominently (**CR-7 · Waiting for review**), with earlier ones as smaller links after it.
- **Not in review** (done, sent back, …): a quieter *Code reviews* section lists them, oldest first.
- **A click** opens the review's page with nothing else over it. In the browser it opens in this tab, or another for ⌘-click, Ctrl-click or a middle click. In the app it opens in a tab of its own, as the Reviews list does.
- **The status** shows when the page knows it: the Reviews tab records what it lists and shows (`reviews/known.svelte.ts`), and nothing is asked per task. Until then it's the reference alone. *needs you* comes from the project's summary (ATM-606).
- **Shared:** `details/ReviewLink.svelte` takes a project and a reference, for ATM-575's cards and list (now named in that task's files). `address.ts` gets `reviewPageOf`.

Tests:
- Component: one review, two (latest first and prominent), none, and not in review. The known status appears once learned, and *needs you* from the summary.
- E2e (`detail.spec.ts`): ⌘-click opens a browser tab on the review's page and leaves the details open; a click opens it here, with the details closed.
