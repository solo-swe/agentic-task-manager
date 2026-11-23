+++
id = "01a20632-7575-7462-a4ae-7729a5d7830f"
number = 125
title = "Review watch idle while it waits; it stops once its shell is gone"
status = "approved"
tasks = ["ATM-727"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T02:18:19Z
updated = 2026-11-23T09:47:44Z
+++

Review watches burned CPU while they waited: on Hakan's Mac, four watches used 8–12% of a core each for hours.

## Why
- **Every 5-second recheck looked in full,** even with nothing changed. A look reads and parses every review (124 here), reopens the project (tasks.json and the archive, 2 MB), and runs `git diff` on the working copy: close to a second of CPU per look.
- **A watch whose shell was stopped ran on, unread,** until its 8-hour timeout.

## Round 2 (Hakan, c1): no timed looks at all
A watch looks only when told something it reads changed, and each of those is now watched (`review_listen/watch.rs`, `Sources`):

| Watched | Why |
|---|---|
| The reviews folder | every comment and decision (as before) |
| The task file (only `tasks.json`; atm's lock files beside it don't count) | taking or finishing a task changes which comments are the agent's (ATM-663) |
| The working copy, only while a live review is open (the live reviews' filter, ATM-607: no `.git/` or ignored files such as `target/`) | an edited file makes a comment on a live review the agent's; without one, nothing there matters |

- **Missed events count as a change** (`atm-fs` `watch.rs`): a watcher error, or a rescan (the system's queue overflowed). Before this, they were dropped.
- **`atm review wait`** looks only when the reviews change too.
- **A watch still wakes every 5 s with nothing told** (`RECHECK`), but only to renew its lease, check its timeout, and stop once its shell is gone (round 1, Unix). It doesn't look.
- Round 1's stamp and its one-minute full look are gone. A comment that becomes the agent's on an edit is told at once now, not within a minute.

## Tests (`tests/scripts/reviews/watch-idle.test.mjs`)
- With nothing changed, a recheck doesn't look; a decision on its own review is heard within 4 s.
- A running watch tells a comment and a decision within 4 s of the agent taking the task, from the task file's notice.
- An edit in the working copy doesn't wake a watch with no live review; once one is open, it does.
- A watch whose shell ends stops (skipped on Windows).
- The workspace tests and the CLI suite pass.

## Measured (round 1, the debug build, this repository)
| | 14 s of waiting |
|---|---|
| Before | about 2.4 s of CPU |
| After | 0.02 s of CPU |
