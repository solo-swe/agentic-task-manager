+++
id = "01a1f5d1-ea8a-7361-be1e-4a6c3da04a9c"
number = 66
title = "A jump to a file stays on it while the files above are drawn"
status = "approved"
tasks = ["ATM-656"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T21:58:57Z
updated = 2026-11-20T10:01:57Z
+++

A jump to a file from the tree now lands with the file at the top of the pane, and it stays there while the files above it are drawn.

## What was wrong (measured)
A new e2e test samples the file's position in every frame, read just after the frame is drawn, for 1.5 s after a jump. It covers a 94-file review: a file in the middle, the last file, and a jump followed at once by a little scroll. Before this change, in WebKit:
- **For good:** the file settled 8 px below the top one time and 11 px above it another. `scrollIntoView` also scrolls whatever holds the pane, and in WebKit it landed differently from run to run.
- **For a frame:** the file was sometimes thrown hundreds of pixels away (−392 px, once −7,731), then pulled back.

Chromium held the file in every run.

## Change (`ReviewPage.svelte`)
**The jump, and the hold that keeps the file at the top:** both now scroll the pane by exactly the card's distance from its top (`toTop`), and nothing else.

ATM-655's fix, keeping code blocks out of scroll anchoring, is already in. With both, the test passes ten of ten (five in Chromium, five in WebKit), and the frame jumps are gone too. I believe the "ends somewhere else" you saw was mostly ATM-655's bug: WebKit's anchoring moved the pane by a block's height when the block's text was highlighted, and since no size changed, nothing ever moved it back.

## Tried and dropped
- **Our own scroll anchoring** for the pane, with the browsers' turned off. A browser postpones a card's resize notice by one frame when the resize comes from the code block's own measurement in that frame, so our version let 96 px flashes through where WebKit's own anchoring didn't.
- **Changing the comment box's focus.** That was ATM-655, and focus wasn't the cause.

## Tests
- **The e2e above,** tagged to run in WebKit too.
- **Unit tests** (`ReviewPage.component.test.ts`): three now give the cards a layout and check the pane's scroll position instead of `scrollIntoView` calls:
  - a jump puts the card at the top;
  - cards above growing by 300 px keep it there;
  - after a wheel it stays where the reader put it;
  - after Next, the hold is off.
- **Checks:** `pnpm run check`, the unit tests (1,785), and the full e2e (536 + the WebKit and timed projects).
