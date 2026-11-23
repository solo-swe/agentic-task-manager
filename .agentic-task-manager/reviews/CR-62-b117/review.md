+++
id = "01a1f513-0935-72e9-be26-4f3a70c2b117"
number = 62
title = "Diff viewer: line numbers level with their lines; Wrap moves none"
status = "approved"
tasks = ["ATM-652"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T18:30:27Z
updated = 2026-11-20T09:47:34Z
+++

Line numbers are now level with their lines, and Wrap no longer moves them.

## What was wrong (measured)
Each number's text centre, against the first letter of its line, in the source view:

| | Before | After |
|---|---|---|
| Split | 1.5 px above | 0.5 px |
| Unified | 1.5 px above | 0.5 px |
| Unified, wrapped | 2.5 px below | 0.1 px |

Before, toggling Wrap moved every number 4 px against its text. There were two causes:
- **Not wrapped:** a number's text sat at the top of its row (a smaller font in the code's row height), not centred in the line's height.
- **Wrapped:** each row started where its first letter's box starts. That box is the glyphs' height, not the line's, so every row sat half the line spacing too low.

## Change (`DiffCode.svelte`)
- **The number:** centred in one line of the code in both modes, so it sits on a wrapped line's first visual line without a style of its own for Wrap.
- **A wrapped row's top:** half a line height above its first letter's centre, which is where its line starts.

The rendered view's numbers were already level (within 1 px). A test now holds that too.

## Not done here
The task suggested CompMD 0.8.0's `view.lines()` and `view.lineHeights()`. The numbers didn't need them: the drift was atm's own CSS and measurement. They stay the way to get Wrap in Split (ATM-588).

## Tests
Two e2e tests:
- every number's text within 1 px of its line's, in Split, Unified and Unified wrapped, and Wrap moving none by more than 1 px;
- the same for the rendered view (a heading and two paragraphs).

Both pass in Chromium and WebKit. WebKit was run with a scratch config, as the suite is Chromium only.

## Checks
- `pnpm run check`, the unit tests (1,785) and the reviews specs (61).
- The full functional e2e: everything passed except one timed test. The long review's first draw took 208 ms against its 200 ms budget. I measured it rather than rerunning it:
  - alone, with this change: 77, 100 and 50 ms;
  - in parallel with the other timed tests, with this change: 143 and 83 ms;
  - in parallel, without this change: 443 and 98 ms.

  So it isn't this change: the timed tests disturb each other when they run together (ATM-641 kept them away from the functional tests, not from each other). That calls for a task of its own.
- The screenshots of the diff viewer may move by a pixel. They are regenerated in the release task.

## Found on the way
In WebKit, the existing test "+ on a line wrapped opens the comment box where it is; the pane stays" fails with and without this change: the pane jumps about 1,600 px. That is ATM-625's bug, still present in the app's engine. A task of its own.
