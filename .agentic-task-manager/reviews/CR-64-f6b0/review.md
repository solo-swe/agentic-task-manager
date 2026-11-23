+++
id = "01a1f561-325f-748a-a5a5-04a5e2baf6b0"
number = 64
title = "The timed e2e tests one at a time"
status = "approved"
tasks = ["ATM-654"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T19:55:50Z
updated = 2026-11-20T09:48:48Z
+++

The timed e2e tests now run one at a time, so no timed test shares the machine with another.

## Change
- `playwright.config.ts`: the `timed` project has `workers: 1` (Playwright's per-project limit). The `svelte` project keeps its five workers, and CI's command is unchanged.
- `AGENTS.md`: the testing note on `@timed` says so (next to ATM-641's).

## Why
In ATM-652's run, the long review's first draw took 208 ms against its 200 ms budget. It wasn't ATM-652's change: without it, with the timed tests running beside each other, the first draw measured 443 ms. Alone, it is 50 to 100 ms.

## Measured: the full run (`--project svelte --project timed`) three times
All 528 passed each time, and every timed test was within its budget.

| | Run 1 | Run 2 | Run 3 | Median |
|---|---|---|---|---|
| Long review: first drawn | 53 | 70 | 121 | **70** ms |
| Long review: settled | 96 | 125 | 141 | 125 ms |
| Long review: slowest frame | 47 | 51 | 90 | 51 ms |
| Tab switch: list | 109 | 111 | 182 | 111 ms |
| Reload: graph | 80 | 61 | 111 | 80 ms |
| The whole run | 2.4 | 2.3 | 3.6 | 2.4 min |

The run was 1.7 to 1.9 min before, so one worker for the timed tests adds about half a minute. Run 3 was slower throughout, the functional tests too: the machine was busy.
