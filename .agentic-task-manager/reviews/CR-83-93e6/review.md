+++
id = "01a1f9ce-24d3-7287-ada4-70197bd193e6"
number = 83
title = "release.sh tags the merged origin/main, from any branch"
status = "approved"
tasks = ["ATM-705"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T16:33:19Z
updated = 2026-11-20T17:11:14Z
+++

`packaging/release.sh` now tags the merged `origin/main` itself instead of `HEAD`. It runs from any branch and any working tree, so 0.42.0's hand-tagging won't be needed again.

- The script fetches `origin main`, reads the version from `Cargo.toml` **in that commit** and tags that commit. The branch you're on and the working tree are never read.
- The branch, clean-tree and "HEAD is origin/main" checks are gone. They only stopped runs; they never changed what got tagged.
- The new check: the script refuses a tag that already exists on `origin`, not only locally.
- If the version doesn't match, the message asks whether the release PR is merged.
- The output names the tagged commit.
- `packaging.test.mjs` has six tests:
  - it tags from another branch with untracked and uncommitted changes;
  - it tags from a `main` behind `origin`;
  - it refuses a version that only the working tree has;
  - it refuses a tag that exists only on the remote;
  - `--push`;
  - it refuses a bad version or an unknown option.
- `packaging/README.md` and `AGENTS.md` now describe this.

**Worth a look:** the tag is made from whatever `origin/main` is at the moment of the fetch. If someone merges another PR between your merge and the run, that commit gets tagged instead. The `Cargo.toml` version check still has to pass for it.
