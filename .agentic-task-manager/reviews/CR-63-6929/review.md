+++
id = "01a1f518-c839-743d-ae42-916676fb6929"
number = 63
title = "Tabs, pop-ups, Back and Forward, and an address for every page: the design"
status = "approved"
tasks = ["ATM-653"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T18:36:44Z
updated = 2026-11-20T11:35:50Z
+++

The design for tabs, pop-ups, Back and Forward, and an address for every page. It is for agreeing before anything is built.

## Why Back lands on the Reviews list (§3)
- **All the app's tabs share one browser history.** Switching tabs adds an entry, so Back walks across tabs and shows another tab's page in this one.
- **The review page's ← Reviews isn't Back.** It is a link to the project's Reviews list.
- **A plain click on a review opens a new tab** in the app, but opens in place in the browser.

§3 maps all of today's behaviour from the code, mismatches included:
- closing task details leaves Back doing nothing;
- right-click shows WebKit's own menu, whose Reload bypasses the app's reload;
- the desktop design doc describes a tab menu and shortcuts that don't exist.

## The model (§4), as the browsers, Linear and Notion have it (§2, sourced)
- **A tab is a place with its own history.** Back and Forward walk only that tab's history; switching tabs adds nothing.
- **Pop-ups come in three kinds:**
  - places over a page (task details, a document): in the address and the history;
  - questions (forms, Submit review): belong to their tab and stay on it while you switch, as macOS sheets do;
  - the app's own (Settings, Help): above every tab.
- **The page's back button is Back,** labelled with where it goes (← Board, ← ATM-633). It goes up to the Reviews list only when there is nowhere to go back to.
- **An address for everything a tab shows:** a review's sub-tab, a comment, the list's filter.
- **Clicks as in a browser:** a click opens in place, ⌘-click or a middle click opens a new tab, and atm has its own right-click menus.

**§5** lists the changes by file, **§6 has four questions for you**, and **§7** the build tasks it implies.
