+++
id = "01a1fa11-d8c8-75de-a8a8-cae62c8d2c39"
number = 87
title = "No revision reaches git as an option: a Rev type, rounds checked when read"
status = "approved"
tasks = ["ATM-659"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T17:47:16Z
updated = 2026-11-20T22:35:26Z
+++

Audit finding R3: a round's `base` and `head` are read from `rounds/N.json`, a file that is committed and merged. They went to git unchecked: `read-tree <base>`, `rev-parse <head>^{tree}`, and others. So a value like `--index-output=x` became an option to git, just by opening the review's page.

**Change**
- **atm-vcs, `Rev` (`rev.rs`):** a revision is either a full object ID (`Rev::object_id`) or a name (`Rev::new`) that doesn't start with `-` and has no control character or space.
  - Every method that hands git a revision now takes a `&Rev`, so no unchecked string reaches git: `resolve`, `resolve_tree`, `in_history`, `message`, `blob_in`, `gitlink_in`, `blob_bytes`, `diff_range`, `diff_working`, `tree_after`, `diff_trees`, `file_in` and `tree_of`.
  - Each git command also gets `--end-of-options` before its revisions.
- **atm-review:**
  - `RoundKind::check`: a round's commits must be full object IDs. The folder store applies it as it reads `rounds/N.json`, and a round that fails is refused.
  - A lookup of a review that can't be read now gives the reason ("CR-1 can't be read: …rounds/1.json: its base "--index-output=x" isn't a commit's full ID"), not "no review CR-1". `atm doctor` names it too.
  - `rounds::tree` returns a `Rev`.
- **Callers:**
  - The CLI's `--record` and the document reads at a revision (`atm-docs`) check what the user gave.
  - A task's `commit` field, which is task text, is checked before the review watch diffs it.
  - The submodule and gitlink lookups take only full IDs.
- **`docs/security-architecture.md`:** a new invariant, **I32**.

**Found on the way:** without `--verify`, `git rev-parse` echoes `--end-of-options` back as part of its answer. `tree_of` returned it inside the tree's ID until the new `object_id` check refused it. It now uses `--verify`, as the other `rev-parse` calls do.

**Tests**
- New:
  - atm-vcs `rev.rs`: names accepted, and options, NUL, newline and spaces refused; object IDs for SHA-1 and SHA-256.
  - atm-review: `a_rounds_commits_are_full_object_ids_or_it_is_refused`.
  - CLI: `wrong-uses.test.mjs` "a round whose commit is no full ID…". A round edited to `--index-output=x`, `-p` or `HEAD` is refused by `review show` (whole, and its round 1), `atm doctor` names it, git writes no file, and the round as atm wrote it reads again.
- Changed: the fixtures that go through the folder store now use full IDs.
- Passing: `cargo test --workspace`, the CLI suite, the dashboard's 117 review and document specs, and clippy.

**Worth a look**
- Every unit test of the atm-vcs methods ("each refuses a revision starting with `-`") is now the type itself. A method can't be called without a `Rev`, and `Rev::new` refuses one.
- A review folder whose round file was edited by hand into a short SHA or a branch name is now unreadable until it's fixed. atm itself has only ever written full SHAs.
- `--end-of-options` needs git 2.30 or later for `rev-parse` (2.24 for the others). CI's runners and macOS's git are newer.
