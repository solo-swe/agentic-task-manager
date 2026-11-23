## crates/atm-cli/src/commands/review.rs:55-67 · round 1 · t1
>     Open(open::Command),
>     // list, show, comments
>     #[command(flatten)]
>     Read(read::Command),
>     // comment, poke, draft, reply, edit, resolve
>     #[command(flatten)]
>     Talk(talk::Command),
>     // submit, close
>     #[command(flatten)]
>     Decide(decide::Command),
>     // wait, watch, inbox
>     #[command(flatten)]
>     Listen(listen::Command),

- [x] **Hakan Serce** · 2026-11-21T13:53:05Z · c1
  Having Command for each would be hard to consume for a reader although they are all namespaced. Can you change the names to OpenCommand, ReadCommand, etc.
  - **claude:0f56a916** · 2026-11-21T13:57:06Z · c2
    Done: `OpenCommand`, `ReadCommand`, `TalkCommand` (beside `DraftCommand`), `DecideCommand` and `ListenCommand`. Each module`s `run` takes its own, and `ReviewCommand` reads `Open(open::OpenCommand)`, … The command line is unchanged, and clippy and atm-cli tests pass. In round 2.
