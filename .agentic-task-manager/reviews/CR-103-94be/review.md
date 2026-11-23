+++
id = "01a1fb5d-1e5d-7580-a346-5f2f710794be"
number = 103
title = "Split the large files: reviews.rs and review_pages.rs done (moves); plans for review_listen.rs and the CLI review command"
status = "approved"
tasks = ["ATM-711"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T23:49:06Z
updated = 2026-11-20T23:50:47Z
+++

The large-file split (ATM-711), by the Rust guide: `foo.rs` next to `foo/`, never `mod.rs`; unit tests with their code; integration tests in the crate's `tests/`. The first two files are done, each a move. The next two files' plans are below, for you to agree before I touch them.

**Done:**
1. **`atm-lib/src/reviews.rs`** (2,237 lines) is now `reviews.rs` (60: re-exports, `store`) plus `reviews/repo.rs` (62), `doctor.rs` (105), `naming.rs` (102), `linking.rs` (234), `commands.rs` (296) and `live.rs` (140).
   - The 25 tests on a real git repository and the real store are integration tests now: `crates/atm-lib/tests/reviews/` with `main.rs`, `fixtures.rs` and a file per area (`listing`, `opening`, `decisions`, `doctor`, `live`, `submodules`, `keys`). Paths became `atm_lib::…`, and the shared helpers are public in the test crate; no test's text changed otherwise.
   - The one test of a private helper (`check_decision`) stays a unit test, in `reviews/tests.rs`.
2. **`atm-lib/src/review_pages.rs`** (1,095 lines) is now `review_pages.rs` (38) plus `review_pages/list.rs` (137), `waiting.rs` (90), `patches.rs` (146), `view.rs` (217) and `page.rs` (135). Its tests build their reviews in memory, so they're unit tests: `review_pages/tests.rs` (367).

In both, private helpers became `pub(super)` so their sibling modules reach them, and each module names what it uses (the workspace's clippy denies `*` imports). Same public API. Same tests: 274 unit and 25 integration, 299 as before. Clippy is clean and the workspace builds.

**Next, to agree:**

3. **`atm-lib/src/review_listen.rs`** (974 code, 538 tests) becomes `review_listen.rs` (the doc, `RECHECK`, the re-exports) and, in `review_listen/`:
   - `wait.rs`: `WaitFor`, `heard`, `with_parents`, `decided`, `listen`, `wait`;
   - `work.rs`: what is the agent's work (`Work`, `work_of` and the commits' files cache, `touches`, `Mine`, `about_my_work`, `decision_about_my_work`, `keyed_now`);
   - `cursor.rs`: `Cursor`, its file, `seen_key`, `save_cursor`, `starting_point`, `all_reviews`;
   - `watch.rs`: `WatchOptions`, `watch`, `look`, `decision`;
   - `inbox.rs`: ATM-708's inbox and the unread line;
   - `lease.rs`: the watch's lease, `WatchState`, `watch_state`, the not-watching reminder.

   Its tests use temporary folders for the cursor and lease files; they stay unit tests, in `review_listen/tests/` by the same groups.

4. **`atm-cli/src/commands/review.rs`** (1,386 code): the arguments stay in `review.rs` (`ReviewCommand`, `DraftCommand`, `Change`, `Body`) with the shared helpers (`opened`, `actor`, `text_of`, `new_round`, `summary_line`, `parse_lines`, `status_of`).
   - The 650-line `run` becomes a short dispatch to a function per group of commands, in `review/`: `open.rs` (create, live, round, tasks), `read.rs` (list, show, comments), `talk.rs` (comment, poke, reply, edit, resolve, drafts), `decide.rs` (submit, close) and `listen.rs` (wait, watch, inbox and its hook, `heard_line`, `duration_of`).
   - Each arm's code moves unchanged into its function. That isn't a pure line move: the `match` is cut into functions.

After those, the rest of the list in ATM-711 (atm-review, atm-vcs, atm-docs, the app, atm-api), each with a plan first.
