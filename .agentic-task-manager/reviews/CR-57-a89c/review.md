+++
id = "01a1f2f2-da6a-72b1-b6df-4a08b501a89c"
number = 57
title = "Timed e2e tests run on a machine of their own"
status = "approved"
tasks = ["ATM-641"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T08:36:04Z
updated = 2026-11-19T08:59:35Z
+++

The timed e2e tests no longer share the machine with heavy ones, so the long review's settle budget holds in a full run. The budget itself is unchanged.

## Cause
The settle step (a screen-sized scroll, then a frame) fits in 26 to 61 ms on its own. In a full run, two heavy tests often ran beside it:
- the WebKit real-review measurement, busy for seconds;
- the jump test on a review of 5,000 lines (ATM-646).

Each frame then waited on CPU the page didn't have, giving 104 to 110 ms against 100. The diff viewer does no extra work under load: in the failing runs it had one long task of about 50 ms, the same as when it passes. So the fix is in how the suite is scheduled, not in the viewer.

## Change
- **Three Playwright projects, one after another** (`playwright.config.ts`):
  1. `svelte`: every other test, at 5 workers as before;
  2. `timed`: the tests tagged `@timed`;
  3. `measure`: `real-review.spec.ts`, which runs only when given a real review.
- **Tags:** `@timed` on `reviews.spec.ts`'s *performance* describe and on `performance.spec.ts`'s three describes. AGENTS.md says to tag a new timed test.
- **The cost:** Playwright starts a project only once the one before it passed, so with a functional test failing, the timed ones don't run. That's said in the config.

## Found on the way: an address that kept its spelling
In one full run `keys.spec` failed: a link to the task already open, written `~ATM-10`, left `~ATM-10` in the address. The page only rewrote it when the link opened *another* task (`state.svelte.ts`, `onNavigate`).
- **Now** `openAddressed` runs on every address naming a task.
- **It rewrites only when the address names the task by something other than its key**, in place, keeping the history entry's state. Before, a rewrite could also move `task=` after `doc=` and drop the document viewer's depth, after which its Back closed the viewer. That's what `documents.spec`'s "a link inside a document" caught once the first fix was in.

## Tests
- **Unit** (`state.test.ts`): the task already open, addressed as `~ATM-2` and as its alias `ATM-9`, gets its key in the address. It fails without the fix.
- **Full functional e2e**, twice with the change: 526 passed each time. The long review's `afterSettled` was 69 and 59 ms. Before, it failed at 104 to 110 in full runs.
- **Unit (1,782) and checks** pass.
