+++
id = "01a204b6-4201-7221-b7d1-30860157e3c4"
number = 116
title = "Dashboard rules from atm: priority and stage from derived, graph focus by kind, Escape, the graph view saved once"
status = "approved"
tasks = ["ATM-680"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T19:23:03Z
updated = 2026-11-23T00:25:28Z
+++

The audit's dashboard-rules findings (D6, D11, D12, D13).

## What changed
- **D6:** `priorityOf` and `stageOf` read atm's `derived.priority` and `derived.stage`, the local rule only for a task atm hasn't served. Kinds already came from atm (`taskKind`, ATM-718); the `kindOf(vocab, status)` calls left look up a status ID's kind in atm's served vocabulary (a column's style, a status count), not a task's rule.
- **D12:** the graph's first focus (`graphFocus`, now in `graph/layout.ts`) is a matching task of kind `active`, else `todo`: never the status IDs `in_progress` and `todo`.
- **D11:** Escape closes the details panel only when it is open and nothing else took the key (`defaultPrevented`).
- **D13:** dragging the graph saves its view once, when the drag ends, not on every pointer move.

## Tests
`vocab.test.ts` and `stages.test.ts` (atm's answer wins, the rule without it), `layout.test.ts` (a project's own status IDs get a focus), `DetailPanel.component.test.ts` (Escape: handled, other keys, closed). A kanban test's fake gave every task P2; it now gives T-4 its P0, as atm would. Units 1876, e2e 592.

## To look at
- The fallbacks stay for tasks atm hasn't served (tests, an older server); the task asked for them.
