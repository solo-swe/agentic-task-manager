+++
id = "01a20462-16cc-718a-b16a-f1286587398c"
number = 115
title = "CLI output: invisible characters dropped, --json everywhere, --started-by, claim hold refuses, one labeller"
status = "approved"
tasks = ["ATM-679"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T17:51:06Z
updated = 2026-11-23T00:24:51Z
+++

The audit's CLI output findings (C8, C9, C11, C12, C13).

## What changed
- **C9:** `output::clean` drops the invisible format characters (bidi embeddings, overrides and isolates, zero-width characters, the soft hyphen, the BOM), by one list, `atm_common::py::invisible_format`, which `repr` quoting shares (it gained the isolates U+2066–2069, as Python's `isprintable` refuses them). `--json` keeps text as it is.
- **C11:** `service start --json` is `{url, started, listed, migrated}`, `service stop --json` `{stopped: [{pid, port, recorded}], stale_record}`, `service --refresh --json` `{restarted, version, url}`; `db init --json` gains `meta` (`{created, keys}`, null without a meta file); `vcs commit --record --json` always has `summary` and `hint`.
- **C13:** `task list --agent` is `--started-by` (`--agent` a hidden alias). Whether a command runs as someone else is decided from the parsed command: `task list`'s `--agent` is a filter, so it renews its caller's claims and reminds them to watch.
- **C12:** `claim hold` refuses, saying to use `atm claim heartbeat`, when it can't read the shell's start time (its watcher would quit at once).
- **C8:** one `Labeller` a process for the lines with task UUIDs, opened again when the task file changed since it read it, or when a line names a task it doesn't know.

## Tests
`output.rs`'s `clean` table; `py.rs`'s repr of the isolates; `output.test.mjs` (a bidi name in list and details, kept in `--json`; `db init --meta-file --json`); `serve.test.mjs` (start and stop `--json`); `git.test.mjs` (`--record --json`); `claims.test.mjs` (`task list --agent` renews the caller's claim); `query.json` (`--started-by`).

## To look at
- C12 isn't tested: a real shell's start time can't be made unreadable in a test.
- C8's staleness is the task file's size and modification time: two writes of the same size within the file system's time resolution would look the same, and a missed UUID still reopens it.
