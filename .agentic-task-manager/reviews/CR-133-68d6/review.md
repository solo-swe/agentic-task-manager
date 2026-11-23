+++
id = "01a20918-e42b-77ba-8240-a5f656f068d6"
number = 133
title = "Claims and worktrees decided by atm, and a guard test against rules in the dashboard"
status = "approved"
tasks = ["ATM-724"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T15:49:15Z
updated = 2026-11-23T15:58:19Z
+++

Claims and worktrees decided by atm, and a guard test against rules in the dashboard (step 8 of docs/dashboard-derived-state.md, group H, §4.6).

## What changed
- **Claims:** `query::held_claims(project, now)` drops a claim whose lease has run out, by the loop's own rule (`leases::holder`). The claims route and the app's command serve only held claims, by atm's clock. The route's ETag includes which claims are held, so a lease running out is a new version though nothing was written. `parseClaims` no longer filters by the browser's clock.
- **Worktrees:** each linked copy carries `shown`, the IDs of the tasks the board shows from it. A copy wins when it got further than every other, by the merge driver's `kind_rank` (now public); a tie goes to the first listed, main first. A task only linked copies have comes from the first of them. `mergeWorktrees` follows `shown`; its `PROGRESS` table is gone.
- **The guard test** (`derived-rules.test.ts`, like `tokens.test.ts`): it fails when a file in `dashboard/src` reads `kindOf(`, `.review_state`, `.started_at`, `.completed_at`, `.review_requested_at`, `.blocked_at`, `meta.sprints` or `meta.statuses` outside comments, unless the file is on the list with its reason. It also fails on a listed file that no longer reads one. A sample shows it catches a rule and skips a comment.
- **Docs:** `docs/multi-project-api.md` §2.1 and §2.2, CHANGELOG.

## Tests
- `held_claims` at three moments, including the lease's own end.
- The claims route: a lease running out is a 200 though the store's stamp is unchanged.
- `serve.test.mjs`'s worktrees test checks `shown` at each step.
- The dashboard's `claims` and `worktrees` unit tests, rewritten for atm's answers; the Kanban spec's worktrees give `shown` as atm would.
- Clippy, `cargo test`, the CLI suite, the functional e2e.

## To look at
- **The ranking changed slightly.** The dashboard ranked review over active over blocked; the merge driver ranks the three alike (one ranking, as the task asked). A copy in review in one worktree and active in another now shows the first listed, not the one in review.
- The claims route now reads the claims on every poll, even for a 304, to know which are held. It's a small local file.
- The guard allows 18 files. Some only show a time or a kind's colour. Six still keep a rule to move (the burndown, the Activity timeline and time use, the Agents tab's insights, the forecast's keying), as ATM-722's review noted.
- One CLI-suite run failed a serve test once (a fetch to a server not up yet, in the malformed-task-file test). It passed alone and in a full rerun of the serve file.
