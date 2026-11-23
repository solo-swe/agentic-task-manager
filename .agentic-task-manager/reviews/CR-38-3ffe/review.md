+++
id = "01a1f143-c6af-77a7-a20a-000088b73ffe"
number = 38
title = "The Comments tab shows its open comments beside the total"
status = "approved"
tasks = ["ATM-614"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T00:45:13Z
updated = 2026-11-19T06:05:13Z
+++

The Comments tab in a review's top bar shows the open comments beside the total: **Comments 7 (2 open)**. With none open it stays **Comments 7**.

## Change
- `ReviewPage.svelte`: the tab's count, and beside it, quieter, *(N open)* from the same open count *Approve* uses. Screen readers hear "Comments: 7, 2 open".
- The reviews list was already there: each row says *2 open of 7*. No change.

It sits on top of ATM-612 (drafts, 6e75ac5, waiting on your size decision), so drafted comments count as open here, as they do for *Approve*.

## Tests
- Component test: the total with the open count and its accessible name; none open, the total alone.
- The checks pass, and so do the reviews e2e specs (56). The full suite fails only on the 1 MiB page-size guardrail, which ATM-612's question to you is about.
