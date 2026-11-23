+++
id = "01a207d8-efbd-709d-a951-35af7a9a3aed"
number = 126
title = "Docs in line with tabs and navigation; atm 0.43.0 and its entries"
status = "approved"
tasks = ["ATM-701"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T09:59:47Z
updated = 2026-11-23T15:57:55Z
+++

The docs in line with tabs and navigation (task 7 of `docs/tabs-and-navigation-design.md` §7), and atm 0.43.0's entries.

## What changed
- **`docs/app-desktop-ui-design.md`**
  - §3 opens with a pointer: how a tab behaves (its history, the pop-ups, the addresses, clicks and menus) is the tabs and navigation design's.
  - What was wrong is corrected:
    - switching is Next and Previous Tab ⌃Tab and ⌃⇧Tab (not ⌘⇧[ ⌘⇧]), and switching adds no history;
    - ⌘-click, ⇧⌘-click and a middle click open a new tab;
    - a tab's right-click menu exists, with its six items.
  - §6 has the same pointer. Its table is the menu as built in `menu.rs`, including the Windows and Linux keys, plus a paragraph on the right-click menus.
- **`docs/multi-project-api.md` §2.7:** the new addresses were already there (ATM-697). A review page is now said to be in a tab under the window's title bar in the app, a page of its own only in the browser.
- **atm 0.43.0:**
  - `Cargo.toml` and `Cargo.lock`;
  - `CHANGELOG.md`: the sprint's work, by task;
  - `WHATS-NEW.md`: five lines for users.
  - The headings are dated 2026-11-23. Round 2: they said *Unreleased* in round 1, and the changelog's unit test wants every release dated; the release task can date them again.

## Tests
`pnpm run check` (the entries for 0.43.0), and the CLI suite (`atm version`, the docs check).

## To look at
- WHATS-NEW's five lines: tabs' own history, browser clicks and menus, a review waiting on its tab, a link for every page, watches no longer slowing the computer.
- The version is bumped mid-sprint, as the dashboard and skills changed (AGENTS.md: running servers restart on the new dashboard).
