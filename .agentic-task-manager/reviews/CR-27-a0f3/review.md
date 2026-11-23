+++
id = "01a1f08d-3a61-74cc-bc80-4624b622a0f3"
number = 27
title = "The review watch catches up on what it missed"
status = "approved"
tasks = ["ATM-624"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-18T21:25:49Z
updated = 2026-11-18T21:43:14Z
+++

The review watch no longer loses what happens while it isn't running.

**Why:** Claude Code's Monitor ends a watch after 5 minutes. A new watch started from *now*, so a comment or decision made in between was never told (CR-20 and CR-23 sat approved).

**What changed:**
- **It catches up.** atm keeps where an agent's last watch on a project was: the comments it told or passed over, and each review's status as it last saw it. It's a small JSON file in atm's state folder (`watches/`), one per project and agent; nothing goes in the project. A new watch first tells, at once and in order, what came meanwhile (comments, replies, decisions), then follows live. The first watch ever starts from now, as before. A cursor that can't be read is treated as none.
- **`--once`:** stops after the first look that told something; the caught-up events count. At the time up with nothing, exit code 3, as `atm review wait`. A harness that reports a background command only when it ends can run it so, and restart it each time.
- **`--since <time>`:** tells what came since that time, whatever was told before; a value that isn't a time is refused.
- **The skills** (loop, do-task) and `atm docs reviews`: start the watch again whenever it ends; it catches up.

**How:** in `review_listen.rs`, the start moved into `starting_point` (the cursor, `--since`, or now). The first look runs at once instead of after the first wait. The cursor is saved after every look. The CLI gets the two flags and exit code 3.

**Tests:**
- atm-lib: the cursor written and read back; a missing or broken one is none; one file per project and per agent.
- The CLI suite: the first watch starts from now (exit 3 with `--once`). A comment and a decision made while no watch ran are told by the next watch, in order, once. The watch after it has nothing old. Each agent has its own cursor. `--since` tells them again. A bad `--since` is refused.
- The whole CLI suite passes (825 tests). Clippy and fmt pass.

The skills changed, so the written skill review is needed before the release PR (with ATM-623's).
