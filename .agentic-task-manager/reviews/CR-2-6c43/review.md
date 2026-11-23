+++
id = "01a1ec3b-fe1f-7571-80a6-743e459f6c43"
number = 2
title = "A review sent back takes changes asked for again, with its message"
status = "approved"
tasks = ["ATM-597"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T01:18:37Z
updated = 2026-11-18T06:38:29Z
+++

Round 2 does step 1 of what we agreed in c1/c2. A review already sent back isn't sent back again: your further words are *Publish comments*.

- **The page:** on a sent-back review, the decision panel shows **Publish comments** (it needs a summary) where *Comment* and *Request changes* were. Approve is there too, now that a sent-back review can be approved (ATM-609, CR-11).
- **The rule** (`rules.rs`): a second *request changes* is refused again (`CR-1 is changes requested: it can't be sent back`), so the CLI and the page agree. Comments, replies and a decision's comment are still taken, as ATM-597 asked.
- **Step 2, drafts:** that's **ATM-612**, *Draft comments, sent together with the review's decision* (this sprint). It covers *Add to my review* / *Comment now* / *Cancel*, the count on the top button, drafts kept in your own atm state and not in the repository, and replies drafted too.

Tests: the rules table (sent back plus request changes is refused), the CLI suite (refused, then `--comment` kept), and the component test (only *Publish comments* is offered, and it sends a comment).
