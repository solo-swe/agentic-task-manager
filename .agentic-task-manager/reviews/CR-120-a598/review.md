+++
id = "01a204fc-b912-7488-9ab8-d88ffdc1a598"
number = 120
title = "Unit tests: every file waits CompMD’s settling out before jsdom goes"
status = "approved"
tasks = ["ATM-709"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T20:40:01Z
updated = 2026-11-23T00:40:18Z
+++

The unit tests' unhandled rejection under load: CompMD's settling outliving a test file.

## What changed
`tests/unit/setup.ts`: an `afterAll` in every file waits out five steps of CompMD's settling loop (an animation frame then a task, as `@compmd/sandbox`'s `Settling` takes them) before the file ends, so a loop that was mid-step when the last test destroyed its view finishes while jsdom's `document` is still there.

## Why here, not upstream
Read from the published `@compmd/sandbox` (`dist/settling.js`): the loop runs a task, `drawn()`, its frames, images, `stylesheetsLoaded(document)`, `document.fonts.ready`, then a frame, and starts again on a change. The setup already stubs `document.fonts` (its `ready` resolved), so in jsdom the loop does end: it only needs the time to. Nothing calls `changed()` once a view is destroyed, so a loop then ends within a few steps. A `view.destroy()` that stopped the loop would be CompMD's to decide; it isn't needed for this, so I haven't filed anything upstream.

## Tests
The full unit run three times in a row on a loaded machine (load average about 11; the failure only ever showed under load): 131 files, 1898 tests each, no unhandled error. No test or error silenced.

## To look at
- Five steps is a margin over the loop's one pass; a CompMD that took more steps after destroy would bring it back, and the run would say so.
