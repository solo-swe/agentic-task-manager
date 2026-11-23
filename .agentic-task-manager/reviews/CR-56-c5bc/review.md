+++
id = "01a1f2e1-1f52-733c-a955-f3f8ee25c5bc"
number = 56
title = "A task's CR badge says where its review stands"
status = "approved"
tasks = ["ATM-621"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T08:16:42Z
updated = 2026-11-19T08:59:02Z
+++

A task's CR badge now says where its review stands, on the task's card and in its details, so you can see at a glance whether it's approved.

## Change
- **The status comes with the task, from atm** (`atm-lib` `query::task_views`). Each task view's `derived.review_statuses` maps each of the task's code reviews (by the review's ID, as `code_reviews` links it) to its status: `draft`, `waiting`, `live`, `changes_requested`, `approved` or `closed`, or `unreadable` for one atm can't read (ATM-617). A review this working copy doesn't have is left out.
  - The reviews are read once for all the views, never per task.
  - The tasks' `ETag` already moves with the reviews' files (ATM-606), so a decision shows on the next refresh.
  - Schema (`api.schema.json`) and API doc updated.
- **`ReviewLink.svelte`:** shows the status on every review link, prominent or not.
  - Colours: approved green, changes requested red, waiting and live the accent, closed and draft muted, as the task asked. The Reviews list's own chips show *waiting* muted, so the two differ there; say if you'd rather they match.
  - What the page learned since the view came (the Reviews tab, `known.svelte.ts`) is fresher and wins.
  - Without a status, an older atm's view or a word it doesn't know, the badge is as before.
- **Callers:** the Kanban card, the list's task card, and the details' "In review" and "Code reviews" links pass the task's status.

## Tests
- **CLI** (`reviews/lifecycle.test.mjs`): a task's view says `waiting`, then `approved` after the decision; a task without reviews has no field; a review made unreadable is `unreadable`.
- **Unit** (`ReviewLink.component.test.ts`): each status's label and `data-status`, prominent or not; no status or an unknown word, the badge as before; `unreadable`; a status the page learned since wins.
- **e2e** (`kanban.spec.ts`): a card in review shows *Waiting for review*, then *Approved* after the next refresh, and the details say it too.
  - The fixture's views come from atm run outside git with no reviews folder, so its `reviewStatuses` option sets what atm would derive. atm's own reading is the CLI test above.
- **Suites:** the whole CLI suite (831), the unit tests (1,782), the checks, clippy, and the full functional e2e. One failure there, ATM-641's long-review timing under load (104 against 100, the WebKit real-review test blocking 3.7 s beside it), unrelated and already on ATM-641.

The screenshots of a card in review get regenerated in the release task.
