+++
id = "01a2104c-c792-73f1-a0d2-aad87d377799"
number = 184
title = "Release 0.44.0: the entries, the security checklist, the Linux screenshots"
status = "approved"
tasks = ["ATM-750"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-25T01:23:17Z
updated = 2026-11-25T01:27:47Z
+++

The release of the sprint *Sidebar overhaul* as 0.44.0, as AGENTS.md's *Versions* and *CI* sections ask.

**To check:**
- `CHANGELOG.md` `[0.44.0]`: every task of the sprint, grouped Added / Changed / Fixed / Developer.
- `WHATS-NEW.md` `[0.44.0]`: five plain-language bullets, written from the changelog.
- `docs/security-architecture.md` §3.3: the 0.44.0 entry. It covers the new task-folder file (the inbox), the entry points (the inbox routes and commands, the documents' listing, `atm inbox`), CI (ATM-749) and the dependencies (CompMD 0.10.0, `syn` 3.0.9 for one proc-macro). The written skill review for the changed skills and `docs/agent/` points to the entries the sprint's tasks added (ATM-763, ATM-770, ATM-772/773, ATM-780).
- `Cargo.toml`: 0.44.0.
- **The screenshots:** 58 Linux baselines regenerated in Docker. They show the item tabs, Tasks' sub-tabs, Board / Dependencies, All's four items, and the version in About. I looked at the standard page, the projects overview and a theme column.
- `visual.spec.ts`: the scope bar and scope menu tests open the Board. A project opens on its Overview since ATM-751, which has no scope bar, so these tests no longer found it.

**Checks run:**
- `pnpm run verify` passed: the check; unit tests 2,137 (96.1% of lines); the CLI suite (926, none failing); functional e2e 634 passed, 84 skipped; screenshots 84.
- `cargo test` (1,120), `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `cargo deny check` (advisories, bans, licences and sources all OK).

**Dependencies dropped:** ATM-746 (backlog) and ATM-740 (skipped) were removed from this task's dependencies on Hakan's word.

**Left for Hakan:** pushing, the release PR to `main`, merging, tagging and publishing. After the release, check that the app job's signing step compiled nothing (ATM-749).
