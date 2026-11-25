+++
id = "01a20b06-334a-74c9-b822-cf1fa366f6a1"
number = 140
title = "Memory growth: the app page grows with every reload in WebKit; findings and follow-ups"
status = "approved"
tasks = ["ATM-734"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T00:48:05Z
updated = 2026-11-24T02:47:01Z
+++

The analysis of atm's memory growth, as the task allows: findings recorded, with follow-ups proposed and a question for you. No fix yet, because the cause isn't confirmed.

**What I found** (`docs/memory-growth-analysis.md`):
- **The process that grows is the app's page**, the WebKit WebContent process: 427 MB after two hours, peaking at 769 MB, almost all of it WebKit's own heap and graphics memory. The app itself is about 105–123 MB, and the servers are 4–14 MB.
- **It grows only while files change**, about 15 MB a minute, and is flat while nothing changes (one-minute samples in §2). In the app, the page reloads on every change the app reports, so each reload leaves memory in WebKit. At an agent's pace that comes to gigabytes in a few hours.
- **The dashboard's own state doesn't leak in Chromium.** The new `memory.spec.ts` checks each task tab across 60 refreshes with new data, a review's Diffs read 160 times, and the document viewer opened 50 times. It forces a garbage collection and checks the JS heap, DOM nodes and listeners, and everything levels off. These tests now guard against the dashboard starting to leak itself.
- **A second, smaller finding:** each `atm claim hold` leaves an `atm _claim-watch` process alive until the calling shell exits, for up to two hours. An agent's harness keeps one shell for its whole session, so 13 of them, at 6 MB each, were alive after an hour.

**Please decide (§5):** which of the four follow-ups to create. They are a WebKit measuring harness, the fix it points to, claim watchers that end with their command, and fewer or coalesced reloads in the app. Also, the 4.5 GB you saw: was it the *Agentic Task Manager* process or its *Web Content* process, and which views were open?
