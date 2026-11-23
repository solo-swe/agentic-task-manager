## docs/task-review-state-machine.md:150-151 · round 1 · t1
> A live review runs beside a task that is still in progress. It has states of its own, and its approval reaches the task only at the end:

- [x] **Hakan Serce** · 2026-11-20T22:54:54Z · c1
  Tell me more about this checkpoitn recording. Say I am reviewing live changes covering 30 files at this point, and just put in a comment on 1 of those files, will that capture that as a checkpoint round, and I will be able to see diff before and after the comment? And say I ended up entering 10 comments, will I end up with 10 rounds?
  - **claude:0f56a916** · 2026-11-20T22:56:09Z · c2
    Checked against the code (`checkpoint_now` and `working_now` in `crates/atm-lib/src/reviews.rs`, and its test `a_live_reviews_page_follows_the_working_copy_and_a_comment_makes_it_a_checkpoint`):

    - **A checkpoint is the whole working copy, not the file you commented on.** Before your comment is stored, atm takes the working tree against the review's base, all 30 files, as you saw it. It records that as a round only if it differs from the last checkpoint. Your comment is anchored to that round.
    - **Before and after:** yes. Each checkpoint is the full change from the base, and the page compares any two rounds (`?round=3&against=2`). Once the agent has changed something after your comment, the next checkpoint against yours shows exactly what changed since you commented.
    - **10 comments don't make 10 rounds.** A new round is recorded only when the working copy changed since the last one. 10 comments with nothing changed in between are one checkpoint, or none if nothing changed since the last round. You get 10 rounds only if the agent changed files between each of your comments; then each round is what you were looking at when you wrote that comment.

    Should I add this to §3.5? It only says "a checkpoint round is recorded", which reads as one per comment.
    - **Hakan Serce** · 2026-11-20T23:00:37Z · c5
      This is fine. Last I remember I was trying to review between latest and round 2, and it was telling me there is no round 3 in the review. Double check this specific bug, but also make sure we test this thoroughly. Or better, just use a solid state machine crate here to model. Do a research of existing rust state machine crates, and also include classic Rust enum/match based state machine approach (current?), and give me examples of which one model our complext states better, in a more maintainable, error-free way.
      - **claude:0f56a916** · 2026-11-20T23:04:12Z · c6
        **The bug: confirmed and fixed (ATM-710, CR-100).** A live review shows its changed working copy as the round after the stored ones, but only when no round is named. Comparing against round 2, the page asked for `round=3&against=2`, and round 3 isn't stored, hence "no round 3". Now that round means the working copy. A test on a real working copy reproduced your error before the fix; it covers asked by number, against an earlier round, unchanged, and past the end.

        **State machine crates.** Surveyed (crates.io and GitHub; versions and dates approximate where not checked):

        | Crate | Last release | Guard refuses with a reason | A pure "can I?" | Notes |
        |---|---|---|---|---|
        | statig | 0.4.1, 2025-07 | no (the handler returns Handled, Super or Transition) | no (clone and try) | hierarchical, most used |
        | smlang | 0.8.0, 2024-08 | yes (`custom_error`) | no | DSL, slow upkeep |
        | rust-fsm | 0.8.0, 2025-07 | no guards | partly | states carry no data |
        | finny | 0.3.0, 2026-09 | bool guards | no | heavy, few users |
        | statum | 0.9.0, 2026-06 | rebuild checks only | graph edges only | typestate, changes fast |
        | sm, typestate, machine, sfsm | 2019–2022 | — | — | unmaintained or compile-time only |

        Typestate crates don't fit: events come at run time from the CLI, REST and IPC, and the state comes from JSON files.

        **smlang:**
        ```rust
        statemachine! { name: Review, custom_error: true, transitions: {
            *Waiting + Approve(Actor) [may_approve] = Approved,
            Live + Approve(Actor) [may_approve] = Approved,
            ChangesRequested + Approve(Actor) [may_approve] = Approved,
        }}
        // may_approve(&self, by) -> Result<(), Refusal>. "Not in this state" is the macro's generic error,
        // and there's no dry run: process_event mutates.
        ```

        **Enums and match** (the shape `rules.rs` already has):
        ```rust
        pub fn check_review(r: &ReviewView, ev: &ReviewEvent, by: &Actor) -> Result<Status, Refusal> {
            match (r.status, ev) {
                (Approved | Closed, _) => Err(Refusal::ended(r)),
                (Waiting | Live | ChangesRequested, Submit(Verdict::Approve)) => {
                    ensure(decides(&r.header, by), || Refusal::not_a_decider(&r.header))?;
                    ensure(r.open.is_empty(), || Refusal::open_comments(&r.open))?;
                    Ok(Approved)
                }
                (Waiting, Submit(Verdict::RequestChanges)) => { /* decider */ Ok(ChangesRequested) }
                (Draft, AskForReview) => Ok(Waiting),
                // … every pair written out; no catch-all, so the compiler flags a new status or event
            }
        }
        // lifecycle::check = check_review + check_task for each named task → one Outcome; apply writes only if all Ok.
        ```

        **My recommendation: enums and match, with the tables as data and a test per row.**
        - **The refusal is the product.** Its words and exit code go to the CLI, REST's `can.why` and the UI. Only smlang lets a guard carry a reason, and even there "not in this state" is generic.
        - **A pure `check` is the main way in.** The `can` map, `check_submit` and dry runs all need one, and no crate gives a dry run with guards.
        - **One event moves a review and N tasks**, all or nothing. The crates model one machine; composing two would take more adapter code than the tables.
        - **The state is files, read per command.** The crates assume a machine that stays in memory.
        - **Exhaustive `match` with no catch-all:** a new status or event fails the build until handled. No dependency for `cargo deny`, and `rules.rs` and `state.rs` already have this shape.

        **For tests, as you asked:**
        - a `const` table of (state, event, actor, expected) with one test per row;
        - a test that walks every (status × event) pair against it, so the code, the table and §3's diagrams can't drift;
        - guard order tested, since the UI shows the first refusal.

        The costs: no diagram drawn from code, and §5.3's two-file write order is ours to keep either way.

        Should I put this in the doc as §5.4, with the draft API from my earlier reply?
        - **Hakan Serce** · 2026-11-20T23:07:44Z · c7
          ok
          - **claude:0f56a916** · 2026-11-20T23:09:04Z · c8
            Done: §5.4 in round 2. It covers no state-machine crate and why (the survey as a table), the draft types for atm-review, atm-lib state.rs and the new lifecycle.rs, and the tests that keep the code, the tables and the diagrams from drifting.

## docs/task-review-state-machine.md:282 · round 1 · t2
> **The review table stays in atm-review.** atm-review is a crate that can stand alone (§3.1 of the review design: a

- [x] **Hakan Serce** · 2026-11-20T22:56:53Z · c3
  I need more clarification here, just give me draft classes, traits methods here in the comment, and tell me which lives in atm-review, atm-lib, tasks, lifecycle, etc?
  - **claude:0f56a916** · 2026-11-20T22:58:58Z · c4
    A draft, using today's names where they exist. Nothing here is written yet.

    **atm-review: the review table, pure.** `rules.rs` keeps it, and no other crate learns it.

    ```rust
    // atm-review/src/rules.rs
    pub enum ReviewEvent<'a> {
        AskForReview,                      // draft → waiting
        Round,                             // waiting/changes requested → waiting; live: a checkpoint
        EndLive,                           // live → waiting (the author's round)
        Submit { decision: Verdict, open_comments: usize, drafts: usize },
        Close { reason: &'a str },
        SetTasks,
    }

    pub struct ReviewRefusal { pub why: String, pub code: u8 }   // 2 usage, 4 conflict

    /// The review's next status, or why not. Reads nothing, writes nothing.
    pub fn review_next(header: &ReviewHeader, event: &ReviewEvent, by: &Actor)
        -> Result<Status, ReviewRefusal>;

    impl<S: ReviewStore + ?Sized> Reviews<'_, S> {
        // add_round, submit, close, set_tasks… stay, each calling review_next first.
        // check_submit is replaced by review_next.
    }
    ```

    **atm-lib `state.rs`: the task table, pure.** `Command`, `Kind` and the guards it has today.

    ```rust
    // atm-lib/src/state.rs
    pub fn task_next(task: &Task, vocab: &Vocabulary, command: Command, claim: Option<&Claim>, by: &str)
        -> Result<Kind, TaskRefusal>;       // the guards change_status checks today, without writing
    ```

    **atm-lib `lifecycle.rs` (new): the composition and the only place a transition is applied.**

    ```rust
    // atm-lib/src/lifecycle.rs
    pub struct State {                       // what check reads, loaded once
        pub tasks: Vec<Task>,                 // the event's tasks, keyed now (review_keys::task_of)
        pub claims: Vec<Claim>,
        pub review: Option<ReviewHeader>,
    }

    pub enum Event {
        Task { task: String, command: state::Command },             // atm task state, task review …
        Review { review: ReviewRef, event: ReviewEventOwned },      // atm review …
        ClaimExpired { task: String },                              // drop_expired_claims
    }

    pub struct Outcome {                      // every write, decided before any is made
        pub review: Option<Status>,
        pub tasks: Vec<(String, Kind, Vec<Note>)>,
        pub claims: Vec<ClaimChange>,         // take, release
    }

    pub struct Refusal { pub why: String, pub code: u8 }

    /// The machine: the task table × the review table. Pure: the dashboard's `can` is this.
    pub fn check(state: &State, event: &Event, by: &str) -> Result<Outcome, Refusal>;

    /// Load, check, then write in §5.3's order: the review, then tasks.json under its lock.
    pub fn apply(project: &mut Project, event: Event, by: &str) -> Result<Outcome>;

    /// What each action would do for this reader, for `GET …/reviews/<CR>`'s `can` (§5.2).
    pub fn can(project: &Project, review: &ReviewRef, by: &str) -> BTreeMap<&'static str, Result<(), Refusal>>;
    ```

    **Callers.**
    - `changes::change_task_status`, `reviews::{create, add_round, submit, close, set_tasks}`, `apply_live_approval` and `leases::drop_expired_claims` become thin wrappers: build an `Event`, call `lifecycle::apply`.
    - atm-cli, atm-api and the app call only those.
    - atm-api's review route adds `can` from `lifecycle::can`.
    - The dashboard reads `can`, and loses `approveBlocked`, `REWORK_STATES` and the open-status lists.

    **Tests.**
    - The two tables are unit-tested alone, as tables.
    - `check` is tested on built `State`s, with every row of §4's tables.
    - `apply`'s ordering is tested with a fake store behind a trait: a write that fails after the review is written leaves what §5.3 says.

    Should I put this in the doc as §5.4?
