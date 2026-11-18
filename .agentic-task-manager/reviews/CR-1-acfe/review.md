+++
id = "01a1ec1e-490c-7662-b019-c555b41cacfe"
number = 1
title = "Large reviews: where the Diffs tab spends its time, and what to change"
status = "changes_requested"
tasks = ["ATM-587"]
author = "claude:0f56a916"
reviewers = ["Hakan Serce"]
created = 2026-11-18T00:46:10Z
updated = 2026-11-18T01:47:41Z
+++

Large reviews scroll smoothly: steps 1 to 4 built, measured on a test shaped like CompMD's CR-1 (94 files, ~3,300 changed lines). Before them, opening the Diffs tab was fast but scrolling got slower the further you went, because every file you passed stayed drawn (7,788 rows and 40,761 elements by the end, frames of up to 130 ms).

## What this round changes

1. **Files off screen aren't drawn:** each file's card has `content-visibility: auto`, so the browser lays out and paints only those near the view (`ReviewFile.svelte`).
2. **Far files give back their rows:** a file more than about four screens from the view drops its drawn diff and keeps its height. It is drawn again when you come back. A file with a comment being written in it, or with the focus in it, or the file you jumped to, is kept (`ReviewFile.svelte`, `FAR`).
3. **Long files are drawn in chunks:** 100 rows at once, the rest 100 at a time when the page is idle, with a placeholder of their height meanwhile (`DiffView.svelte`, `CHUNK`). Chunks of 300 still made a 120 ms frame when you scrolled into a 600-line file; 100 makes it about 30.
4. **A file at a revision is cached by the browser:** at a full commit or tree ID, atm's `documents?…&at=` answer is tagged and sent with `Cache-Control: private, max-age=31536000, immutable`, and the page lets the browser's cache answer. Reopening a review, or comparing rounds, fetches nothing again. A branch name, `HEAD`, a short ID or an error is never cached (`crates/atm-api/src/routes/documents.rs`, `dashboard/src/lib/documents.ts`).

## Measured (the same test, before → after)

- First file drawn: 83 → 65 ms.
- Scrolling top to bottom: 2.7 s → 1.4 s; the median frame 25 → 17 ms; the slowest frames 130, 130, 119 → 110, 30, 28 ms; long tasks 3 → none.
- Rows in the page at the end: 7,788 → 30 to 200.

The one ~100 ms frame left comes right after the tab opens, while the files near the first screen are made together.

## Budgets, now in the test

The first file and that first settling under 200 ms; every scroll step after the first three under **100 ms** rather than the 50 asked for; fewer than 1,500 rows. A step measures about 30 ms on its own but reached 97 while the whole e2e suite ran, so 50 would fail on a loaded machine. Twice each in CI.

## Not in this round

5 (highlight only what is shown): to decide after you try CR-1 in the app. 6 and 7 become backlog tasks.

The numbers in full: `docs/review-performance.md`, *Built, and measured again*.
