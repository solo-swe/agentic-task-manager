+++
id = "01a20fd6-600c-77cb-9512-c1b569b14635"
number = 180
title = "Docs: Help on the sidebar and tabs; the desktop UI design points to the overhaul"
status = "approved"
tasks = ["ATM-758"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T23:13:57Z
updated = 2026-11-24T23:22:56Z
+++

The docs in line with the sidebar overhaul as built.

**Help** (`docs/help.md`, the dashboard's Help):
- *Your projects*: the app chooses a project at the sidebar's top, a browser with the arrow next to the title; *Hide inactive projects* as the overview names it.
- **New: *The sidebar and tabs*** — the scope and its items (each a new tab, ⌘-click in the background, the right-click menu), the Inbox's and Code Reviews' counts, the hidden sidebar's rail, + and its chooser (keys, typing filters), tab titles (*ATM - Tasks*, *ATM - CR-134*), each tab's own Back and Forward; in a browser the items are the tabs.
- *Views*: the six items as a project has them, with Tasks' (Board, List, Dependencies) and Analysis' (Progress, Agents) views; Inbox and Documents described.
- *The skills*: `atm-triage-inbox` added.

**The desktop UI design** (`docs/app-desktop-ui-design.md` §4): replaced by a pointer to `docs/sidebar-overhaul-design.md`, with what still holds (the component, its toggle and width, the Notion-style split, no vibrancy).

**Checked:** Help's component test (its headings and in-page links) and its e2e specs. A Help screenshot, if one shows the changed text, is regenerated in the release task.
