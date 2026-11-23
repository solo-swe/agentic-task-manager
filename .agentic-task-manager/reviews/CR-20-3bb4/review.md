+++
id = "01a1edc5-977a-7058-a293-2d2663c63bb4"
number = 20
title = "A task named in a review opens its details over it"
status = "approved"
tasks = ["ATM-593"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T08:28:31Z
updated = 2026-11-18T18:06:43Z
+++

A task named in a review now opens its details **over the review**, instead of sending you to the project's task list.

**Where a review names a task:**
- **the Overview's Tasks fact** (it was a link to `/projects/<id>/list?task=…`);
- **the Reviews list's rows**: the keys are links now, each opening its task, not the review;
- **keys written in the description and the comments** (`ATM-12`, a provisional `~ATM-12`, an old `T-7`, another project's `LFB-5`). They're linked only when they name a task the page knows; a key naming nothing stays text. A key inside a link or in code stays as written.

**What a click does:**
- A task of this project: its details over the review, the review's own address with `?task=ATM-12`. Closing them (×, Escape, Back) leaves the review as it was: the same tab, round, file and scroll.
- Another project's task: its project's page, with it open. In the app it opens in a tab of its own.
- ⌘-click, Ctrl-click or a middle click: a tab of its own, at the task's project page with its details. That's a browser tab, or an app tab in the app.
- Every link's `href` is the task's own page, so copying or hovering a link shows where it goes.

**How:**
- `compmd.ts` `safeTree` gets `taskKnown`: a known key in a text node becomes a `#task=<key>` link.
- `CompMd` takes `tasks: TaskLinks` (`known`, `open`) and follows `#task=` links through it. A middle click too.
- `reviews/links.ts` gives the comments the page's task links through context, as ATM-586 does document links.
- `ReviewsTab` decides what opening a task does: the same `findTask` and `refOf` the rest of the dashboard uses.

Tests:
- `compmd-processor.test.ts`: a known key links, an unknown one stays text, and a key in code or inside a link is untouched. A provisional key works too, and the text reads the same either way.
- `ReviewList.component.test.ts`: a known key in a row is a link with its address, and opens its task (⌘ in a tab of its own) without opening the review. An unknown key stays text.
- E2e: the Tasks fact opens T-001's details over CR-2 at `CR-2?task=T-001`, and Escape is back at `CR-2` on the same tab. A comment's `T-002` links and opens, `ZZZ-9` doesn't. ⌘-click opens a browser tab at the task's list. The list row's key opens the details with the list still under them.

All the review e2e tests pass on their own. The long-review performance test fails when four or five run at once and passes one at a time. That's the known flake (ATM-615), which I'm taking next.
