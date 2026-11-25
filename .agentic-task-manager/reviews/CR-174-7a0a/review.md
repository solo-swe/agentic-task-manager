+++
id = "01a20f40-d805-7731-acfc-4165503b7a0a"
number = 174
title = "App start: the hidden Analysis page asks atm for nothing"
status = "approved"
tasks = ["ATM-776"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T20:30:37Z
updated = 2026-11-24T22:21:09Z
+++

The app's start: show the active tab and load nothing for what isn't shown (ATM-776, from Hakan).

**What Hakan saw, and what it was:**
- **Tabs showing one at a time, jumping from one to another:** that was `cargo tauri dev`. There the window loads the page from Vite's dev server, as hundreds of unbundled modules each compiled on first request. In a release build Hakan saw no jumping.
- **A load latency at the start:** that is real, in release too. I measured it:
  - **atm's answers are quick:** for this repo, on `atm _serve` in a release build, the project list takes 0.01 s, the tasks (1.77 MB) 0.1 s, the stats 0.075 s and the reviews 0.06 s.
  - **The page:** with the app fixture and the 500-task project restored among four tabs, the window's frame is drawn at about 0.1 s and the first card at about 0.3 s, in Chromium and in WebKit.
  - **Hidden tabs:** nothing is loaded for them, and only the active tab is ever shown (the guard test from the first commit, `9dde789`).

**The waste found, and fixed (`ProgressTab.svelte`):** the Analysis page stays mounted while hidden. Its effect asked atm for the project's stats on every load and whenever the project's stats changed, so every start asked for them three times: once by the load and twice by a page nobody saw. For this repo each is 245 KB, about 75 ms of atm's time, carried over IPC and parsed. It now asks only while it is the view shown (`dashboard.tab === 'progress'`); shown, it asks as before.

**Test (`app.spec.ts`, the start's guard test):** besides no other tab's project being asked for, the active tab's own are asked once each: its tasks, claims, worktrees and stats. The Analysis page's own specs (`progress`, `burndown`) still get their stats when shown.

**Not changed:**
- The first title shows the project's ID until the project list names it (a few ms).
- A review's tab also loads its project's tasks, claims and worktrees, which the review page links to.
- Say if either is still felt.

`pnpm run check`, the unit suite (2121 passed), the functional e2e suite (630 passed).
