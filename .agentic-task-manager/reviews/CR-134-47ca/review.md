+++
id = "01a20944-376a-717c-b942-73c274b147ca"
number = 134
title = "Release 0.43.0"
status = "open"
tasks = ["ATM-741"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T16:36:35Z
updated = 2026-11-23T17:34:12Z
+++

The sprint *Audit and tabs* as 0.43.0, on `dev/0.43.0`.

## What changed
- **`WHATS-NEW.md`:** a sixth bullet for what users notice from ATM-722: atm's figures agree everywhere, and the overview loads less. Both entries are dated 2026-11-23.
- **`docs/security-architecture.md` §3:** the 0.43.0 entry is finished:
  - the entry points: the stats route and command;
  - who is an agent;
  - the CI changes;
  - the review watch;
  - the dependencies;
  - a written skill review of every skill and `docs/agent/` change (no API key for the sandbox run);
  - the checks run.
- **Two advisories** published during the sprint, which failed `cargo deny`, are fixed by patch releases of crates already in the tree:
  - `cookie` 0.18.3 (RUSTSEC-2026-0594, private cookies after a fork, which atm never uses);
  - `url` 2.5.9 (RUSTSEC-2026-0592, a stack overflow on nested `blob:` URLs).

  These change `Cargo.lock` only, with a CHANGELOG line.
- **Screenshots:** 16 of 84 Linux baselines regenerated in Docker and looked at: the Kanban and List (atm's progress in the header), the projects overview, the Settings menu and the update notice.

## Tests
- `pnpm run verify`: checks, unit tests with coverage (1,902, 96.5% of lines), the CLI suite (873) and the functional e2e (594).
- The screenshots (84) in Docker; the first verify run's Linux build failed once with "can't find crate" (stale artifacts) and passed alone.
- `cargo test` (1,040), clippy, `cargo deny check`.

## Round 2: the GitHub setup (`8faf602`), on Hakan's word
- **The `release` environment** is created, with one deployment rule: tags matching `v*`.
- **The `v*` tag ruleset** is active: only an admin (who bypasses) may create, move or delete a release tag. It is kept in `.github/rulesets/release-tags.json`, as `main.json` is.
- **The nine secrets are still repository secrets.** GitHub never shows a secret's value, so they can't be copied: each goes into the environment again from where it is kept (`gh secret set <NAME> --env release`), and the repository copy is removed after. Until then nothing breaks, since a job in the environment reads the repository's secrets too.

## To look at
- **Before you tag:** the secrets into the environment, as above. After the release, check that the app job's signing step compiled nothing.
- Pushing, the release PR to `main`, merging, tagging and publishing are yours.
- `docs/codebase-audit-2026-11.md` (the audit the sprint's tasks link to) is still untracked in the working copy: it isn't in any commit. Commit it, or say and I will.
