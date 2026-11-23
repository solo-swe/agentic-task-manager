+++
id = "01a1fe54-9117-767d-b8a2-93fa06791d64"
number = 106
title = "Split the large files, part 3: atm-review, the CLI command groups; plans for the rest"
status = "approved"
tasks = ["ATM-711"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-21T13:38:37Z
updated = 2026-11-21T13:57:54Z
+++

The split, part 3: the atm-review files you agreed to (ATM-711's question), and the CLI's command groups as you asked on CR-105. Each is a move only, with the same tests passing.

**Done:**
1. **atm-review `rules.rs`** (529 lines of code, 1,162 of tests):
   - The table (`ReviewEvent`, `Next`, `review_next` and its two helpers) is in `rules/table.rs`, re-exported, so the API is unchanged. `rules.rs` keeps the rules' helpers and the `Reviews` methods.
   - The tests are in `rules/tests/` by group: `table` (15), `deciding` (10), `comments` (8) and `rounds` (6). The shared fixtures are in `rules/tests.rs`.
2. **`folder.rs`, `rounds.rs`, `files.rs`:** one concern each, so only their tests moved out, to `folder/tests.rs`, `rounds/tests.rs` and `files/tests.rs`. They are unit tests on temporary folders, or on the store alone.
3. **The CLI's `atm review` commands (CR-105, c4 and c7):**
   - Each group has its `Command` enum in its module: `open`, `read`, `talk` (with `DraftCommand`), `decide` and `listen`.
   - `ReviewCommand` is `Open(open::Command)`, …, `Listen(listen::Command)`, each with `#[command(flatten)]`, so the command line is the same.
   - Each `run` takes only its own enum, so there are no `unreachable!` fallbacks.
   - One visible change: `atm review --help` lists the commands by group, so `close` comes before `wait`.

**Checked:**
- `cargo test -p atm-review` passes, the same 137 tests. `cargo clippy --all-targets` passes, and so do atm-cli's tests and the whole CLI suite.
- **Every moved string literal is unchanged.** My split script first took four spaces off every test line, which changed the text of three files tests' multi-line strings (the tests caught that). I redid those moves with no de-indent (rustfmt indents the code, never a string's text). I also compared every string literal before and after each earlier split (CR-103, CR-104). The only difference is one `\` line continuation in review_listen, whose value is the same, since Rust drops the next line's leading spaces.

**The plans for the rest, to agree (each a move):**
- **atm-vcs `lib.rs`** (771 code, 642 tests):
  - The git process helpers (`run`, `ask`, `step`) go to `git.rs`, and the path helpers (`normalize`, `real_path`, `relative_to`) to `paths.rs`.
  - `GitVcs`'s methods are split into `impl GitVcs` blocks by concern: `commit.rs` (staging and committing), `history.rs` (revisions, log, base), `files.rs` (status, tracked, ignored, submodules) and `remote.rs`.
  - The tests go to `tests/` by the same groups. They run real git, the unit's own dependency, so they stay unit tests.
- **atm-docs `lib.rs`** (535, 836): the code stays. The tests go to `tests/` by group: `paths`, `documents` and `images`.
- **atm-api `routes/reviews.rs`** (391, 778): the code stays, and the tests go to `routes/reviews/tests.rs`.
- **The app's `commands.rs`** (1,114, 425 unit, plus integration tests):
  - Split by area: `commands/state.rs` (`AppState`, `AppError`, the filters), `commands/reads.rs` (projects, tasks, documents, files), `commands/reviews.rs` (the review and draft commands) and `commands/windows.rs` (the subscriptions and window commands).
  - The unit tests go to `commands/tests.rs`. The `integration_tests` go to the app's `tests/` folder where they use only the public API, else to `commands/integration_tests.rs`.
- **The app's `lib.rs`** (236, 1,283): the tests go to `tests.rs`, and `integration_tests` as for commands.rs.
