+++
id = "01a20e86-693c-7542-bee0-3643f278b83c"
number = 166
title = "Inbox: the page"
status = "approved"
tasks = ["ATM-762"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T17:06:59Z
updated = 2026-11-24T17:36:07Z
+++

The Inbox item's page (ATM-762, [the inbox design §6](docs/inbox-design.md#6-the-page-inbox-item)), in the app and the webapp. It replaces the Inbox's "coming" placeholder.

**The page (`inbox/InboxPage.svelte`):**
- **The quick-add box** on top. Enter adds, ⇧Enter is a new line, and the focus stays in the box for the next note.
  - It is focused when the page opens, unless the focus is somewhere already. Without that rule, walking the item tabs with the arrow keys stopped at Inbox, which `tabs.spec.ts` caught.
  - In the app, its draft is kept on its tab across a switch (`tab-parked`). The page made at start-up has no tab yet, so it takes the first one it is shown on.
- **The filter** Open · Discarded · Injected · All, with their counts, and *archived*. Both are in the address (`?state=all&archived=1`), so a reload, a link and Back keep them.
- **Each item:** its ID, and its first line as its title (a click shows the rest, rendered with CompMD), when and who. Then by state:
  - open: Edit (in place) and Discard, which asks why in a dialog and needs a reason;
  - discarded: its reason and Restore;
  - injected: its tasks. This project's are links to their details over the page; another project's key is shown as text.
- A refusal shows atm's message. The page is asked again after each change, and every 10 s while in view (the triage changes it too).

**The client (`api/inbox.ts`):**
- `loadInbox` reads the route conditionally (`ETag`), dropping anything unexpected from the answer.
- `addItem`, `editItem`, `discardItem` and `restoreItem` send their change as the user: ATM-761's routes with the page's session token in the browser, the app's commands in the app.
- They share the review changes' `send`, which now takes an `area` (`reviews` by default, or `inbox`), rather than copying it.
- `ipc.ts` maps the inbox route to `project_inbox`.

**Also:**
- The sidebar's Inbox badge, and the new tab chooser's, is the summary's `inbox.open` (`projects.ts` `inboxOpen`).
- `dashboard.address`: the address as the last navigation left it, which the page reads its filter from. `$app/state` doesn't resolve in the unit tests, so this keeps the page testable.

**Tests:**
- `InboxPage.component.test.ts`:
  - the items and counts, each state's actions, the injected item's links;
  - the rest of the text on a click;
  - the box focused, Enter adds and empties it while ⇧Enter doesn't add;
  - a refusal shown with the text kept;
  - the filter from the address, a choice being a step there;
  - Discard needing a reason, and Restore;
  - Edit in place;
  - an empty inbox and one that can't be read.
- `inbox.test.ts`:
  - the route's and the page's address (defaults, unknown values) and the app's command for it;
  - the listing parsed, a 304, an error;
  - each change's route, method, body and token in the browser, and its command in the app;
  - a refusal, and no change sent without a token.
- `Sidebar.component.test.ts`: the Inbox badge.
- `inbox.spec.ts`, with the fixture now playing atm's inbox routes and commands (`helpers.ts`):
  - Enter adds an item, as the user with the session token;
  - Discard asks why, Discarded is kept over a reload, Restore, then Back;
  - All lists every state, and an injected item's task opens its details over the page;
  - Edit;
  - in the app, through the commands with no HTTP request, and the draft kept across a tab switch.
- `tabs.spec.ts`: the Inbox is its page.
- `pnpm run check`, the unit suite (2078 passed), the functional e2e suite (623 passed).
