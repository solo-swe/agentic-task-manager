+++
id = "01a1ed77-46c7-73d1-a630-a4032f4e8edb"
number = 12
title = "Reviews waiting for you, wherever you are"
status = "approved"
tasks = ["ATM-606"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:02:59Z
updated = 2026-11-18T08:10:40Z
+++

You can now see which reviews are waiting for you, wherever you are in the app.

**What counts as waiting for you** (`review_pages.rs` `waiting_for`):
- **review:** the decision or round is yours (the list's existing *Needs you*: open or live with you a reviewer, or sent back to you as its author).
- **answered:** in a review you take part in (its author, a reviewer it names, or someone who wrote in it), not yet decided and not a draft, someone else wrote after you last did. "After" goes by the comments' numbers, since two comments in the same second share a time. It clears when you write back, or when the review is decided.
- Nothing is stored about what you've read: a GET still writes nothing (security I11). That's the one difference from what the task said: there is no "seen" record, so opening a review doesn't clear it; answering it does. Tell me if you want a real "seen", kept in the browser.

**Where it shows:**
- Every project's summary gets `needs_you.code_reviews` and `needs_you_reviews`, the longest waiting first. A task in review whose review is counted there isn't counted again in `needs_you.review`.
- **The Reviews tab** has a yellow count, named for screen readers ("Reviews, 2 waiting for you").
- **The project switcher's** badge and dot include it. It now uses the shared `needsYou`, which also fixes it leaving out work done together.
- **The dock badge** and the notification count it, and the notification now says "N things need you".
- **The list:** a review answered since you wrote is marked *New reply* with the *Needs you* stripe, and *Next review needing you* takes it too.

**Keeping it fresh:**
- The summary's stamp includes the reviews' files (`FileStore::stamp`: their number and latest change), so the summary cache and `api/projects`' ETag move with a comment.
- The app watches a project's reviews folder when it has one. A project without one isn't given one.
- One read of the store per summary (`FileStore::reviews`), not one per review.
- "You" is the user at the machine (their git name), as review requests are, never an agent's identity from the server's environment.

**Tests:**
- Rule table (`review_pages`): each case: decision yours, answered, you wrote last, not yours, a draft, live, approved, closed.
- atm-lib (`reviews.rs`): a real project's summary counts a review once and not its task, sending it back clears it, the agent's reply makes it *answered*, and the stamp moves.
- App badge: the count includes code reviews, and the new wording.
- Unit tests: `needsYou` and `reviewsWaiting`.
- E2e: the tab's count and accessible name, *New reply* on an answered row, and no count when nothing waits.
- CLI suite: the summary JSON with the new fields.
