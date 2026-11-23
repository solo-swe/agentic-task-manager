+++
id = "01a2044d-957e-70cd-9637-96df7b9b79ae"
number = 114
title = "Skills and packaging: pre-approvals, the loop skill command, checked Homebrew inputs, pinned host keys and images"
status = "approved"
tasks = ["ATM-678"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-22T17:28:43Z
updated = 2026-11-23T00:24:29Z
+++

The audit's skills and packaging findings (A4, A11, A12, A13), the CI and permission parts with your yes of 2026-11-22.

## What changed
- **A4:** the loop skill names `atm sprint list --json` (it said `atm sprints --json`).
- **A12, the Homebrew scripts:** `packaging/homebrew-formula.sh` and `homebrew-cask.sh` refuse a version that isn't MAJOR.MINOR.PATCH and a checksum that isn't 64 lowercase hex digits, printing nothing. The characters are listed (`[!0123456789abcdef]`), not a range: on macOS's shell `a-f` also took in `A-F`, which the new test caught.
- **A13:** `atm version` and `atm docs` pre-approved for every skill in `skills/claude-code.json`.
- **A12, the deploy-key pushes** (`homebrew.yml`, `skills.yml`): a first step refuses anything but a `v<MAJOR>.<MINOR>.<PATCH>` tag; the Homebrew workflow checks out the tag (the skills one did); ssh trusts only github.com's host keys, inline in each workflow, taken from `gh api meta`, with `StrictHostKeyChecking=yes`.
- **A11:** Dependabot watches the cargo crates, grouped like npm (minor and patch together, a major alone); the sandbox's proxy and base images are pinned by digest (`docker buildx imagetools inspect`).
- `docs/security-architecture.md`: the entry for all of it.

## Tests
`tests/scripts/packaging.test.mjs` (bad versions and checksums, for both scripts), `skills.test.ts` (each skill has both pre-approvals; plan and close sprint lists), `ci-workflow.test.ts` (cargo in Dependabot, the tag check before the checkout, the pinned keys, the same keys in both, the sandbox digests).

## To look at
- A host key rotation by GitHub will fail the Homebrew and skills pushes until the keys in both workflows are updated.
- The skills changed: the agent sandbox, or the written skill review, before the release PR.
