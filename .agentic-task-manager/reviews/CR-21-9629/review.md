+++
id = "01a1f00a-11b9-7488-8b23-a5fdb4d19629"
number = 21
title = "A long review is as long as its files: cards off screen keep their height"
status = "approved"
tasks = ["ATM-615"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T19:02:34Z
updated = 2026-11-18T19:37:21Z
+++

A long review is now as long as its files: every card off screen keeps its height.

**What was wrong** (at every speed, not only under load):
- The diffs pane stacks the cards in a flex column. A card the browser skips off screen (`content-visibility: auto`) has a least height of 0, so the column squeezed every card off screen to its 2 px border. The pane was a few screens long instead of about 115,000 px, and the test's scroll reached the "end" after a few dozen steps. Under load, too few to reach the last file.
- A card that left the view while its diff was still loading was remembered by the browser at that height, a line high, so the pane shrank under the reader.
- A card scrolled past before the page was idle was still made once it was, far from the view by then.

**The fix**, in `ReviewFile.svelte`:
- The card is `flex: none`, and it stands at its estimate plus its header until it is drawn.
- A body still loading stands at the estimate.
- A card's height is kept only from a drawn diff.
- Leaving the margin cancels a card's pending make.

**Tests:**
- The long-review spec checks that no card is shorter than its header.
- With the CPU slowed 4×, it passes 10 times in a row: the first file drawn in 184 to 198 ms, about 143 steps, no rows far from the view. The slowdown was used for this run only and isn't left in the spec.
- The full functional run, the unit tests and the checks pass.
