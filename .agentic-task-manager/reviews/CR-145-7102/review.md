+++
id = "01a20b68-a360-716a-91de-f51434797102"
number = 145
title = "Badge for unseen code review updates"
status = "approved"
tasks = ["ATM-733"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T02:35:36Z
updated = 2026-11-24T02:38:36Z
+++

Badges for what arrived on a review while its page is open (ATM-733).

- `lib/reviews/unseen.ts`: the page remembers what it showed first (every comment ID, the last round); a comment or reply that arrives later is new until at least half of it has been in view; a round past the last one seen is new until the Diffs show it. The reader's own comments and drafts are never new to them.
- Sub-tabs: Overview badged with the new comments on the review as a whole, Diffs with the new comments on files and the new round.
- Round picker: the new round is marked "(new)", with a button beside it to show it.
- Comments: a new one is marked with a "New" label until seen.
- Tests: `unseen.test.ts` (unit), and an e2e test in `reviews.spec.ts` with comments and a round arriving while the review is open.
