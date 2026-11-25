+++
id = "01a2102e-cf1c-75e7-a932-b8b465bc0eea"
number = 183
title = "A missed poll tick is done once the load is over; e2e tests wait for CompMD's drawing"
status = "approved"
tasks = ["ATM-781"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-25T00:50:32Z
updated = 2026-11-25T01:00:06Z
+++

The e2e suite on one worker (`--workers 1`, a slow run) had failures the default runs never showed. Each one, and why:

**The two clock tests (`memory.spec.ts` the kanban tab's 60 refreshes, `refresh.spec.ts` the 304 test): a page fix.** A poll tick that finds a load still out is skipped (ATM-675), and since ATM-776/777 a load asks the project list last, after its data is shown. A test that saw the new data and advanced the fake clock by one interval hit a tick skipped while that list was still out, and no other tick came. The page now keeps the tick (`state.svelte.ts` `poll`, `#pollMissed`): once the loads are over it reloads, unless a load that started after the tick already did; at most one waits, however many ticks came. Slow answers still don't pile up, and a page on a slow machine no longer misses a whole interval. Unit tests: the existing one (a reload asked after the tick covers it), and a new one (three ticks while a load is out make one reload, after it).

**The rendered side's + tests (`reviews.spec.ts`): tests that measured before CompMD drew.** After *Show 2 unchanged lines* the side is drawn again; a test that measured a block then pointed at it pointed where it had been. A helper `drawn(…)` waits for every CompMD block in it to be `data-compmd="ready"` (a changed text goes back to `loading` first, so it waits for the new drawing); used by `openDiffs` (ATM-780's wait, moved into it) and after the three *Show 2 unchanged lines* that measure.

**`documents-page.spec.ts`: a selector that matched two elements.** `[data-view="recent"]` is the Recent button and, once loaded, the list; a fast run looked before the list came. `button[data-view="recent"]` now.

**Checked:** the whole suite on one worker, 634 passed (it had 3, then 2, failing), the check and the unit tests (2137). No timeout raised, no sleep added.

(The disk filled up during the runs: `cargo clean`, on Hakan's word, freed 172 GB of build output.)
