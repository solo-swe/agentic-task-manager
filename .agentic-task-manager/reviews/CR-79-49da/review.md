+++
id = "01a1f8d4-2189-7329-a1da-dc9eb10d49da"
number = 79
title = "Comments on rendered Markdown in a margin, with a callout to each, like Word"
status = "approved"
tasks = ["ATM-651"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T12:00:14Z
updated = 2026-11-20T12:04:35Z
+++

Comments on rendered Markdown, as Word shows them (your CR-76 comment: "like a callout from the line, like MS word"). This replaces CR-76's approach, which cut the page under each block.

- **The page stays whole**, as the document viewer draws it: no cut parts, so footnotes and the rest stay where they belong. The cut code is gone (`renderedParts`' cuts, DocumentPage's `after` slot).
- **A margin right of the page** holds each thread in a balloon, level with the text it is on. Balloons are pushed down only where they would overlap.
- **A callout line** runs from the start of the commented text to the page's edge and on to its balloon: dashed, solid for the selected thread.
- **The commented text** is marked faintly, more strongly when its thread is selected.
- **A new comment** opens its box in a balloon the same way, with its callout in the accent colour.
- **In Split**, the side with comments gets the margin's room on top of its share, so both pages stay the same width.
- **Thread boxes fit the balloon:** header, author and button rows wrap whole, and labels and buttons never break inside. This also applies to the source view's threads when they are narrow.

**Tests:** the e2e test now checks, in Chromium and WebKit:
- each balloon right of the page and level with its paragraph;
- a callout per thread;
- balloons never overlap;
- the page whole (one part, the paragraphs as close as without comments);
- a new comment written and shown there;
- the numbers and marks beside their text.

The three tests about where the comment box opens now expect it beside the text. The checks, 1,821 unit tests and the whole e2e suite pass, except ATM-693's WebKit jump test, which also fails without these changes.
