+++
id = "01a1fb09-6ddc-7485-ac64-7cf5a1823a43"
number = 97
title = "Dashboard: only the latest load applies; a review page per review"
status = "approved"
tasks = ["ATM-675"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T22:17:41Z
updated = 2026-11-20T22:40:24Z
+++

The audit's D1 and D2, and its stale-guard abstraction.

- **`latest()`** (`dashboard/src/lib/latest.ts`): `ask()` gives a ticket; `ticket.current()` after each await says it's still the latest ask, and `current(key)` also that the key is still what's shown. `busy` says whether an ask is still out.
- **D1, the state:**
  - `fetchAndApply`, a task opened on the Omniboard, `refreshArchive` and `refreshProjectSources` each take a ticket.
  - A project's answer applies only while `workspace/project` is still shown, so the same ID in two workspaces is two projects.
  - The worktrees are assigned after that check, not before.
  - The timer calls `dashboard.poll()`, which waits while a load is still out. Focus, links and the app's events still reload, and the newest answer wins.
  - A task opened on the Omniboard shows its details from whichever load brings its data in (`showPeeked`).
- **The other guards** (ReviewsTab, OverviewReviews, DocumentViewer) use `latest()`. The DocumentViewer now also drops an older poll of the same file that comes in last. The audit said AllProjectsBoard, but the guard was in OverviewReviews.
- **D2:** `<ReviewPage>` is keyed by the review's ID. The new e2e test failed without the key.

Also a one-line security doc fix, committed separately under ATM-659: `rounds/N.json` read as a missing scenario file to the invariants test.

To check: `poll()` against `reload()`. Only the timer skips while a load is out.
