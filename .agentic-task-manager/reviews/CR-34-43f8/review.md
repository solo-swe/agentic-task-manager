+++
id = "01a1f0fd-53ca-77ab-b6c9-8c24316943f8"
number = 34
title = "The reviews list in groups: waiting for you first"
status = "approved"
tasks = ["ATM-610"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T23:28:16Z
updated = 2026-11-19T06:02:30Z
+++

With no filter or search, the reviews list is grouped. Each group sits under its heading with its count, and each review is in the first group it fits.

| Group | What's in it | Order |
|---|---|---|
| **Waiting for your review** | waiting or live, needing you | the longest waiting on top, each saying "Waited 3 days" (from its latest round) |
| **With open comments** | the rest with open comments | the latest activity first |
| **Sent back** | changes requested | the latest first |
| **Waiting on others** | not decided, none of the above | the latest first |
| **Recently decided** | approved or closed in the last 5 days (`RECENT_DAYS`) | the latest first |
| **Older** | decided before that | folded until opened; this browser remembers |

Any filter or search shows the plain list, as before.

## Two choices to check
- **Waiting on others** isn't in your list of groups. Without it, a draft, or a review waiting on someone else with no open comments (an agent's own waiting review, say), would disappear from the unfiltered list.
- **Next review needing you** now opens the first one *shown*, the top of *Waiting for your review*, not the first in the server's order.
- **The "Sent back" heading:** you wrote "Sent back by you"; I used "Sent back", because the group is every review with changes requested, including one sent back by someone else.

## Change
- **atm-lib:** each list entry has `waiting_since`, its latest round's time (documented in `docs/multi-project-api.md`).
- **Dashboard:** `groupReviews` and `waitedFor` in `page.ts`; the headings, ages and fold in `ReviewList.svelte`.

## Tests
- **Rust:** `waiting_since` is the latest round's time, and moves with a new round.
- **Unit:** each group; a review fitting two going to the first; the order in each; the 5-day edge; empty groups left out; a missing time; the age's wording.
- **Component:** the headings and counts, *Waited 3 days*, *Older* folded, opened and remembered, a filter's plain list, and *Next* following the order shown.
- **E2e:** the grouped list, and a filter listing them plainly.
- **Suites:** the full functional suite passed except one timing test, which I'm not counting as passed. The long-review Diffs test measured 115 ms against its 100 ms budget, under the full suite's load; an hour earlier the review specs alone measured 31 ms. This change doesn't touch the Diffs tab. I noted it on ATM-629, the task on budgets under load, rather than raising the budget or rerunning.
