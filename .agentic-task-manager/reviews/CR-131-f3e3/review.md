+++
id = "01a2084d-e031-771f-ae24-3d1f4f10f3e3"
number = 131
title = "The release compiles with no signing secret; the secrets in a release environment"
status = "approved"
tasks = ["ATM-661"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-23T12:07:31Z
updated = 2026-11-23T16:00:35Z
+++

The release's signing secrets kept from everything that compiles, and in a `release` environment (audit A1, A5, A11), as Hakan decided on 2026-11-23.

## What changed
- **`release.yml`**
  - The app job compiles the app as Tauri's bundler would (`app:bundle --target universal-apple-darwin --no-bundle`), in a step with no secrets, before the certificate is imported. The signing step's `tauri build` then finds nothing left to build: it only bundles, signs and notarizes. The CLI jobs already compiled before their first secret.
  - `environment: release` on the two jobs that read a secret (`build`, `app-macos`).
  - Runners pinned: `macos-15`, `windows-2025`, `ubuntu-24.04` (no `-latest`).
- **`homebrew.yml` and `skills.yml`:** `environment: release`, and `ubuntu-24.04`.
- **`ci-workflow.test.ts`**, for all three workflows: a step with a secret comes after every step that compiles and compiles nothing itself; every job with a secret names the environment; no `-latest` runner; the app compiles with no secret before it is bundled.
- **Docs:** `packaging/README.md` (the secrets are the environment's; its `v*` deployment rule and tag ruleset; a hand run names the tag as its ref) and `docs/security-architecture.md` (the release machinery), CHANGELOG.

> [!IMPORTANT]
> **The GitHub setup before the release is in ATM-741 (Release 0.43.0)**, round 2: the `release` environment for `v*` tags, the secrets moved into it, a `v*` tag ruleset. Until the secrets are in the environment, the jobs that name it read none.

## Tests
`pnpm run check`, and the unit suite (`ci-workflow.test.ts`). The workflows themselves only run on a tag.

## To look at
- `--no-bundle`: Tauri 2's `tauri build` has it, and it builds with the same features and profile as the bundling run. If the signing step compiled anything, it would show in its log. Worth a look at the first release's log.
- A hand run of Homebrew or Skills from `main` would now be refused by the environment's `v*` rule. The README says to run them with `--ref v<version>`.
