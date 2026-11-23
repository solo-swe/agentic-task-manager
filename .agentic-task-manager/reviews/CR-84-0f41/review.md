+++
id = "01a1f9f2-d79e-77d5-ab78-c9603b750f41"
number = 84
title = "Tag a release with plain git; the Release workflow checks the tag"
status = "approved"
tasks = ["ATM-705"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T17:13:24Z
updated = 2026-11-20T17:17:25Z
+++

This is option 1 from CR-83: a release is now tagged with plain `git tag`, and the Release workflow checks the tag.

- **The script:** `packaging/release.sh` and its tests are deleted.
- **The workflow:** `.github/workflows/release.yml` has a new first job, `tag` ("The tag is main's version"), on `ubuntu-24.04`. It checks out with full history and without stored credentials, and checks two things:
  - the tag matches the version in the tagged commit's `Cargo.toml` (moved here from `build` and `app-macos`, which no longer check it themselves);
  - the commit is on `main` (`git merge-base --is-ancestor "$GITHUB_SHA" origin/main`).

  `build` and `app-macos` both need it, so a wrong tag fails within seconds, before anything is built or signed.
- **The docs:** `packaging/README.md` (*A release*) gives the three commands (fetch, `git tag -a v<version> origin/main`, push) and how to delete a wrong tag. `AGENTS.md`'s CI section is updated too.
- **Tests:** `ci-workflow.test.ts` checks the `tag` job's two checks and its full history, and that both build jobs need it.

**Worth a look:** this is a CI change, a new job that needs no new permissions. The token stays read-only, and the one `contents: write` grant is unchanged. A tag on a release branch, or on an unmerged commit, now fails the release. Before, only the version was checked.
