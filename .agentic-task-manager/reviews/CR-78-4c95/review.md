+++
id = "01a1f8b3-614e-70bd-abef-694fab694c95"
number = 78
title = "Rendered line numbers: subtle gray, and a row of several lines shows its first"
status = "approved"
tasks = ["ATM-692"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T11:24:27Z
updated = 2026-11-20T11:46:08Z
+++

The rendered view's line numbers, as you asked:

- **Subtle gray:** every number is gray. A changed line's is only a little darker: the theme's muted text colour, not bold, never the accent and never black. The accent stays only while the pointer is on a line or a line is picked for a comment.
- **One number per row:** a row that numbers several lines (a paragraph's two short lines drawn on one visual line) shows its first line, `3`, not `3–4`. The + beside it still says the range it comments on.

**Test:** a new e2e test with a document whose paragraph has two short lines and a changed line checks:
- the changed line's number is not the accent and not bold;
- it is darker than another line's, which is gray too;
- the paragraph's row says 3;
- its + says 3–4.

The same commit also carries ATM-690's e2e test (40 added lines tinted by one box) and makes the + test wait until the wrapped rows are placed before it scrolls (it failed about 1 run in 4 otherwise; 40 of 40 since).

Checks, unit tests and the whole e2e suite pass, except one WebKit jump test. It also fails without these changes (3 runs in 24), so I recorded it as ATM-693.
