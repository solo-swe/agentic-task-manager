+++
id = "01a1f14b-b863-7079-8830-12a2b5dab6cb"
number = 41
title = "A card in review links to its code review"
status = "approved"
tasks = ["ATM-619"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:53:53Z
updated = 2026-11-19T05:26:01Z
+++

A task card in review shows its code review: open it straight from the board or the list.

## Change
- **On a card in review** (the Kanban card and the List's card): its latest review, as the details panel shows it since ATM-585. It's the same `ReviewLink`: the reference and its status when the page knows it, *needs you* when the project's summary says so. Its `href` is the review's page.
- **Opening it:** a click opens the review, never the task's details. ⌘-click, Ctrl-click or a middle click opens a tab of its own. In the app, a tab of the app.
- **Two fixes found on the way:**
  - the Kanban card took Enter for itself, even on a link inside it; now Enter on the link is the link's;
  - `ReviewLink` let a ⌘-click through to the card, which opened the task as well.
- **Only tasks in review with a review show it.** A task in review without a CR shows nothing new, and neither does a done task (its reviews are in its details).

## Tests
- **Component (TaskCard):** the latest review as a link to its page; a click navigates to it and doesn't open the task; none without a review or out of review.
- **e2e (kanban):** the badge on a card in review; ⌘-click opens CR-2 in a new tab with the details closed; Enter on the link opens the review.
- **Suites:** the kanban, detail and reviews-overview specs (54), the unit tests and the checks pass.
