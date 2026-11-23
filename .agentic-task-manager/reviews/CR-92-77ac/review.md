+++
id = "01a1faba-9555-7497-9134-d051b68477ac"
number = 92
title = "Status, type and priority IDs and sprint names shell-safe; four sprint names reserved"
status = "approved"
tasks = ["ATM-665"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T20:51:34Z
updated = 2026-11-20T22:38:17Z
+++

Audit findings C3 and L13. Status, type and priority IDs, and sprint names, accepted shell characters, though agents paste them into commands (`atm task state … <status>`, `--sprint '<name>'`). And a sprint could be named `backlog`, `new`, `active` or `next`, after which `--to backlog` could never mean that sprint.

**Change**
- **`vocabulary.rs`, `terms()`:** status, type and priority IDs follow the rule releases and components already have (`valid_short_id`: letters, digits, `.`, `_`, `-`, at most 64), checked on load.
- **`vocabulary.rs`, `sprint_name_problem`:** the one sprint name rule, checked on load, on `sprint add` and on `sprint rename`. Names may have letters and digits in any script, spaces and `._-&,:()/+#`, at most 48 characters with no space at either end.
  - The set comes from the names this project has: `Task&review state machine`, `Code reviews: records and CLI`, `The atm app, continued`, and generated ones like `Nov 20-3:05pm`.
  - Refused: `$`, backtick, `;`, `|`, quotes, `!`, `<`, `>`, `\`, `*`, `?` and newlines. The error lists each refused character as `';'`, `'$'`, `` '`' ``.
- **`sprints.rs`, `usable_name`:** `sprint add` and `rename` also refuse the four reserved words, in any case. They're refused only where a sprint gets named, so a project that already has one still loads.
- **`docs/security-architecture.md`:** I8 says so.

**Tests**
- `security.json`:
  - a status `done$(x)` and a sprint `s1;x` are refused on load;
  - `sprint add s1;x` and `sprint add Backlog` are refused with exit 2 and write nothing.
- atm-lib's rename table: `s1;x`, `` a $(rm) `b` ``, `BACKLOG` and `next` are refused; this project's kinds of names, and `Café (v2) / #3+`, are taken.
- Changed: three tests created sprints called "New" or "Next". One of them, `keys.test.mjs`, even moved a task `--to Next`, which is exactly the ambiguity this removes.
- Passing:
  - this project's own `tasks.json` loads under the new rules (`atm doctor`);
  - atm-common's and atm-lib's tests;
  - the CLI suite;
  - the dashboard's 575 e2e tests, whose fixtures load through atm;
  - clippy.

**Worth a look**
- A project whose task file already has a status ID with a space, or a sprint named with a `'`, no longer loads until it's renamed. The error names the entry and what's allowed. I chose that over a migration because agents act on these values.
- This commit also has a `cargo fmt` reflow of one line in `review.rs`, from ATM-662.
