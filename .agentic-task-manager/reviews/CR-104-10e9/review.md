+++
id = "01a1fb6d-1c2f-74ba-bc7d-a4c9eea510e9"
number = 104
title = "Split the large files, part 2: review_listen.rs and the CLI review command"
status = "approved"
tasks = ["ATM-711"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T00:06:34Z
updated = 2026-11-21T00:08:07Z
+++

Round 2: the two files whose plans you approved in round 1.

3. **`atm-lib/src/review_listen.rs`** (1,512 lines) is now `review_listen.rs` (51) plus `review_listen/`:
   - `wait.rs` (128), `work.rs` (207), `cursor.rs` (111), `watch.rs` (185), `inbox.rs` (126), `lease.rs` (240);
   - its tests in `review_listen/tests.rs` (92, the shared builders) and `tests/{work,cursor,heard,inbox,lease}.rs`.

   They make reviews in memory and use temporary folders for the cursor and lease files, so they stay unit tests. A move: the fields and methods that siblings or tests reach became `pub(super)`.

4. **`atm-cli/src/commands/review.rs`** (1,486 lines) is now `review.rs` (642: the arguments, the shared helpers, a dispatch `run` of 9 lines, its unit tests) plus `review/`:
   - `open.rs` (167), `read.rs` (210), `talk.rs` (276, with the drafts' commands), `decide.rs` (84), `listen.rs` (225, with the hook);
   - each group's `run` holds its commands' arms as they were.

   Changed besides the move:
   - the `match` is cut by group, each with an `unreachable!` for the others;
   - `listen`'s arms lost their `return`, since its `match` is the function's tail;
   - the modules are declared after the `as_json!` macro they use;
   - each group keeps the old `run`'s `allow(clippy::too_many_lines)` ("one arm per command"), over 84 to 276 lines now instead of 650.

   `review.rs` is still 642 lines, nearly all of it the clap argument types, which stay together as the plan said. Moving them to a `review/args.rs` would leave about 200 lines; say if you'd like that.

Tests:
- atm-lib: 274 unit and 25 integration; atm-cli: 36;
- the CLI suite (the review commands as users run them);
- clippy, with the workspace's lints.

Next, by ATM-711's list, each with its plan first: atm-review (`rules.rs`, `folder.rs`, `rounds.rs`, `files.rs`), atm-vcs, atm-docs, atm-api's review routes and the app.
