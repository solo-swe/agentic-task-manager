+++
id = "01a1f29f-7c5f-72c0-8d72-629890910724"
number = 51
title = "Agents never miss review comments: atm knows whether a watch runs"
status = "approved"
tasks = ["ATM-642"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T07:05:00Z
updated = 2026-11-19T07:43:08Z
+++

Agents can no longer quietly stop watching their reviews: atm knows when a watch runs, and says so when it matters, without being noisy. This is the design you agreed.

## Change
- **A live watch is known** (`atm-lib` `review_listen`). `atm review watch` keeps a lease beside its cursor (`watches/<hash>.live`, the time it lasts to). It's renewed on every look, at most every 5 s, lasts 60 s unrenewed, and is removed when the watch ends. So a stopped, crashed or killed watch is known within a minute.
- **`watch_state`.** Whether a watch runs; if not, the agent's open reviews (its own, or naming a task it worked; waiting, live or changes requested), and those changed since its last watch stopped (by the cursor's time). It reads review headers and the task list only, no diffs, so it's cheap enough for every command.
- **The reminder** (`atm-cli` `main.rs`). After a command that renews claims, not `review watch` or `wait` themselves, an agent that isn't watching, with a review of its work open, gets one line on stderr, e.g.:
  `atm: you aren't watching your reviews (2 open: CR-48, CR-49; changed since your last watch: CR-49). Start \`atm review watch --json\` as a background command…`
  - At most every 10 minutes, but at once when a review changed that the last reminder didn't name.
  - Never on stdout, never for a person, never with `--agent`.
- **`atm loop next --json`** gains `"watch": {"running", "open", "since_last_watch"}`.
- **Skills and docs:**
  - `atm-loop-tasks` and `atm-do-task`: start the watch when `watch.running` is false or the stderr line appears, and run it as a plain background command (not under a monitor with a short cap).
  - `atm docs reviews` explains all of it.
  - The security doc starts the 0.41.0 section with this change's written review.

## Tests
- **atm-lib unit:**
  - the lease (holds until its time; missing or broken, never; written while running, removed on drop);
  - which reviews are open (own or naming its task; waiting, live, changes requested; not approved, closed or draft) and which changed after the last watch (none before any watch);
  - the reminder's rule (quiet with a watch or nothing open; again after 10 minutes, or at once on a new change; many reviews shortened).
- **CLI suite** (`reviews.test.mjs`):
  - nothing said with nothing open;
  - said at once on the `review create` that opened one, on stderr only;
  - quiet the next time;
  - never for a person;
  - `loop next --json` says `running: false` with the open review, and `running: true` while a watch runs.
- **Scenarios:** `loop next --json`'s expected output now includes `watch`.
- **Suites:** the whole CLI suite, `cargo test` (atm-lib, atm-cli, atm-api), clippy, fmt and `pnpm run check` pass.

Tried on this session too: with my watch started by the older binary, `loop next --json` said `running: false`. After restarting it on the new binary, it says `running: true`.
