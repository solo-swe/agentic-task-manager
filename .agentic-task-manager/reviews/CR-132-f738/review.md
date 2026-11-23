+++
id = "01a20861-d8f0-7053-baff-0eebb1bdf738"
number = 132
title = "Every agent harness is an agent; inside one, no acting as a person"
status = "approved"
tasks = ["ATM-660"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T12:29:19Z
updated = 2026-11-23T15:52:05Z
+++

Who is an agent (audit L2): two of the three parts Hakan decided on 2026-11-23. The third waits on a question (below).

## What changed
- **`atm_lib::actor::harness_identity`:** the identity of the agent harness atm runs in, from the markers it puts in its commands' environment:
  - Claude Code: `CLAUDE_CODE_SESSION_ID`, else `CLAUDECODE`;
  - Codex: `CODEX_THREAD_ID`, else `CODEX_SANDBOX`;
  - Gemini CLI: `GEMINI_CLI`, with `GEMINI_SESSION_ID`;
  - OpenCode: `OPENCODE`;
  - any harness naming itself in `AI_AGENT`.

  Each is `<harness>:<session>` (`agent:<name>` for `AI_AGENT`), per linked worktree, as Claude Code's always was. `agent_identity` uses it before the git name.
- **`atm_common::edits::AGENT_PREFIXES`:** `is_agent` knows every harness's prefix, not only `claude:`. So a review mode holds for a Codex or Gemini agent: it can't finish a `blocker` task without its review, and a `batch` task gets its pending sprint review.
- **`check_named`**, called in `main` for every command but those whose `--agent` is a filter: inside a harness session, an `--agent` or `ATM_AGENT` naming a person is refused (exit 2, naming the session and the person). Naming an agent is taken.
- The dashboard's `isAgent` uses the same prefixes; a unit test reads the Rust list, so they can't drift.
- The CLI suite's environment drops every harness's markers, not only Claude Code's.
- Docs: `docs/security-architecture.md` (the review modes), CHANGELOG.

## Tests
`tests/scripts/scenarios/agents.json`:
- each harness's markers give its identity;
- a person's name inside a session is refused, by `ATM_AGENT` and by `--agent`;
- an agent's name is taken; outside a harness any name is;
- a Codex agent can't finish a blocker task.

Also `actors.test.ts`, clippy, `cargo test`, the CLI suite and the functional e2e.

## To look at
- The markers come from what the harnesses document or set, as far as I could find (the projects that detect agents list the same). Cursor and Copilot reportedly set none, so their agents still count as the person whose git name they run under. OpenCode's `OPENCODE` I couldn't confirm from its own docs: a marker that is never set does no harm.
- **The third part, not done: a person only on positive evidence.** Read literally ("an interactive terminal with no harness markers"), every run that isn't at a terminal becomes an agent: git hooks, CI steps, scripts, the CLI suite's own runs (hundreds of scenarios rely on the git name being a person). It is also the only rule that would catch Cursor and Copilot. The task is blocked on this question.
