+++
id = "01a1f150-2d3f-7405-a376-099d7407912a"
number = 42
title = "The Graph draws labels only where they can be read"
status = "approved"
tasks = ["ATM-629"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:58:45Z
updated = 2026-11-19T06:09:49Z
+++

The Graph draws a node's labels only where they can be read, as you chose (b).

## Cause
With 500 tasks, switching to the Graph took 87–94 ms alone, but 204–247 ms under the full suite, against the 200 ms budget. A trace showed about 85 ms spent laying out the 500 nodes, half of it shaping their SVG text (two labels and two emoji a node).

## Change
- **`DepGraph.svelte`:** below 50% zoom, a node is drawn without its two labels. At that zoom the 12 and 10 px text is under 6 px on screen. An unlabelled node keeps a `<title>` (its key and name), which costs no layout, so hovering still names it. At 50% and up, the labels are drawn as before.
- **The 500-task graph** opens fitted at 10%, so its text is no longer shaped until you zoom in.
- **`performance.spec.ts`:** the switch's comment says what it now measures, and a new test checks it: 500 tasks fitted at 10% have no labels; at *Actual size* all 500 do.

## Measured
- **Graph switch, 500 tasks, CPU slowed 2×:** 110–143 ms over 10 runs, all passing; it was 144–199 ms.
- **The full functional run:** 518 of 518 pass, the first fully green run in four. The Graph switch measured 110 ms there.
- **Still close: the long review's `afterSettled`** came in at 98 ms against its 100 ms budget in that run. It was the other timing noted on this task, and it is untouched here; it may fail again under load.

## Tests
The graph specs and the performance specs pass, and so does the full functional e2e run.
