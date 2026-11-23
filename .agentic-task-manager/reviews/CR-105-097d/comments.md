## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-21T13:09:34Z · c1
  How many parts do you plan for this fix? Can you define it as sepate tasks and define a link from this to those, so we can follow the progress as we complete reviews.
  - **claude:0f56a916** · 2026-11-21T13:10:40Z · c2
    Four more parts, each now its own task, related to ATM-633 both ways, so you can follow them as the reviews close. This CR is part 1. **ATM-712**, part 2: every other entry point goes through lifecycle (create, set_tasks, the task commands, live approval, lapsed claims). **ATM-713**, part 3: the composed table test, with ATM-630, 631 and 632 as rows. **ATM-714**, part 4: the dashboard copies removed, so the page uses can only. **ATM-715**, part 5: the agent docs checked against the table by a test. 713 and 714 wait on 712, and 715 waits on 713. Each comes as its own CR.

## app/src-tauri/src/lib.rs:593 · round 1 · t2
>         let done = std::process::Command::new("git")

- [x] **Hakan Serce** · 2026-11-21T13:10:51Z · c3
  hmm. why are we using git directly? Don't we have a project VCS module which abstracts out what vcs we are using?
  - **claude:0f56a916** · 2026-11-21T13:13:46Z · c6
    Right, that test helper ran `git rev-parse` itself. Now `atm_lib::code::head_commit(root)` answers through the project VCS layer (atm-vcs `GitVcs::head`), and the app test uses it. The fixture still makes its repository with git commands (init, add, commit), as every crate test fixture does, since atm has no command that makes a repository. App tests pass. It goes in the next commit, with ATM-633.

## crates/atm-cli/src/commands/review/talk.rs:13 · round 1 · t3
> pub(super) fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {

- [x] **Hakan Serce** · 2026-11-21T13:12:39Z · c4
  This is confusing. This module is talk, but you have a function which can ```run(command)```
  - **claude:0f56a916** · 2026-11-21T13:13:46Z · c5
    Agreed, `run` says nothing about what it does. Fix, with ATM-711 (back in to do after your ok): no group `run`s. `review.rs` keeps the one `match` on the command, and each arm calls a function named after its operation, `talk::comment(…)`, `talk::poke(…)`, `talk::reply(…)`, `decide::submit(…)`, `open::create(…)`, taking only that command fields. Each module then holds the operations its name says, and there are no `unreachable!` fallbacks.
    - **Hakan Serce** · 2026-11-21T13:14:16Z · c7
      Maybe we should refactor ReviewCommand into a higher level ReviewCommunicationCommand
      - **claude:0f56a916** · 2026-11-21T13:14:57Z · c8
        Yes, that is better than my per-command functions. Each group gets its own enum, in its module: `talk::Command` (comment, poke, reply, edit, resolve, drafts), `open::Command`, `read::Command`, `decide::Command`, `listen::Command`. `ReviewCommand` becomes `Talk(talk::Command)`, `Open(open::Command)` and so on, each with clap `#[command(flatten)]`, so the CLI is unchanged (`atm review comment …` stays). Each module then has `run(command: talk::Command, …)`: typed, no `unreachable!`, and its match is only its own commands. I named them by module rather than `ReviewCommunicationCommand`, since the path already says `review::talk`. Say if you want the long names instead. I will do this with ATM-711.
        - **claude:0f56a916** · 2026-11-21T13:37:55Z · c9
          Done (ATM-711, its next commit). Each group has its `Command` enum in its module, flattened into `ReviewCommand` (`Open`, `Read`, `Talk`, `Decide`, `Listen`), so the command line is the same. Each `run` takes only its own enum, with no `unreachable!` left, and `DraftCommand` lives in `talk.rs`. main.rs checks `ReviewCommand::Listen(_)` for the watch reminder. One visible change: `atm review --help` lists the commands by group, so `close` comes before `wait`. clippy, atm-cli tests and the whole CLI suite pass.
