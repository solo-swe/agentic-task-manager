+++
id = "01a20af0-8d37-7594-86b1-c9bf4e54b5e3"
number = 138
title = "CR sub-tabs keep the diff position"
status = "open"
tasks = ["ATM-730"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T00:24:26Z
updated = 2026-11-24T00:24:26Z
+++

Each of a code review's sub-tabs now keeps its place. Switching to Overview or Comments and back to Diffs returns the diff list to where it was, and so does Back or Forward.

How it works (`ReviewPage.svelte`):
- A sub-tab's pane is rebuilt each time it is shown, so the place is saved while you scroll and put back when the pane comes back.
- Overview, Comments and the file tree are restored by their scroll position (a `keepsPlace` attachment).
- The diffs are restored by the first card in view and how far into it you had scrolled. Rebuilt cards start at an estimated height, so that card is held at the same offset while the cards around it are drawn, the same mechanism a file you jump to already uses (`pinned`, now with `pinOffset`).
- Going to a file (from the tree, a link or the Comments tab), Prev/Next, or a round link still goes where it was asked, not to the saved place.

Please check:
- The new e2e test in `reviews.spec.ts`, which also runs in WebKit: 30 files, scroll halfway down, switch to Comments, then Overview, then Back and Forward, and the same card at the same offset (within 2 px). Going to a file still lands on that file.
- Whether the Overview and Comments scroll should also be kept. The task named the diffs; I did those too because it was cheap.
