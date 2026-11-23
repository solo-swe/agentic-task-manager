+++
id = "01a1f034-c6de-725d-9935-10f562d182b7"
number = 23
title = "The Diffs tab file tree folds its folders, and all at once"
status = "approved"
tasks = ["ATM-595"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T19:49:13Z
updated = 2026-11-18T20:48:52Z
+++

The file tree beside the diffs now folds its folders.

**What you can do:**
- **One folder:** its row has a chevron. A click, or ← and → on it, collapses or expands it, with its files and subfolders.
- **All at once:** *Collapse all* and *Expand all*, icon buttons at the top of the tree (Lucide's chevrons-down-up and chevrons-up-down, with titles).
- **A folded folder sums what is in it:** `src 12 files · 3 with comments · 5 not viewed` (or *none viewed*).

**What it keeps:**
- What is folded is kept per review for the session, so a reload keeps it. It is stored nowhere else.
- Going to a file opens the folders on its way: a click in the diffs, ] and [, the address's `file=`, or the file at the top as you scroll. The file shown is always in the tree.

**For keyboards and screen readers:** the list is `role="tree"`, its rows `treeitem`s with `aria-level`, a folder `aria-expanded`, the file shown `aria-selected`.

**How:** `page.ts` gets four small helpers (`shownRows`, `treeFolders`, `openedTo`, `filesIn`), and `ReviewPage.svelte` the state, the buttons and the styles.

**Tests:**
- Units: the helpers, at any depth, with a folder named like the start of another (`a` and `ab`).
- The component: click and keys, the sum, Collapse all and Expand all (and none without folders), kept for the session per review, and a jump to a file opening its folder.
- An e2e test: one folder folded, kept across a reload, all folded, and ] opening the folder of the file it goes to.
- The full functional run passes.
