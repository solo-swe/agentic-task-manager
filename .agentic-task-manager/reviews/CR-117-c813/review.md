+++
id = "01a204c2-0a3d-74c3-9dcd-78181ba0c813"
number = 117
title = "atm’s own right-click menus in the app: links, tabs, elsewhere"
status = "approved"
tasks = ["ATM-699"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T19:35:55Z
updated = 2026-11-23T00:25:45Z
+++

atm's own right-click menus in the app (docs/tabs-and-navigation-design.md §4.5).

## What changed
- **`lib/context-menus.ts`:** what a right-click is on (`menuTargetAt`: text, a title-bar tab, a link/card/row by the address `newTabAddress` gives, else the page) and each menu's items (`menuItems`), on the window's tabs and page actions passed in.
  - A link, card or row: Open, Open in New Tab (in the background, as ⌘-click), Open in New Window, Copy Link (`atm://`).
  - A title-bar tab: New Tab, Duplicate, Move to New Window (closed here once opened), Copy Link, Close, Close Other Tabs (disabled with one tab).
  - Elsewhere: Back, Forward (each disabled at its end), Reload (the app's, ATM-584).
- **`ui/AppContextMenu.svelte`:** one Bits UI menu for the window, opened at the pointer (`customAnchor`), mounted by the routes' layout in the app only. WebKit's menu is turned off everywhere else; over editable or selected text the system's menu shows; a right-click something already answered (the history arrows' lists) stays theirs.
- The e2e fixture answers `open_window`, recording the address.

## Tests
`context-menus.test.ts` (each target, each menu's items and what each does, a window that won't open, a gone tab); `app.spec.ts` right-clicks a card, a tab, the title bar's end and the search box. Units 1892, e2e 593.

## To look at
- Duplicate opens the copy right of the active tab, not of the tab right-clicked (`AppTabs.open`'s place).
- Not run in the real app: WKWebView's own menu being off, and the clipboard there, are checked only in Chromium's emulation.
