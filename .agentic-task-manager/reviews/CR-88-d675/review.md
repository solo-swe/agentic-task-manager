+++
id = "01a1fa19-9124-70bc-8cf9-72367815d675"
number = 88
title = "The review watch judges again what was not about my work yet"
status = "approved"
tasks = ["ATM-663"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T17:55:41Z
updated = 2026-11-20T22:36:31Z
+++

Audit findings R5, R6 and R12, all in the review watch (`crates/atm-lib/src/review_listen.rs`).

**R5: what wasn't about my work then is judged again.** The watch put every comment into `seen` before judging it. So a comment judged not about the agent's work was never told, even after the agent's commit touched its file or the agent started the task it was about. Decisions were the same.
- A comment now goes into `seen` only once it is told, or passed over for good: the agent's own, another agent's, or in a review that has ended. The rest is judged again on each look while its review is open.
- A decision on a review is recorded only once it is told, or when the new status isn't a decision.

**R6: a comment known by more than its number.** `seen` was keyed by `review/cN`, and a merge renumbers clashing IDs. The key is now the review, the author, the time and a hash of the text. The old `review/cN` keys are still read, so a cursor from before this change tells nothing again.

**R12: watches of one agent sharing one lease and one cursor.**
- Each watch now has its own lease file (`<cursor>.<pid>.live`). A watch is running when any of them holds, and the old shared lease counts too. Leases that ran out are removed when found.
- The cursor is saved under the watches' lock, merged with what the agent's other watches kept, and each watch takes the merged result. So a comment one watch told isn't told again by another.

**Tests**
- New in `listening.test.mjs`:
  - a comment on a file the agent touches only in a later commit is told after that commit, once;
  - a comment and a request for changes made before the agent takes the task are both told once it does, once;
  - with two watches of one agent, the one that ends leaves the other's lease, and `loop next` still says one runs.
- New unit tests: cursors merged between two watches; a comment's key the same after a renumbering, and different for other text or another author.
- Passing: atm-lib's 292 tests, the CLI suite and clippy.

**Worth a look**
- A comment edited after it was told has a new key, so it is told once more, with its new text. I took that as right: they are new words.
- A decided review that isn't about the agent's work is looked at again on every look. That costs a header comparison per review.
- A watch that is killed, rather than ending at its time, still leaves its lease until it runs out (60 s), as before.
