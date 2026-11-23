+++
id = "01a1ed82-267b-7074-a78a-8bb86dd23b8e"
number = 13
title = "Open means not yet decided; Waiting for review is the status"
status = "approved"
tasks = ["ATM-613"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:14:52Z
updated = 2026-11-18T08:12:39Z
+++

Round 2 fixes a compatibility break in round 1, which you found: "7 tasks in review, but only 1 open CR".

**Cause:** round 1 renamed the stored status too, so `review.md` was written with `status = "waiting"`. atm 0.40.0, which your app runs, doesn't know that word, and its review list silently skips a review it can't read. CR-13 to CR-19 disappeared for you.

**Fix** (`atm-review` `files.rs`): the file keeps writing this status as `open`, as atm before 0.41.0 wrote and reads it. It also reads `waiting`, as written for a while. Everywhere else (`--json`, REST, the dashboard) it's `waiting`, *Waiting for review*, as in round 1. A clone or an app on an older atm sees every review.

**The seven reviews already written:** I changed `status = "waiting"` to `status = "open"` in CR-13 to CR-19's `review.md`, one line each and nothing else. `atm review list` on the installed 0.40.0 now lists all eight open reviews.

Tests:
- atm-review: waiting is written `open`, and `open` and `waiting` both read as waiting; every other status by its own name both ways.
- The CLI suite: a new review's file says `open`, and `waiting` in the file reads the same.

Separately, 0.40.0 silently skipping a review it can't read is a bug of its own. A review shouldn't vanish without a word. That's a follow-up task (ATM-617).
