+++
id = "01a1f10f-d532-742d-8375-829a6887a16b"
number = 36
title = "Every project's reviews on the overview; reviews in the needs-you chip"
status = "approved"
tasks = ["ATM-575"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T23:48:29Z
updated = 2026-11-19T05:37:50Z
+++

Built as you decided: reviews on a fourth overview tab, the project's own Reviews tab kept, and reviews counted in the existing needs-you chip.

## Change
- **The overview's Reviews tab** (`/reviews`, after Agents):
  - every project's reviews in the reviews list you know (search, filters, groups), each row naming its project;
  - a **project picker** (All projects, or one);
  - a row opens its review in its own project (in the app in a tab of its own);
  - a project whose reviews can't be read is named above the list, and the others still show;
  - the filter is kept, as a project's is.
- **The project's Reviews tab:** unchanged. It's that list for one project.
- **The needs-you chip:** counts the reviews waiting on you, from the project summary's `needs_you_reviews`, and lists them after the tasks with why ("waits for your review", "answered since you wrote"). This applies on a project's title bar and on the overview's cards. A card whose only waiting item is one review now opens that review; before, it opened the Kanban board.
- **`/reviews` is a page:** in `atm serve` (`pages.rs`), the route matcher, the e2e server, and `docs/multi-project-api.md`.

## Not done here
- The original task's **sidebar Reviews section**: you chose the overview tab instead.
- **CR links on task cards:** ATM-619's job.
- **Deep links:** already work.

## Tests
- **Unit:**
  - `mergeProjectReviews`;
  - `NeedsYouMenu` with reviews: counted, listed after the tasks, a row opening its review, one review alone, none without a way to open;
  - `ReviewList`: a row's project and the entry passed to its link and open handlers;
  - `pages.rs`: `/reviews`, and `/reviews/CR-7` refused.
- **E2e:**
  - the tab across two projects (the picker, a row's link into its project, an unreadable project named);
  - the chip opening a review;
  - the overview's arrow keys across five tabs.
- **Suites:** the full functional suite passed **except three timing tests under its load**:
  - switching tabs: Graph 217 ms against 200;
  - the large review: 221 ms (**69 ms alone**);
  - the long review's step after settling: 118 ms (**34 ms alone**).

  None of them touch this change. They're on ATM-629 (budgets under load), not raised.
