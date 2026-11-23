+++
id = "01a1f8e9-fe52-713d-9813-7653bd11649e"
number = 81
title = "A file jumped to stays put in WebKit: no anchoring scroll while held, the cards above frozen"
status = "approved"
tasks = ["ATM-693"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T12:24:06Z
updated = 2026-11-20T15:53:41Z
+++

In WebKit, a file you jump to sat 19 px off for a few frames, about 1 jump in 8, and once 5,803 px off for a frame. I measured it frame by frame (the pane's scroll, every card's height, every correction the page made):

- **The 19 px:** after the page put the file at the top, WebKit's scroll anchoring scrolled the pane by a line on its own, in a frame where no card changed size. The page's hold only reacts to a card changing size, so it didn't put the file back until a card above changed.
- **With anchoring off alone:** when a card above shrank from its estimate to its real height, the file was 96 px off for that frame. The page's ResizeObserver correction lands a frame late in WebKit; anchoring had been hiding that.

**The fix:** while a file is held after a jump:
- the pane's scroll anchoring is off (`.held`), so WebKit never scrolls it on its own;
- the cards above the held file keep the height they had (`frozen`, their content clipped to it). They're out of view above it, so nothing visible changes, and nothing above the file changes size.

When the reader scrolls, clicks or presses a key, or jumps to a change, the hold ends. The cards above take their real heights, and the file stays where it was: the page scrolls by what they changed, since no anchor saw it. After that, the browser's anchoring is back.

**Measured:** the jump test passed 40 of 40 in WebKit and 40 of 40 in Chromium; before, it failed about 3 runs in 24 in WebKit. The checks, 1,821 unit tests and the whole e2e suite (570) pass.
