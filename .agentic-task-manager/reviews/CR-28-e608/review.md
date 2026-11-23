+++
id = "01a1f094-86b7-74a1-b992-9c278f0ee608"
number = 28
title = "+ on a line opens the comment box where it is"
status = "approved"
tasks = ["ATM-625"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T21:33:48Z
updated = 2026-11-18T21:43:58Z
+++

**+** on a line now opens the comment box right under it, and the diffs pane stays where it is, wrapped or not.

**What was wrong:**
- Picking a line cuts its run in two after that line, so the comment box sits right under it.
- The first half is the same `CompMd` with fewer lines, but it kept showing its old drawing, all of the run's lines, until its next highlighting turn.
- So when the box mounted and took the focus, it stood below the whole old run, and focusing it scrolled the pane there: 444 px in the test's 40-line Markdown file, 1,564 px with Wrap (taller lines, a bigger jump).

**The fix** (`CompMd.svelte`): a drawn, paced text (a review's run of code) whose text changes is marked *stale* before the page updates (`$effect.pre`).
- Its old drawing is hidden.
- The new text shows as written (the plain copy a first draw shows) until CompMD draws it in its turn; `data-compmd` is `loading` meanwhile.
- So nothing measures or focuses against a drawing of an earlier text. This helps any run whose text changes, not only one cut by a comment.

**Tests:**
- E2e (`reviews.spec.ts`), two cases, wrapped and not: a Markdown file of 40 long lines, Unified, **+** on line 21 in the middle of the pane. The pane moves less than 40 px, and the box is in view, right under the line. Both failed before the fix (444 px and 1,564 px).
- Unit (`CompMd.component.test.ts`): a drawn text given a new one shows the new text as written with the old drawing hidden, until its turn draws it.
- Checks, all unit tests (1,714), and the functional e2e run: 502 of 503 passed.
- **The one failure:** *performance with 500 tasks › switching tabs*, the Graph tab at 247 ms against 200, in the full parallel run. Alone it is 111–142 ms, three runs in a row. This change doesn't touch the Graph tab. I didn't rerun it or raise the budget; it is ATM-629, to find the cause the way ATM-615 did.
