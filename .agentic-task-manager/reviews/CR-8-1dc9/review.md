+++
id = "01a1ed41-fea6-7414-a731-9357b2db1dc9"
number = 8
title = "Live reviews: follow a change as it is made"
status = "approved"
tasks = ["ATM-576"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T06:04:47Z
updated = 2026-11-18T06:11:48Z
+++

You can now review a change while an agent is making it.

- **New live review** on the reviews list opens one. You give it a title, optionally a task and the commit to compare against (HEAD by default), and its page opens.
- The page follows the working copy ("Following the working copy"), asking again every 2 s. The round picker's *Now* is the copy as it is; the earlier rounds are the checkpoints.
- A comment, or an approval, first keeps what you saw as a checkpoint (a `working` round), so its lines stay anchored while the agent goes on.
- A live review isn't sent back. It ends when you approve it, when the agent's `atm review round` makes it open, or when you close it, saying why (*Close review*).
- An approved live review covers its task while nothing has changed since.

Commits: 79e8ade (the feature: atm-lib's `RoundDiffs`/`view.now`, the REST and CLI checkpoints, the page), ea6822b (e2e and docs).

Tests: atm-lib covers the page following the copy, a comment's checkpoint, and approving with and without a change since. The e2e tests cover a file written while the page is open showing up, a comment sent, Approve and Close without Request changes, and the New live review form.
