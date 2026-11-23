+++
id = "01a1fab2-bd94-7110-88e2-988e857f440c"
number = 91
title = "The --*-file options read no secret: one door, from any folder"
status = "approved"
tasks = ["ATM-662"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T20:43:00Z
updated = 2026-11-20T22:36:00Z
+++

Audit findings A2, C1 and C17's file part. `--description-file ~/.ssh/id_ed25519`, a pre-approved command, put a key into `tasks.json`, which `atm vcs commit` then commits. There was no size cap, and the seven call sites reported a missing file in three different ways.

This is your option (a): no folder rule, so agents' scratch files outside the project keep working, but no secret is read.

**Change**
- **atm-cli, `option_file::read_option_file(cwd, option, file)`:** the one door for `--description-file` (add, edit), `--json-file`, `--body-file`, `--summary-file` (create, round) and `--meta-file`. It refuses:
  - a key or environment file;
  - a file in `.ssh`, `.gnupg` or `.aws`;
  - all of the above judged after following symbolic links, so a link to a key is refused too;
  - anything but a regular file (`/dev/zero`, a FIFO, a folder);
  - anything over 1 MiB, also checked while reading;
  - anything that isn't UTF-8.

  Every refusal is one wording, `--<option> <file>: <why>`, with exit code 2 (a usage error), before anything is written.
- **atm-common, `secrets`:** the key-file and environment-file rules, moved from atm-docs so the CLI may use them under the layer rule. The document viewer uses the same rule as before.
- **`docs/security-architecture.md`:** a new invariant, I33.

**Tests**
- `tests/scripts/option-files.test.mjs`, which runs atm itself:
  - a file in the project and one in a scratch folder outside it are read;
  - each of these is refused with exit 2, `tasks.json` unchanged: a key in a fake `~/.ssh`, its `config`, a `.pem`, a `.env`, a link to the key, `/dev/zero`, a folder, a 2 MiB file and a missing file;
  - `task add --json-file`, `task add --description-file` and `db init --meta-file` refuse the same way.
- Unit tests: atm-cli's `option_file` (exactly 1 MiB is read, `.env.example` is read) and atm-common's `secrets`.
- **Changed tests:** a missing file was exit 1 with three wordings; it is now exit 2 with the one wording, in `refusals.test.mjs`, `wrong-uses.test.mjs`, `lifecycle.test.mjs` and atm-cli's `text_of` test.
- Passing: clippy, the affected crates' tests and the CLI suite.

**Worth a look**
- The exit code for a missing file changed from 1 to 2. Agents read exit codes, but no skill branches on this one.
