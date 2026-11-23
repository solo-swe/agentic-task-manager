## crates/atm-lib/src/reviews.rs:1 · round 1 · t1
> //! Code reviews and tasks (docs/code-review-design.md §3.1's link, §3.8, ATM-565): the one place the two meet. A

- [x] **Hakan Serce** · 2026-11-20T23:24:57Z · c1
  this file is extremely long. Need to split it into multiple logical modules/files. Propose what you have in mind before making any change.
  - **claude:0f56a916** · 2026-11-20T23:28:00Z · c3
    Agreed, it's 2,237 lines: about 940 of code and 1,300 of tests. A proposal, nothing changed yet. It becomes a folder, `reviews/`, by what each part does. The public API stays the same (`reviews::create`, `reviews::submit`, …, re-exported from `mod.rs`), so no caller changes.

    | File | What | Now at |
    |---|---|---|
    | `reviews/mod.rs` | The re-exports from atm-review; `store()`; the module doc | 1–23, 79–104 |
    | `reviews/repo.rs` | Which git repository a review is in: `vcs`, `repository_here`, `repository_of_files` | 24–78 |
    | `reviews/doctor.rs` | `check()`: what `atm doctor` says about reviews and their tasks | 105–202 |
    | `reviews/naming.rs` | How a review names its tasks: `key_now`, `task_reference`, `names_task`, `list_naming`, `key_of_id`, `resolve`, `named` | 281–391 |
    | `reviews/linking.rs` | A review's tasks: which can be opened, moved or taken back (`openings`, `Opening`, `waiting_review`, `ask_review`, `link`, `kind_of`, the agent helpers) | 203–280, 392–526 |
    | `reviews/commands.rs` | The review commands: `create`, `add_round`, `edit_summary`, `submit` and `check_decision`, `close`, `set_tasks` | 527–809 |
    | `reviews/live.rs` | Live reviews: `working_now`, `checkpoint_now`, `subscribe_working_copy`, `live_approval_for`, `apply_live_approval` | 810–937 |

    The tests go beside what they test: `reviews/tests/` with one `fixtures.rs` (`dir`, `git`, `project`, `reopen`, `round`, `task`, `live`, `keyed`…) and a file per area (`opening.rs`, `decisions.rs`, `live.rs`, `submodules.rs`, `keys.rs`, `doctor.rs`). The largest code file would be about 290 lines (`commands.rs`).

    **Order, with ATM-633:**
    1. This split as a move only: no logic changed, the same tests passing, a review of its own.
    2. Then ATM-633 builds on it. `commands.rs` is exactly the part ATM-633 turns into thin callers of `lifecycle::apply` (§5.4), and `linking.rs`'s rules go into the task table. Splitting first keeps that change readable.

    Should I go ahead? And are there others you'd like split the same way? `review_pages.rs` and `review_listen.rs` are long too.
    - **Hakan Serce** · 2026-11-20T23:30:02Z · c7
      same here, no mod.rs, make sure you read the guides
      - **claude:0f56a916** · 2026-11-20T23:30:34Z · c8
        Yes, corrected in c6: `reviews.rs` stays the module file, with `reviews/repo.rs`, `doctor.rs`, `naming.rs`, `linking.rs`, `commands.rs` and `live.rs` beside it in `reviews/`. No `mod.rs` anywhere. Tests by the guide too: unit tests with their code, or `reviews/tests.rs` (then `reviews/tests/`); the real-repository ones in `crates/atm-lib/tests/`. ATM-711 carries the rules from the guide, so each file follows them.

## crates/atm-lib/src/review_pages.rs:683 · round 1 · t2
>         Some(new) => {

- [x] **Hakan Serce** · 2026-11-20T23:25:21Z · c2
  this file is also extremely long, tell me how you are going to split it
  - **claude:0f56a916** · 2026-11-20T23:28:22Z · c4
    It's 1,095 lines: about 720 of code and 370 of tests. The same way, a folder `review_pages/` with the public API unchanged (`review_pages::review_page`, `list_reviews`, `needs`, …):

    | File | What | Now at |
    |---|---|---|
    | `review_pages/mod.rs` | The re-exports; `me()`; the module doc | 1–21, 179–189 |
    | `review_pages/list.rs` | The reviews list: `ReviewQuery`, `status_named`, `fits`, `words`, `mentions`, `whole`, `list_reviews` | 22–55, 142–178, 190–242 |
    | `review_pages/waiting.rs` | What waits on whom: `needs`, `waiting_for`, `waiting_reviews`, `reviews_stamp` (the summary's and the badge's) | 56–141 |
    | `review_pages/patches.rs` | Reading a round's patch into files: `FilePatch`, `file_patches`, `one_file`, `paths_of_header`, `moved_submodule`, `with_links` (submodule commits) | 243–349, 363–397 |
    | `review_pages/view.rs` | One view of a review: `round_of`, `view`, `placed`, `with_placement` (where each chain sits), `shown_with`, `drafts_of` | 350–362, 398–546, 565–620 |
    | `review_pages/page.rs` | The page itself: `review_page`, `live_page_stamp`, and the live review's working copy as a round (`WithNow`) | 547–564, 621–724 |

    The tests split the same way (`list`, `waiting`, `patches`, `view`, `page`). The largest file would be about 230 lines (`view.rs`). It goes with `reviews.rs` as one move-only change: no logic changed, the same tests passing, its own review, before ATM-633's build.
    - **Hakan Serce** · 2026-11-20T23:28:50Z · c5
      are you even reading the guides!
      we are not using mod.rs convention at all
      - **claude:0f56a916** · 2026-11-20T23:29:21Z · c6
        You are right, and the Rust guide says so: `foo.rs` next to a `foo/` directory, never `foo/mod.rs`. Corrected: `reviews.rs` stays as the module file (the re-exports, `store()`, the module doc), and its parts go in `reviews/` beside it (`reviews/repo.rs`, `reviews/doctor.rs`, `reviews/naming.rs`, `reviews/linking.rs`, `reviews/commands.rs`, `reviews/live.rs`). The same for `review_pages.rs` with `review_pages/{list,waiting,patches,view,page}.rs`. For the tests, the guide too: unit tests stay with the code they test while it reads well, else `foo/tests.rs`, else `foo/tests/` by group. The tests that run on a real git repository and the real review store are integration tests, so they go to `crates/atm-lib/tests/`, one file per area. The plan for every large file is on CR-99 (c6).
