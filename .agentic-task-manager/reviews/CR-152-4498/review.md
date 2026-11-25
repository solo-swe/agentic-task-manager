+++
id = "01a20c6c-dcd4-7505-9b7f-dd04569c4498"
number = 152
title = "App: a lease renewal doesn't reload the page"
status = "approved"
tasks = ["ATM-767"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T07:19:50Z
updated = 2026-11-24T16:38:06Z
+++

The app stops reloading its page for a lease renewal (ATM-767, from the memory analysis §3b, CR-140): the biggest of the three reload costs.

**The change (atm-lib, `shown.rs`):** the app's subscriptions (`Member::subscribe` and each member in `Workspace::subscribe`) are told of a project's change only when what the page shows of it changed. That is the store's stamp with the claims store read for its holders, each claim without `lease_expires_at` and `renewed_at`. Every atm command an agent runs renews its lease and rewrites the claims store; that now reloads nothing. A new holder, a released claim, a task changed or the inbox changed still reload. A notice repeated for the same change also reloads once.

**Not done here (please decide):**
- **Partial reloads** ("the page reloads only the part that changed"): the store's change callback takes no argument, so the notice would have to name the part all the way down (atm-common's `OnChange`, atm-fs's watcher, atm-db, atm-lib, the app's `Changed`), with the page reloading parts. A lease renewal was most of the reloads (§3b), and ATM-768 makes an unchanged part cost nothing on a reload. I'd do partial reloads only if the measurements after ATM-768 still show a cost. Want a task?
- **The browser** already polls with `ETag`s, so an unchanged part is a 304. A renewal there changes only the claims route's answer, which is small.

**Tests:**
- `shown::tests`: the claims without their leases; a renewal tells nothing, a new holder and a task change tell once each, a repeated notice once; a project whose files appear is a change.
- `workspaces::tests`: through the real folder watcher, a member's subscription isn't told of a renewal and is told of a new holder.
- clippy, `cargo test`.
