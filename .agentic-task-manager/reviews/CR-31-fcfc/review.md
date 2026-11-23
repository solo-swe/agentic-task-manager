+++
id = "01a1f0cc-8d44-701e-b161-07a5ad7efcfc"
number = 31
title = "The diffs follow the file tree's order"
status = "approved"
tasks = ["ATM-596"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T22:34:59Z
updated = 2026-11-19T05:43:32Z
+++

The diffs now come in the file tree's order, not git's. In CompMD's CR-1, `pnpm-lock.yaml` was last in the tree but in the middle of the diffs; now it's last in both.

## Change
- `treeOrder` in `page.ts` lists the view's files as the tree reads them, top to bottom: folders before the files beside them, each level by name, a submodule where its folder is.
- The page takes that order as its `files`, so these all follow it:
  - the cards;
  - `]` / `[` and Prev / Next change and comment (`n p c C`);
  - the round's file select;
  - the Viewed count.
- Folding a folder in the tree folds nothing on the right.

## Tests
- Unit tests of the order: none, one, nested folders with a lock file at the root, and a submodule. The tree and the order agree.
- An e2e test that the cards and `]` follow the tree when the view lists the files in git's order.
- Updated tests that assumed the view's order: three component tests and the e2e change-navigation test.
- The full functional suite passes (508 tests).
