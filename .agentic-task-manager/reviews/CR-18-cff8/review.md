+++
id = "01a1edaa-f5bd-7353-b4f4-ef9e6525cff8"
number = 18
title = "A long line scroll bar stays off the fold row"
status = "approved"
tasks = ["ATM-590"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:59:26Z
updated = 2026-11-18T08:17:49Z
+++

A long line's sideways scroll bar no longer covers the closing *Show n unchanged lines*.

**What I could and couldn't reproduce:** headless Chromium (the e2e browser) uses classic scroll bars, which take their own room inside the code box, so the new test passed before the fix too. What you saw needs macOS's overlay scroll bars, laid over the content, which a headless browser doesn't draw. Before ATM-589 the run was also a line shorter than its rows, which put its bottom, and an overlay bar there, over the fold. So the fix makes the layout hold whatever kind of scroll bar the system draws:

- **Room for an overlay bar:** the code block keeps padding under its last line, so an overlay bar covers neither that line nor what follows the run. A classic bar takes its own room as before.
- **A run draws nothing past its end:** `.diff-code` clips (`overflow: clip`, with a 2 px margin for focus rings), so neither an overlay bar nor a gutter row can reach the fold.
- **The fold row is above** anything a run before it could still draw (`position: relative; z-index: 1`).

The rows are unchanged: they're measured from the code element, not the block with its padding, and the ATM-589 and wrap tests pass.

Test (e2e), in Split and Unified with a 600-character line before the last fold:
- the block scrolls sideways and ends above the fold;
- `elementFromPoint` at the fold button's centre, and just inside its top edge, is the button;
- the run clips and the fold row stacks above it. Those last two are the guards for the overlay case the browser can't show.

The review screenshots will shift by the padding; they're regenerated in the release task. It's worth a look in the app on your Mac, the one place the overlay bars show.
