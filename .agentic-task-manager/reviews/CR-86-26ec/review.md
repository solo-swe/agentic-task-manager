+++
id = "01a1fa03-e7c6-7638-a273-c968df0026ec"
number = 86
title = "comments.md: a body or path cannot forge another comment"
status = "approved"
tasks = ["ATM-658"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T17:32:02Z
updated = 2026-11-20T22:33:56Z
+++

Audit findings R1 and R2: a comment could forge another comment in `comments.md`.

- **R1, the body:** a body line shaped like `- **Hakan Serce** · 2026-… · c9` was read back as a reply by Hakan. The review watch then told it to agents as a person's word.
- **R2, the path:** a newline in a comment's path could add a heading, or a resolved comment, of its own.

**Change** (`crates/atm-review`)
- **`files.rs`:** a body line that `read_item` would take for a comment's first line is written with a `\` mark after its indent, and read back without it. That covers any indent, with or without a box or poke mark, and lines already carrying marks, each of which gets one more. Every other line is written as given, so older files read the same; only a line that is a mark followed by a comment's shape reads differently.
  - Headings can't be forged from a body: body lines are always indented, and a heading starts at column 0.
- **`rules.rs` (`Reviews::comment`):** a comment's path must have no control character and must be one of its round's files; a renamed file's old path counts too. This one rule covers the CLI, the REST routes and the app.
- **`docs/security-architecture.md`:** a new invariant, **I31**, with its tests. I30 was already taken.

**Tests**
- New in atm-review:
  - a body with reply-, box-, poke-, mark- and heading-shaped lines round-trips as one comment's body, with no replies and no extra chain;
  - an older file's lines with `\` marks read as they were;
  - paths: one with a newline, a `\r` or a tab is refused, and so is a file outside the round; a renamed file's old and new paths and the whole review are accepted.
- The rules, folder and drafts test fixtures now list their round's files, which the new rule needs.
- Passing: `cargo test --workspace`, the CLI suite (the review tests, a submodule comment included), the dashboard's 79 review specs, and clippy.

**Worth a look**
- Refusing a file outside the round is the stricter half. The dashboard only offers the round's files, so nothing we have breaks, but a script commenting on a file the diff doesn't show now gets an error.
- Not covered: an author name or time with a newline in it, which would forge the same way. Those come from `ATM_AGENT` or git's user name rather than from comment text. ATM-660 (who is an agent) is the closest task; I can add the same check there.
