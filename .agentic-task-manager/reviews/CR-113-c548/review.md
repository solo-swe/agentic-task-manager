+++
id = "01a20431-72ce-7505-b2ec-5fb5428bc548"
number = 113
title = "Review rounds as increments: one view function, rounds recorded from the round before"
status = "approved"
tasks = ["ATM-725"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T16:57:59Z
updated = 2026-11-22T17:26:44Z
+++

Review rounds are a chain of increments, and every view adds them up.

## What changed
- **One way to make a view:** `atm_review::rounds::between(review, Since::Base | Since::Round(M), N)` diffs round N's tree against the review's base commit (whole) or round M's tree (since M). So whole(N) = whole(M) + since(M, N), whatever range each round was opened with. `compare` is `between` since M. atm's task folder is in no view; a view spanning a files round shows only the files those rounds name (ATM-622 kept).
- **atm-vcs** `diff_between(from, to, only, leave_out)`: a diff of two trees or commits with its files.
- **The page's view** (`review_pages/view.rs`) and **`atm review show --round N [--against M]`** both use `between`. The whole change used to be round N's own stored diff from its own base, so it differed from review to review.
- **Recording rounds:** `rounds::as_increment`, called by `reviews::add_round`: a range round that doesn't start where the round before ended is recorded from there, and `atm review round` says so. A working or files round, or a previous head not in this clone, is kept as given.
- **Comments:** `rounds::in_view` decides whether a view holds a comment's round (made on K: whole(N) for K ≤ N, since(M, N) for M < K ≤ N). The page gets `in_view` on each chain and draft; a comment not in view is listed on its file as "A comment on round K", with a link there.
- **Picker:** "Round N (whole)", "Since round M", "Round N only".
- **Docs:** `docs/code-review-design.md` §3.6 (the model, in a table), `docs/multi-project-api.md`, `docs/agent/reviews.md` (agents no longer name the first round's base), `docs/security-architecture.md`.

## Tests
- A table over three reviews of the same three commits, opened cumulatively, incrementally and neither: every whole(N) and since(M, N) equals git's diff between those commits, and the views meet at each round's tree.
- A working round after commits adds only its own change; `as_increment`'s cases; `in_view`'s table; a view must start before its round.
- The page's helpers (`elsewhereOf`), the drafts, and two e2e specs: the picker's three views, and a comment on another round listed with its link.

## To look at
- Existing review folders are read through the new views without being rewritten: a review whose rounds were stored cumulatively now shows its whole change and its since views correctly, but each stored round's own `files` list (the Rounds card) is still as it was recorded.
- `docs/agent/` changed: the agent sandbox, or the written skill review, before the release PR.
