+++
id = "01a1ed55-1201-727f-ab0f-371b976bf1c4"
number = 11
title = "A review sent back can be approved"
status = "approved"
tasks = ["ATM-609"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T06:25:37Z
updated = 2026-11-18T06:31:52Z
+++

A review in *Changes requested* can now be approved. It no longer waits for its author's next round, as long as every top-level comment is resolved, as for any approval.

- **The rule** (`atm-review` `rules.rs`): Approve is allowed from `changes_requested` as well as `open` and `live`. Its tasks in review are approved as with any approval: their review moves from *changes requested* to *approved*.
- **The page:** ATM-601's *Sent back* reason for disabling Approve is gone. On a sent-back review with everything resolved, Approve is one click. With a comment open, it stays disabled and says why.
- **Docs:** the state diagram and the status table in `docs/code-review-design.md`.

Tests:
- The rules' status and decision table: sent back plus Approve gives approved.
- atm-lib: a review sent back is refused while its comment is open; once the comment is resolved, it's approved with no round since, and its task too.
- Component: on a sent-back review, Approve is one click with everything resolved and disabled with a comment open.

A task the agent already took back to *in progress* isn't moved by the approval; it gets its review again with its next round, as before.
