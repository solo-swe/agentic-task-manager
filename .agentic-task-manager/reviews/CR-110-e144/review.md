+++
id = "01a1fefe-9266-75fc-8b98-7e02a16ae144"
number = 110
title = "Split the large files, part 4: atm-api routes/reviews, atm-docs, atm-vcs"
status = "approved"
tasks = ["ATM-711"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T16:44:18Z
updated = 2026-11-21T17:02:40Z
+++

The split, part 4: the plans you approved on CR-106, for atm-api, atm-docs and atm-vcs. Each is a move only, with the same tests passing. The app's `commands.rs` and `lib.rs` come next, once ATM-677's uncommitted edits to them are in.

**Done:**
1. **atm-api `routes/reviews.rs`:** the code stays, and its tests (17) are in `routes/reviews/tests.rs`.
2. **atm-docs `lib.rs`:** the code stays, and its tests (30) are in `tests/` by group:
   - `paths` (6): the paths it takes, hidden files, key files, a file's kind;
   - `rendering` (8): Markdown, fences, images, binary;
   - `reading` (16): at a revision, in a submodule, from the working copy; the repository fixture is here.

   **One change from the plan:** the plan said `paths`, `documents` and `images`. The image tests are spread over all three concerns and share the reading fixture, so the groups follow the code instead: `paths`, `rendering`, `reading`.
3. **atm-vcs `lib.rs`** (771 lines of code), as planned:
   - `git.rs`: `run`, `run_fed`, `ask`, `step`, now `pub(super)`. It is still the one place the crate starts git (I24).
   - `paths.rs`: `normalize`, `real_path`, `relative_to`.
   - `GitVcs`'s methods are in `impl` blocks by concern:
     - `commit.rs`: `known`, `paths`, `changes_under`, `commit`;
     - `history.rs`: `resolve`, `resolve_tree`, `in_history`, `message`, `first_commit_adding`, `head`;
     - `files.rs`: the working copy's and a commit's files, ignore rules, tracking, the working state, submodules;
     - `remote.rs`: `default_base`, `file_on_remote_branch`, `remote_url`.
   - `lib.rs` keeps `open`, `GitVcs` and the methods about the repository itself (`new`, the submodule helpers, `in_repository`).
   - Two private methods became `pub(crate)` because they are now used across files: `known` and `submodule`. The public API is unchanged.
   - The tests (36) are in `tests.rs` (the fixtures, and the tests of the crate as a whole) and `tests/{paths, commit, files, history, remote}.rs`.

**How it was moved:**
- `csplit` cut each file at its line numbers, and a few methods and tests were moved with Edit first, so each group was one run of lines. No text was retyped.
- rustfmt re-indented the moved code. It never changes a string literal.

**Checked:**
- `cargo test --workspace` passes: the same counts for atm-api (65), atm-docs (30) and atm-vcs (36).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass.
