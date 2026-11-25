+++
id = "01a20fca-cf33-717c-9d08-d64534528333"
number = 177
title = "Review page: its task leads the heading; no back link in the app"
status = "approved"
tasks = ["ATM-779"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T23:01:19Z
updated = 2026-11-24T23:02:41Z
+++

From Hakan: in the app the review page's own back link repeats the title bar's Back; and the heading should name the review's task, without the project key again before the reference.

- **The heading** (`ReviewPage.svelte`): the review's tasks first, in brackets, each a link that opens it as the facts' task link does (⌘-click in a tab of its own): `[ATM-1] CR-15 My fancy CR`; several share one bracket, comma-separated. A task's key names the project, so the project key shows only for a review of no task. The tab's label stays `ATM - CR-15` (the sidebar design §3).
- **No back link in the app** (`ReviewsTab.svelte`): the page's back link only in the browser; the app's title bar has the tab's Back and Forward.

**Tests:** `ReviewPage.component.test.ts` (a task, several, none; the link's click and ⌘-click), `app.spec.ts` (no back link in the app; the title bar's Back returns to the list), `reviews.spec.ts` (the browser's review page names its task).
