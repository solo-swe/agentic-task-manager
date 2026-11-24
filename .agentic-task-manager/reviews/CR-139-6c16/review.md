+++
id = "01a20af8-6a9f-72ff-a072-74633da86c16"
number = 139
title = "Keep an unsent comment or reply when the review updates"
status = "open"
tasks = ["ATM-732"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T00:33:02Z
updated = 2026-11-24T00:33:02Z
+++

A comment or reply being written in the diff view now survives updates to the review: its text, its cursor and the focus.

**The cause:** a diff's rows are cut into segments at each chain, and the `{#each}` over them is keyed by where each segment starts. When an agent's comment arrives above a box, the segments after it are cut again and rebuilt, and the box is rebuilt with them. A reply's text lived in that `ChainBox`, so it was lost. The new-comment box kept its text, which lives in `DiffView`, but lost the focus and the cursor, so the next keystrokes went nowhere.

**The fix:** a new module, `reviews/unsent.ts`.
- **What it keeps:** text that hasn't been sent, with its cursor and whether the box had the focus. It is keyed by what the box is for (`comment:<file>`, `reply:<comment id>`, `edit:<comment id>`) and updated as the text changes.
- **The attachment:** `keepsUnsent(key, focus)` replaces `focusOnShow` on the three boxes. A box made again with something kept gets its cursor back, and the focus only if it had it, so a box rebuilt while you type elsewhere doesn't take the focus from you. A box opened fresh is focused as before.
- **`ChainBox`:** a rebuilt chain reopens the reply or edit that was in progress, with its text.
- **Forgetting:** sending, Cancel, Escape and picking other lines forget the kept text. `ReviewPage` forgets everything when it goes, so nothing carries over to the next review (ATM-675).

**Please check:**
- The e2e test in `reviews.spec.ts`: a half-written reply on line 3 and a comment being written on line 31 with the cursor inside it, then agent comments arriving on lines 2 and 30. Both texts are kept, the comment box keeps the focus and the cursor, and typing continues where it was. I checked that it fails without the fix (the reply box is gone).
- The unit tests in `unsent.test.ts` and `ChainBox.component.test.ts`.
- A side effect: a reply you leave open without cancelling and then lose (by folding its chain, say) reopens with its text the next time that chain is shown on the same review page.
