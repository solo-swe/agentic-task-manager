+++
id = "01a1f831-821d-739c-8ec5-8bb9f6f43623"
number = 70
title = "A live review updates without flicker, and the place read stays put"
status = "approved"
tasks = ["ATM-687"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T09:02:36Z
updated = 2026-11-20T10:00:58Z
+++

Round 2: the rule that kept everything inside a card out of scroll anchoring (`ReviewFile.svelte`) is gone.

Since the live pane keeps the place read itself, with the browser's anchoring off, that rule no longer did anything for a live review. It only changed reviews that aren't live, and there it broke ATM-656's jump: in WebKit, 1 run in 8 had the file gone to sit 18 px off for about 20 frames.

Without it, in WebKit: ATM-656's jump test passes 8 of 8, and the six live-review tests 48 of 48 (eight runs each). In Chromium, both pass twice.
