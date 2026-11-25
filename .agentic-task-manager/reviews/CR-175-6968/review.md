+++
id = "01a20fc6-8f23-7511-9276-f526de3b6968"
number = 175
title = "App: the page shows no item row"
status = "approved"
tasks = ["ATM-778"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T22:56:40Z
updated = 2026-11-24T23:01:48Z
+++

In the app a page showed the webapp's full row of item tabs (Overview, Inbox, Tasks, …) whenever the sidebar was hidden. That was a stopgap from ATM-753, kept "until ATM-755"; the new tab's chooser (ATM-755) and the hidden sidebar's rail (ATM-771) have landed since.

Now the item row is the webapp's only (the sidebar design §2.3, §5). In the app a page shows only its sub-tabs: Tasks (Board · List · Dependencies) and Analysis (Progress · Agents); Overview, Inbox, Code Reviews and Documents none.

**To check:** `ViewPanel.svelte` and `ProjectsOverview.svelte` (the condition is `!inApp`), and the rail test in `app.spec.ts`: with the sidebar hidden, no `.item-tabs`, Tasks' sub-tabs still there.
