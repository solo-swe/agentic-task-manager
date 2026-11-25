+++
id = "01a20e24-14b2-75b2-a5a6-8328b09e5046"
number = 158
title = "Flaky e2e tests: graph keys, the long review's nodes, fixed dates"
status = "approved"
tasks = ["ATM-742"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T15:19:35Z
updated = 2026-11-24T16:24:32Z
+++

The three intermittent e2e failures from the retrospective (ATM-742, ATM-726). Only test changes; nothing on the page changes.

**graph-zoom: the early key press.** The graph takes keys only once it is laid out in its panel (`DepGraph`'s `active`: loaded, not empty, sized). That is also when it sets its SVG's `viewBox`. Under load, `+` could arrive before that and be dropped, so auto-fit stayed on. The new `focusGraph(page)` (helpers) waits for the `viewBox` (the state, not a timeout), then focuses. Every key test in `graph-zoom.spec.ts` uses it, and so does the graph test in `tabs.spec.ts`.

**The long review's nodes after a retry.** The count was read after a fixed `waitForTimeout(1000)` and one forced garbage collection. The page lets go of cards and makes the ones near the view when it is idle. A loaded run (CI's retry, after the first attempt timed out) doesn't always get there within a second. And one collection can leave what the previous one only let go. Now the test waits until the page's nodes are the same over two idle periods in a row, as the page shows it, and collects until the count stops falling (at most 6 collections). The 30,000 bound is unchanged. I couldn't reproduce CI's 38,057 or the first attempt's 30 s timeout here: that needs CI. This removes the two timing assumptions that a slow run breaks. If it comes back in CI, a `--trace on` run of that job is the next step.

**Fixed dates.** I swept `tests/e2e/` for literal dates that a rule compares with now:
- the review list (only decided reviews older than 5 days fold): fixed by the earlier change (`RECENTLY`, `daysAgo`);
- the 17 November dates left in `overview-reviews.spec.ts` and `app.spec.ts`: on waiting reviews, which no rule ages;
- "… ago" labels, inactive projects and the board's last 24 hours: their specs fix the page's clock (`setFixedTime`), or build times relative to the run (`claims.spec.ts`).

Nothing else needed changing.

**Runs:**
- `graph-zoom.spec.ts`, `tabs.spec.ts` and `overview-reviews.spec.ts` with `--repeat-each 10`: 440 passed.
- The timed long-review test ten times in its project, after the whole suite (614 passed): 22,347 nodes alive every time (bound 30,000).
- `pnpm run check`.
