+++
id = "01a1fb2e-a5e0-7660-9263-058ba1ecfbea"
number = 99
title = "Review events reach the agent: --once in the skills, an inbox, Claude Code hook; looks 20x faster"
status = "approved"
tasks = ["ATM-708"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T22:58:20Z
updated = 2026-11-20T23:31:37Z
+++

Review events now reach the agent without it having to remember to read its watch. This does A, B and C from your analysis, plus two speed fixes the work turned up.

- **A, the skills:**
  - `atm-loop-tasks` and `atm-do-task` start the watch with `--once`, and say why: its ending is what tells the agent.
  - They start it again at once after each report, in the middle of a task too, and answer the unread line with `atm review inbox`.
  - `docs/agent/reviews.md` says the same.
  - `atm review inbox` is pre-approved for both skills. `atm-push` starts no watch, so it's unchanged.
- **B, the inbox:**
  - A second cursor per agent records what it has read.
  - A watch with `--once` counts as read; one that runs on doesn't.
  - Every command an agent runs prints `atm: 2 unread about your work: CR-1 comment by …, CR-1 changes requested. …` on stderr until it reads them.
  - `atm review inbox [--peek] [--json]` shows them, and marks them read unless `--peek`.
  - The first time, the inbox starts from the watch's cursor.
- **C, the hook:**
  - `atm agents hooks install|status|uninstall [--project]` adds `atm review inbox --hook` to Claude Code's `UserPromptSubmit`, `PostToolUse` and `Stop`, and leaves every other setting as it was.
  - The hook gets the session from its stdin, adds the unread events to the agent's context, or holds a reply while any are unread.
  - It always exits 0 and prints nothing when it has nothing to say.
  - It's yours to run; the skills never do. Security doc: new invariant I35.
- **Speed:** one look took 12 to 28 s here (debug build, about 100 reviews). The watch had paid that every 5 s all along. The causes, found by sampling:
  - **One `git diff` per commit of the agent's tasks, on every look.** A commit's files are now cached per project, since a commit never changes.
  - **Every review read in full for each review.** Now one read.
  - **The task archive copied on every key lookup.** `keys::resolve` now takes it borrowed.
  - Now a full look takes 0.6 s, and the inbox, which reads only reviews changed since it was last read, takes 60 ms when nothing has changed.

Tests: `tests/scripts/reviews/inbox.test.mjs` (the inbox, `--once` counting as read, the hook's answers, install, uninstall and refusal), atm-lib's `agent_hooks` tests and the unread line's, and `skills.test.ts`.

To check:
- The inbox reads only reviews changed since its last read. A comment that only becomes yours later (you start its task afterwards) reaches you through the watch, not the inbox.
- The skills changed, so the agent sandbox, or the written skill review, is due before the release PR merges.
- To try the hook: `atm agents hooks install` (yours), then a comment on a review of this session's work.
