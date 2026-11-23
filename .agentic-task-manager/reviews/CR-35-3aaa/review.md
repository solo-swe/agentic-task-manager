+++
id = "01a1f104-fed5-7338-a32f-0cfdf7863aaa"
number = 35
title = "Diffs tab memory bounded: the leak fixed, content-visibility dropped, a bound in the spec"
status = "approved"
tasks = ["ATM-620"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T23:36:38Z
updated = 2026-11-19T06:05:02Z
+++

These are the options you chose from `docs/diff-viewer-profile.md`: 1, 2 and 3. The doc now ends with a **Built** section with the numbers before and after.

## Change
- **1, the render after destroy:**
  - in `render-turns.ts`, a cancelled turn is marked, so a turn already picked to go next but cancelled before the page was idle never renders, and the next one goes;
  - `CompMd.svelte` never draws once destroyed. This also covers a text whose CompMD load finished after it was gone.
- **2, `content-visibility` dropped** from `ReviewFile.svelte`'s cards, with `contain-intrinsic-size`. A loading card's `min-height` at its estimate stays (ATM-615).
- **3, the memory bound:** the long-review spec goes back to the top after reaching the end, forces a garbage collection, and requires fewer than **30,000 DOM nodes alive**.

## Measured on the spec's 94-file review (Chromium)

| | Nodes alive | First line on opening | Slowest step after settling |
| --- | --- | --- | --- |
| before | 93,240 | 62 ms | 30 ms |
| after | 21,840 | 45–48 ms | 23–31 ms |
| before, CPU 4× slower (10 runs) | 71,449 | 222–270 ms | 62–66 ms |
| after, CPU 4× slower (10 runs) | 20,407–21,840 | 146–211 ms | 54–82 ms |

- **The bound** catches the leak (71,000–93,000 nodes) with room above the fixed count.
- **Not met: 10 runs in a row with the CPU 4× slower.** One of the ten runs opened in 211 ms, over its 200 ms budget. Before these fixes that budget failed in most runs, so this is better, but the task's condition isn't met. It's noted on ATM-629 (budgets under load), and nothing is raised.

## Tests
- **Unit:** a turn cancelled after it was picked never runs.
- **Component:**
  - `CompMd` gone after its turn was given never renders, and the queue moves on;
  - gone before CompMD loaded, it never renders.
- **E2e:** the memory bound.
- **Suites:** the full functional suite passes.

## Not here
- **4 and 5:** the CompMD tasks you're filing.
- **6:** measured next, as you said.
