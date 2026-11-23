+++
id = "01a205dc-5afa-7636-8314-cdc911b96b83"
number = 124
title = "Right-click on selected text: atm’s menu with Copy; the menu above every layer"
status = "approved"
tasks = ["ATM-699"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T00:44:17Z
updated = 2026-11-23T00:46:35Z
+++

The fix for what CR-117 missed (Hakan): selecting text and right-clicking showed WebKit's menu, with its own Reload.

## What changed
- **Selected text** (outside a field) gets atm's menu: **Copy** (the text as selected) first, then Back, Forward and Reload. It counts as selected text on a card or a tab too.
- **A field you type in** (search, a comment box) keeps the system's menu, for its Cut and Paste.
- **The menu is drawn above every layer** (z-index 200). It was at 60, under the details panel (100) and the sheets (110), so a right-click inside them opened the menu out of sight. The e2e found this.
- `docs/tabs-and-navigation-design.md` §4.5 says so.

## Tests
- Unit tests: a selection, on a card, in a field, spaces alone; the selection's items and Copy; a failed copy is reported.
- e2e in the app: the task's description is selected in the details panel, then a right-click gives Copy, Back, Forward, Reload, and Copy puts the text on the clipboard. It passed 5 of 5 runs in parallel, and the full functional suite passed.
