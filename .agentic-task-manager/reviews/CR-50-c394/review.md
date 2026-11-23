+++
id = "01a1f267-543e-74c0-a057-1de50508c394"
number = 50
title = "A poke never keeps a review from an approval"
status = "approved"
tasks = ["ATM-643"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T06:03:40Z
updated = 2026-11-19T06:10:21Z
+++

The bug: a poke shows as a comment with no Resolve button, which is right, but Approve was then disabled with "1 comment is still open". This fixes it.

## Cause
The dashboard decided "open" itself, differently from atm:
- **Dashboard:** `isResolved` in `reviews/page.ts` called a chain resolved only when every top-level comment's `resolved` was `true`. A poke has no box (`resolved: null`), so its chain was never resolved and counted as open.
- **atm:** `open_comments` counts only boxes left unticked, so atm would have accepted the approval.

## Fix (`reviews/page.ts`, `ReviewPage.svelte`)
- **`isOpen` (new):** a top-level comment's box is unticked, exactly atm's rule. The open count, Approve's check and the Open filter use it.
- **`isResolved`:** a box ticked and none open.
- **A chain of pokes is neither,** so it is listed under *All* only and never blocks Approve. A poke chain that also holds an open comment is still open.

## Tests
- **Unit (`page.test.ts`):** a table of chains: resolved, two resolved, one open, open, a poke, a poke with an open comment, a poke with a resolved one, and none. Also the filter, with a poke's chain under *All* only.
- **e2e (`reviews.spec.ts`):** a review with a resolved comment and an answered poke: no Resolve on the poke, no open count on the Comments tab, and Approve pressable and sent.
- **Suites:** the reviews specs (59), the unit tests and the checks pass.

You can approve CR-49 once this is in. Until then the dashboard's check blocks it, though `atm review submit CR-49 --approve` would be accepted.
