+++
id = "01a20eb3-6daf-747d-8d1c-fc8e270bc417"
number = 169
title = "Reviews: the list reads each review once"
status = "approved"
tasks = ["ATM-775"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T17:56:09Z
updated = 2026-11-24T18:11:49Z
+++

The Code Reviews list took seconds to show (ATM-775, from Hakan: "5-10 seconds latency before the review list populates").

**Cause:** `list_reviews` read each listed review with `whole`:
- `whole` opened its working copy's store, a `git rev-parse` each;
- it asked the store for the review, and `FileStore::get_review` finds one by reading every review in the folder (`read_all`).

So a list of n reviews read n² of them: with this repo's 166, about 27,000 reads per request. The page asks per project, and All projects asks for each one.

**Change (`review_pages/list.rs`):** `Wholes` reads each working copy's reviews once, whole (`FileStore::reviews`), when the first review found there is asked for, and looks each one up by ID. A review not found there falls back to the main store's `get_review`, as before.

**Measured** on this repo, the reviews route on `atm _serve` (release builds, three requests each):
- before (the installed atm): 1.76, 1.74, 1.76 s;
- after: 0.074, 0.062, 0.058 s;
- the same answer both times, 106,312 bytes.

**Test:** `the_list_reads_each_review_whole_from_its_own_working_copy`. A review in the main copy, and one only in a worktree with a comment. Both are listed with their branch and comment counts, and the worktree review's words (its summary and its comment) are searched in its own copy.

`cargo test -p atm-lib` (334 + 45 passed), clippy, the CLI suite (no failures), the functional e2e suite (624 passed).

Not changed: with `?task=`, `list_naming` still opens a store per review to resolve old keys (only for a task's own reviews).
