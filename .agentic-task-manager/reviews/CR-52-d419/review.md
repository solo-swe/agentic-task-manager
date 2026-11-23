+++
id = "01a1f2a4-aa57-71ad-9752-cb131120d419"
number = 52
title = "The + follows the pointer at once on a long review in the app"
status = "approved"
tasks = ["ATM-644"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T07:10:40Z
updated = 2026-11-19T07:15:59Z
+++

The hover lag on a long review in the app (your CR-7 recording), fixed at its source.

## Cause
`DiffCode.svelte` re-measured a run whenever anything in it changed. Its `MutationObserver` watched the run's whole subtree, including its own gutter, so each step of the pointer re-measured the line under it, by adding or removing that line's + button. Each measure called `getComputedStyle` and `getBoundingClientRect`, forcing a restyle and layout of the *whole* page. In the app's WebKit, on a review half a million pixels tall, that's about a second each, so the forced restyles queued up behind the pointer.

## Change
- The observer watches only CompMD's element (`.compmd`), so only a change in what CompMD drew re-measures the run.
- `measure` reads computed style only when it needs it: wrapped lines, or code not drawn yet. Otherwise the line height comes from the drawn code's height over its lines, as before.

## Tests (`reviews.spec.ts`, performance)
- **The pointer sweep** (80 lines on Split's old side, then in Unified wrapped) now counts `getComputedStyle` calls once every run is drawn. Before: 187 and 116. Now: 0 and 0. Each step still takes two frames, with the + always on the line.
- **A new measured test for ATM-646:** a review shaped as CR-7 (15 files, one of 20,000 lines, ~500,000 px) and a jump to its last file. It logs the restyle triggers during the jump: computed-style reads fell from 96 to 32 with this change. Sheet assignments and pages opened (12 and 8) are ATM-646's next steps.
- **Suites:** the review specs pass except ATM-641's timing budget under load (100 against `< 100`; alone 27–44 ms; noted on ATM-641). The unit tests and the checks pass.

`watch:page` has already rebuilt your app's dashboard with this. Hovering on CR-7 after clicking the last file should keep up now. The jump itself still restyles for each CompMD page opened, which ATM-646 reduces and CompMD's task removes.
