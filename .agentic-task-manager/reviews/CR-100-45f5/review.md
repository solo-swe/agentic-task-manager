+++
id = "01a1fb33-be75-71fb-bcea-53c60f5845f5"
number = 100
title = 'A live review compared against an earlier round: the working copy, not "no round N"'
status = "approved"
tasks = ["ATM-710"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T23:03:54Z
updated = 2026-11-20T23:32:19Z
+++

The bug you hit on CR-58: a live review's latest compared against an earlier round said "no round N".

- **The cause:** a live review shows its changed working copy as the round after the stored ones, but only when no round is named. Comparing against round 2, the page names the round it shows (`?round=3&against=2`), and round 3 isn't stored.
- **The fix:** on a live review, asking for the round after the stored ones means the working copy, so `against` compares with it. If nothing changed since the last checkpoint, that round is the last one.
- **Tests** (atm-lib, on a real working copy): asked by number, compared against round 1, unchanged, and a round past it, which is still refused. The test failed before the fix with your error.

Not done: the e2e test the task names. The e2e tests serve fixed review pages from a stand-in server, so they can't reach this server-side code.
