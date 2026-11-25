+++
id = "01a20bb8-4de9-72c3-a618-066784a65660"
number = 148
title = "The app's sidebar: a scope selector and the items"
status = "approved"
tasks = ["ATM-753"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T04:02:37Z
updated = 2026-11-24T16:34:33Z
+++

The app's sidebar (ATM-753, sidebar design §2): a scope selector and the scope's items, in place of the list of projects.

**The scope selector** (§2.1): at the sidebar's top, the scope's logo, name, what needs you and progress bar, and a Bits UI menu of *All projects* and the workspace's projects (missing ones dimmed and not chosen). It **follows the active tab's page**; a choice by hand holds until you switch tabs or the tab's page moves to another scope, and is kept in the preferences (`sidebarScope`; a project gone since is All).

**The items** (§2.2): six for a project, four for All, with their Lucide icons. Each is a real link (`<a href>`) to its page's first view, so the right-click link menu and Copy Link work. A click **always opens a new tab** right of the active one and shows it, even when a tab already shows that page; ⌘/Ctrl or middle click opens it in the background (⇧ brings it forward). The item the active tab shows is marked. Badges: Code Reviews, the reviews waiting on you (All: summed over its projects); Inbox gets its count with the inbox page (ATM-762).

**The title bar** loses the project chooser in the app; the webapp keeps it. **The page's item row** (ATM-752) is hidden in the app while the sidebar is open.

**Please note:**
- **With the sidebar hidden, the item row comes back in the app.** The design says the new tab's chooser (ATM-755) covers that case, but until it exists, hiding the row would leave no way to change item. ATM-755 can take the row away for good.
- **ATM-581's behaviour changes**, as the design says: from a review's tab, a project in the sidebar used to turn that tab into the project's page. Now its items open a new tab.
- **The link menu's Open** opens in the tab shown, as it does for every link; the design wanted the sidebar's Open to be a new tab. That's left as it is, since Open in New Tab is right beside it.
- The pure parts are in `lib/sidebar.ts`: an item's address, the scope rule, the preference.

**Tests:** `sidebar.test.ts` (every item's address per scope and workspace, the scope rule, the preference), `Sidebar.component.test.ts` (project and All scopes, following the tab, the kept choice and a switch ending it, a missing project, new tab and background, the mark, badges, resizing). E2E (`app.spec.ts`): the selector's menu and a hand choice, items opening new tabs and in the background, the scope following tabs, no chooser in the title bar, and from a review's tab another project in a new tab. The e2e helper `openView` follows a link in the app, where the item row is the sidebar's. Unit tests (1,999), the app spec and the functional suite pass.
