+++
id = "01a1ed3d-c933-709b-b21a-458563326281"
number = 7
title = "The reviews list keeps its last filter"
status = "approved"
tasks = ["ATM-604"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T06:00:11Z
updated = 2026-11-18T06:06:13Z
+++

The reviews list now opens with the filter you last chose: press *Open* once, and it's still pressed the next time you come back. It's kept in this browser, per project.

- Kept: the statuses, author, reviewer, task and branch. The search words aren't kept; they're for the moment.
- *Clear* forgets the filter.
- Going to another project opens that project's own filter.

Where: `dashboard/src/lib/review-filter.ts` (stored under `atm-review-filter`, one entry per project; anything stored that isn't a filter is ignored), and `ReviewsTab.svelte`.

Tests: unit tests in `review-filter.test.ts`, and an e2e test in `reviews.spec.ts`. The e2e test chooses *Open* plus a search, reloads, and checks that *Open* is still pressed with the search empty; then it clears and reloads, and the list is unfiltered.
