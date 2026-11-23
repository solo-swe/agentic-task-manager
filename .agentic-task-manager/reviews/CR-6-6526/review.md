+++
id = "01a1ecbb-725d-725e-b2f5-dc36a09d6526"
number = 6
title = "Approve is disabled, saying why, while it would be refused"
status = "approved"
tasks = ["ATM-601"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T03:37:49Z
updated = 2026-11-18T05:59:23Z
+++

**The bug:** with comments still open, *Submit review*'s **Approve** was enabled, and pressing it failed with atm's refusal.

**The fix** (`ReviewPage.svelte`): Approve is disabled, and says why, while the review's rules would refuse it:
- comments are open: "1 comment is still open: resolve them first" (or "N comments are");
- the review is sent back: "Sent back: it is approved after its author's next round". A sent-back review with nothing open no longer offers the one-click Approve either.

**Where the reason shows:**
- **On hover:** on the button's wrapper, since a disabled button doesn't show a tooltip in every engine.
- **Below the buttons:** as a line with an ⓘ.
- **For screen readers:** as the button's description.

**Tests**
- The page: each reason disables Approve and says it; with nothing open, Approve is enabled with no reason.
- e2e: the submit flow sees Approve disabled with its reason while a comment is open, and checks a refusal with *Comment* instead of Approve.
