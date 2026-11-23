+++
id = "01a2084a-f285-77ca-9457-0f0ad041533b"
number = 130
title = "The dashboard: the ago and date formats in one place, one card state"
status = "approved"
tasks = ["ATM-683"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T12:04:19Z
updated = 2026-11-23T15:56:35Z
+++

The dashboard's repetition (audit §7), one piece per commit. No behaviour change: the unit tests and the functional e2e are the check.

## What changed
- **The "ago" formats** (`2211776`): `formatAgo` ("12 m ago", the cards' and claims') and `formatAgoOrDate` ("5 min ago", the review list's) in `format.ts`, out of `claims.ts` and `ReviewList.svelte`. They still read as before: the two are different on purpose, minutes against the largest unit. The review list's takes `now` now, so it is tested.
- **The full dates** (`8736932`): one `formatDateTime` for the bare `toLocaleString()` calls (the details panel's notes, claim, times and waits, the stage chip's merge, the project card's last done).
- **The cards' state** (`4052ba8`): `taskCardState(task, context)`. The sprint and its colour, the type style, stranded, the kind, the claim, the wait and the code review, worked out once for `TaskCard` and `KanbanCard`. A pure function of the task and four of the state's fields, unit-tested.

## Tests
`card-state.test.ts` (new), `format.test.ts` (`formatAgoOrDate`, every unit and its limits), the unit suite, `pnpm run check`, the functional e2e.

## To look at
Not done: each is a task of its own, larger than this one's other pieces and with more risk of a visible change.
- `request<T>` in `api/http.ts` for the six fetch wrappers, and `ipcFetch` honouring `init` (D14): in the app there are no ETags, so what "honouring" means needs a decision.
- Splitting `ReviewPage.svelte` (1,855 lines) and `state.svelte.ts` (1,146).
- Moving `app.css`'s component styles into their components, and `DetailPanel`'s inline styles into classes on tokens.
- The hand-made popovers, the project selector's listbox and the card's `div role=button` as Bits UI pieces or buttons.
- `<SprintBadge>` and `<TaskWaits>`, shared by the two cards' markup.
