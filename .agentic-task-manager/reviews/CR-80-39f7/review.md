+++
id = "01a1f8da-2070-701f-b4c6-d6608bc439f7"
number = 80
title = "A card’s review badge wraps between its parts, never inside one"
status = "approved"
tasks = ["ATM-702"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T12:06:47Z
updated = 2026-11-20T12:13:32Z
+++

On a narrow Kanban card, the review badge broke inside its parts: `CR-` over `79`, `Waiting for` over `review`, `needs` over `you`.

The badge (`ReviewLink.svelte`) now wraps between its parts (the reference, where the review stands, *needs you*), each kept on one line, and stays inside the card. Wrapped, the card's badge is a rounded box instead of a two-line pill.

**Test:** `kanban.spec.ts` narrows a card to 200 px and checks that every part of its badge is on one line and the badge stays inside the card. The other badge tests in the Kanban and details specs pass too.
