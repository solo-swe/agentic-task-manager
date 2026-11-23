+++
id = "01a1f0e3-ec22-7031-8cf3-6ab607fc6ad5"
number = 33
title = "Live reviews follow the working copy: the app's watch, the browser's 304s"
status = "approved"
tasks = ["ATM-607"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T23:00:31Z
updated = 2026-11-19T06:04:54Z
+++

Your ask on CR-8: a live review's page shouldn't ask for the working copy every 2 s.

## The app: told, not polling
- **The command:** `subscribe_working_copy` (a new app command, in the main window's capability) watches the project's folder recursively. The page asks again when a **file of the work** changes, and doesn't poll every 2 s.
- **What doesn't count:** `.git/`, atm's task folder, and files git ignores. A build writing thousands of files into `target/` or `node_modules/` costs **one** `git check-ignore --stdin` per settled burst, with paths on stdin, never as arguments. The check goes through atm-vcs's single git runner, so security invariant I24 still holds.
- **The layers:** `atm-fs` (a filtered tree watch), `atm-vcs` (`any_not_ignored`), `atm-lib` (`subscribe_working_copy`), the app command, and `watchWorkingCopy` in the dashboard.

## The browser: still a 2 s poll, but cheap
- **The ETag:** only a live review's page of its working copy now is tagged. It's made from:
  - the reviews' stamp;
  - the working copy's state (`working_state`: the commit checked out, `git status`, the changed files' sizes and times, the task folder left out);
  - who asks.
- **The poll:** the page sends `If-None-Match`. While nothing changed, it gets a **304 with no diff made**, and the page stays as it is.

## Tests
- **Rust:**
  - a filtered watch ignores `.git/`-only bursts;
  - `any_not_ignored` (ignored, tracked-but-matching, new files, outside a repository);
  - `working_state` (changes with the work only);
  - `subscribe_working_copy` (an ignored file, `.git/` and the task folder tell nothing; a real file does);
  - `live_page_stamp` (stable, changes with a write, not for a named round);
  - the route: 304 with no page made, a new tag after a change, a refused stamp's status.
- **Dashboard:** `watchWorkingCopy` subscribes and ends; `loadReview` sends its tag, a 304 is the kept page, and an untagged answer forgets the tag.
- **Not covered:** an e2e *in the app* (Playwright drives the browser build, not the app). The app's side is covered by the unit tests above.
- **Suites:** clippy, `cargo test`, the unit tests and the functional e2e pass, except "switching tabs" (Graph 225 ms against 200, under the full suite's load): that's ATM-629, untouched here.

## Also
- `docs/security-architecture.md` records the new command.
- `docs/multi-project-api.md` records the command and the ETag.
- A Prettier-only fix to ATM-631's CLI test rode along.
