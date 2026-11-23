## (the review) · round 1 · t1
- [x] **Hakan Serce** · 2026-11-20T23:14:35Z · c1
  this is good. Does the hook approach only work for Claude Code? What about other harnesses? How does it work with them?
  - **claude:0f56a916** · 2026-11-20T23:15:19Z · c2
    The hook is Claude Code's only: `atm review inbox --hook` reads Claude Code's input and answers in its format. The other two parts work in any harness:

    - **The unread line (B)** is on stderr of every atm command an agent runs, in any harness. The skills tell the agent to answer it with `atm review inbox`. It depends on nothing but the agent running atm, which it does all the time in a loop.
    - **`--once` (A)** helps wherever the harness tells the agent when a background command ends. One that streams a command's output as it comes would work with or without it.

    For the hook itself, the others as far as I know (I haven't checked each one's current docs, so worth checking before building):

    | Harness | Hooks | What atm could do |
    |---|---|---|
    | Cursor | yes: hooks around shell commands, edits and stopping | the same as Claude Code's: context after a command, the stop held |
    | Gemini CLI | yes, Claude Code–style tool and session events | the same, its own input and answer format |
    | Codex CLI | a `notify` command at the end of a turn, as far as I know nothing that adds context | only a reminder at the end of a turn |
    | OpenCode | plugins (JavaScript), with events | a small plugin calling `atm review inbox --json` |
    | Copilot, Antigravity | none I know of | the unread line only |

    `--hook` could take the harness (`--hook claude|cursor|gemini`), and `atm agents hooks install --harness X` would write each one's settings the way `atm agents install` already writes each one's skills. The rules would be I35's for each harness.

    Should I make that a follow-up task, starting with checking each harness's current hook docs?
    - **Hakan Serce** · 2026-11-20T23:21:33Z · c3
      so what is the behavior when working with Claude vs say Antigravity, using atm?
      - **claude:0f56a916** · 2026-11-20T23:22:13Z · c4
        Say you comment on a review while the agent is in the middle of a task, a long test run say.

        **Claude Code, with the hook installed:**
        1. The agent's next tool call (any, even the test's next poll) ends, and the hook puts your comment in its context: `atm: 1 unread … CR-7 (ATM-12): comment c3 by Hakan …`. It sees it within seconds, without remembering anything.
        2. If it was about to end its reply, the Stop hook holds the reply until it has seen it.
        3. Its `--once` watch ends too, and Claude Code tells it so. Whichever comes first, it's read once: the inbox marks it read, and nothing tells it twice.

        **Claude Code, without the hook:** the `--once` watch ends and Claude Code tells the agent. If the watch wasn't running, the next atm command it runs ends with the unread line.

        **Antigravity:** no hook, as far as I know. What reaches the agent:
        1. **The unread line**, on stderr of the next atm command it runs. In a loop that's within a step or two (`atm claim hold`, `atm vcs commit`, `atm task state`…). While it runs only other tools (a long build, edits), it doesn't see it until its next atm command.
        2. **The watch**, if Antigravity tells an agent when a background command ends or shows its output as it comes. I don't know which it does; worth checking. If it does neither, the watch tells no one, and the unread line is all it gets.

        So the difference: with the hook, at the next tool call, guaranteed; without it, at the next atm command, which in a loop is soon but not guaranteed during a long non-atm stretch.

        If Antigravity has any hook or rule mechanism, atm could add it as `--harness antigravity` in the follow-up task I proposed (c2).
      - **claude:0f56a916** · 2026-11-20T23:22:22Z · c5
        A correction to step 3 above: the watch can tell it a second time. Reading the inbox (the hook, `atm review inbox`) moves only the inbox cursor; a running watch keeps its own. So with the hook installed, the agent may see a comment from the hook and again when its watch ends. That costs nothing but a repeat. If you want, reading the inbox could move the watch cursor too, so each comment is told once.

## crates/atm-lib/src/review_listen.rs:1238 · round 1 · t2
>         for (status, expected) in [

- [x] **Hakan Serce** · 2026-11-20T23:28:23Z · c6
  what is going on with these huge files, are we doing something deeply wrong here, or is it just the tests? In any case, split the file in logical modules/files, and make sure the files have good unit tests mocking dependencies. And move integration testst (test that doesn't mock much) into their own files. Not just ofr this file, check guides for how to decide.
  - **claude:0f56a916** · 2026-11-20T23:29:58Z · c7
    Both: mostly the tests, and two real problems under them. Measured, 13 Rust files are over about 1,000 lines:

    | File | Code | Tests |
    |---|---|---|
    | `atm-cli/src/commands/review.rs` | 1,386 | 100 |
    | `app/src-tauri/src/commands.rs` | 1,114 | 425 |
    | `atm-lib/src/review_listen.rs` | 974 | 538 |
    | `atm-lib/src/reviews.rs` | 937 | 1,300 |
    | `atm-vcs/src/lib.rs` | 771 | 642 |
    | `atm-lib/src/review_pages.rs` | 724 | 371 |
    | `atm-review/src/folder.rs` | 651 | 648 |
    | `atm-docs/src/lib.rs` | 535 | 836 |
    | `atm-review/src/rules.rs` | 529 | 1,162 |
    | `atm-review/src/files.rs` | 511 | 565 |
    | `atm-review/src/rounds.rs` | 455 | 742 |
    | `atm-api/src/routes/reviews.rs` | 391 | 778 |
    | `app/src-tauri/src/lib.rs` | 236 | 1,283 |

    What's wrong, against the Rust guide:
    1. **The tests stayed inline** where the guide says to move them out (`foo/tests.rs`, then `foo/tests/`) once they hurt reading the code. Most of these are more tests than code.
    2. **Many unit tests aren't unit tests.** In `reviews.rs`, `review_listen.rs` and `review_pages.rs` they set up a real git repository and the real review store. The guide puts those in the crate's `tests/`, and wants unit tests with fakes. atm-review already has a memory store to fake with.
    3. **Six files have too much code of their own** (from `commands/review.rs` at 1,386 down to `review_pages.rs` at 724). The review area grew a feature at a time, each added to the file that was there, and nothing split it.

    Nothing deeper, as far as I can see: the layers are as the guide says, and the review store sits behind a trait. The files just weren't split as they grew.

    **ATM-711** (in this sprint) is the plan. One file at a time, the reviews area first, before ATM-633's build changes those files. Each step:
    - its plan agreed first, as on CR-100;
    - a move only, with the same tests passing;
    - `foo.rs` beside `foo/` (no `mod.rs`);
    - fakes added where a unit test needed a real repository, that test kept as an integration test in `tests/`.
    - **Hakan Serce** · 2026-11-20T23:31:03Z · c8
      go ahead
