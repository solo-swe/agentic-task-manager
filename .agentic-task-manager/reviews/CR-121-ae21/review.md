+++
id = "01a2050b-0c12-75b1-be03-6dea9182ae21"
number = 121
title = "State machine, part 4: the list’s decided from atm, OPEN_STATUSES gone"
status = "approved"
tasks = ["ATM-714"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T20:55:39Z
updated = 2026-11-23T00:40:51Z
+++

State machine, part 4: the dashboard's copies of the review rules removed.

## What changed
- The review page showing its buttons from `can` only, its `approveBlocked` and `sentBack` fallbacks gone and the e2e stand-in sending `can`: done in ATM-723 (CR-112, approved).
- **The reviews list:** atm's list entries carry `ended` (`Status::ended`: approved or closed); the list's groups (`page.ts` `groupReviews`) read it instead of their own approved-or-closed check.
- **`OPEN_STATUSES`** in `reviews/types.ts`, the dashboard's copy of `Status::OPEN`, removed: nothing used it. `REWORK_STATES` was already gone.
- The e2e fixture's reviews lists get `ended` as atm gives it; `docs/multi-project-api.md` names the field.

## Tests
`listing.rs` (a waiting review isn't ended; closed, it is); `page.test.ts` (atm's `ended` decides the group, not the status); the list fakes give it. Units 1899, CLI suite and e2e 594 green.

## To look at
- The page still reads `status` for how a review looks (its label and colour) and which group it waits in (waiting, live, changes requested): those are statuses shown, not a rule of what may be done.
