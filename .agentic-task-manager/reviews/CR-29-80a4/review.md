+++
id = "01a1f0a1-30cc-7667-8d0b-4a204e3280a4"
number = 29
title = "Rendered view: the comment box opens under the text selected"
status = "approved"
tasks = ["ATM-627"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T21:47:38Z
updated = 2026-11-19T06:04:11Z
+++

In the **Rendered** view, the comment box for selected text was drawn after the *whole* document. A comment on line 5 of a long document opened its box thousands of pixels below, and focusing it scrolled the page away from the text. This is the "document and comment box mixed up" you saw in the app.

## Change
- The box now opens **right under the selected text**, over the side it was selected on (split or unified). It covers the lines below it until you comment or cancel.
- The rendered view is its own stacking context, so the box draws above the page, its change marks and its folds, and stays under the card's sticky header.

## Tests
A new e2e, split and unified, also run in WebKit (with a temporary Playwright project, not committed). It selects a paragraph near the start of a long document and checks that the box:
- has the focus and is in view;
- is the topmost element at its centre;
- is less than 200 px below the selected text.

The full functional suite passed except "switching tabs" (performance), which passes alone and is the open budget task ATM-629.

## To check
- Does the box over the following lines feel right, or would you rather it push the document down? Pushing it down means splitting CompMD's part at the selection, which is more work.
