+++
id = "01a1eda8-d7da-756a-9fba-3959228188d3"
number = 17
title = "Nothing drawn over the last fold"
status = "approved"
tasks = ["ATM-589"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T07:57:07Z
updated = 2026-11-18T08:17:17Z
+++

The number drawn over the closing *Show n unchanged lines* belonged to a real line: an empty last line of the run (line 31 in the test's file). The code block never drew it, so its row, placed from the row list, landed on the fold below. Two causes, both fixed:

- **`fenced` lost it** (`diff.ts`). It took the joined text and added a final newline only when the text didn't already end with one. A run ending with an empty line joins to text ending in `\n`, so that line went. It now takes the lines themselves: an empty last line is a line, no lines is no line.
- **The block didn't draw it either.** CompMD's code text drops the fence content's last newline, and in a `<pre>` a final newline draws no line after it. So an empty last line gets one newline more, and the e2e test checks the drawn text ends `line 30\n\n`.

Also, `DiffCode` takes its line height from the code block as drawn (its height over its lines), not from its CSS (`line-height`, else font size × 1.6). The CSS value was off by a few pixels per line, about 10 px over four lines. Now that every line is drawn, the drawn height is exact, so rows can't drift from the text over a long run. Wrapped runs (ATM-588) measure each line as before.

Not the cause: a file's own trailing newline. `linesOf` already drops it, and a test now covers it (with CRLF, and a file ending with an empty line keeping that line).

Tests:
- `diff.test.ts`: `fenced` keeps every line, an empty last one with its extra newline, no lines and one empty line. A hunk ending with an empty context line gives its rows and the folds around it, in Split and Unified. `linesOf` on trailing newlines.
- E2e: the run ends with the empty line drawn, the last row ends above the fold, and the rows keep a real line height (not squeezed to fit), in Split (4 rows) and Unified (5).
