+++
id = "01a204ca-d1a1-7111-966a-5efe960b20a7"
number = 118
title = "Pop-ups by kind: a question left open stays on its tab"
status = "approved"
tasks = ["ATM-700"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T19:45:30Z
updated = 2026-11-23T00:38:20Z
+++

Pop-ups by kind (docs/tabs-and-navigation-design.md §4.2, §6.2's recommended answer).

## What changed
- **`lib/tab-parked.ts`:** what a tab keeps of a question left open, by tab and key, taken back once; nothing without a tab (the browser); a closed tab's goes with it (`tab-actions` close).
- **`ReviewPage.svelte`:** its Submit review and Poke forms, left open or half written, are parked for the app tab it is shown on when a switch takes the page away, and taken back when that tab shows it again. New prop `appTab`.
- **`ReviewsTab.svelte`:** the page is keyed by the tab and the review, so another tab on the same review is its own page, with its own forms.
- Already so, unchanged: the app's own pop-ups (Settings, Help, What's new, About, Connect your agents) are the dashboard's, above every tab, open across a switch; task details and documents are in the tab's address, so they are there again with it; a menu or dropdown on a page closes as the page goes.

## Tests
`tab-parked.test.ts`; `ReviewPage.component.test.ts` (back on its tab, not on another, not in the browser); `app.spec.ts` (a half-written Submit review on one tab, another tab and back, the same review in a third tab with no form, Settings open across a switch). Units 1898, e2e 594.

## To look at
- The dashboard has no Add task dialog or confirmation today: the review page's two forms are the questions there are. A comment or reply being written in a composer isn't parked; its drafts are kept by atm already when the drafts flow is used.
