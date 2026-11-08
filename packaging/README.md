# Releasing the atm CLI

How the `atm` binary reaches users ([better-way-to-distribute-atm.md](../docs/better-way-to-distribute-atm.md) §1):
release builds on a tag, the macOS ones signed and notarized, every one with its checksum; the Homebrew formula in
`solo-swe/homebrew-tap` (the tap agentic-status-bar publishes to), so users install and update with
`brew install solo-swe/tap/atm` and `brew upgrade`. The app (T-455 onwards) brings its own updates,
through Tauri's updater (T-483; agentic-status-bar, not a Tauri app, uses Sparkle).

## What's here

| File | What it does |
|---|---|
| `release.sh <version> [--push]` | Checks the version is `Cargo.toml`'s and that `main` is clean and pushed, then tags `v<version>` (and pushes the tag with `--push`) |
| `sign-macos.sh <binary>` | Signs a macOS build with the Developer ID (hardened runtime) and notarizes it; the release workflow runs it |
| `homebrew-formula.sh <version> <SHA256SUMS>` | Prints `Formula/atm.rb` for a release, from its checksums; the Homebrew workflow writes it to the tap |
| `skills-export.sh <version> <checkout>` | Puts this version's `skills/atm-*` and the README's atm section into a checkout of `solo-swe/skills`, leaving other apps' parts alone ([ADR-002](../docs/adr/ADR-002-atm-public-skills-repo.md)); the Skills workflow commits, tags `atm-v<version>` and pushes |

## A release

1. The release PR is merged to `main`; the version is in `Cargo.toml`, with its `CHANGELOG.md` and `WHATS-NEW.md` entries
   (`check-versions.mjs`).
2. `packaging/release.sh 0.33.0 --push` tags `v0.33.0` and pushes the tag.
3. `.github/workflows/release.yml` builds atm for macOS (arm64, x86_64), Linux (x86_64, arm64, on Ubuntu 22.04 for an
   older glibc) and Windows (x86_64), signs and notarizes the macOS builds, and drafts a GitHub release with
   `atm-<version>-<target>.tar.gz` (`.zip` for Windows) and `SHA256SUMS`.
4. Check the draft, then publish it. `.github/workflows/homebrew.yml` writes the formula to the tap, and
   `.github/workflows/skills.yml` the skills to `solo-swe/skills` (`npx skills add solo-swe/skills`); both can also be
   run by hand with a tag. Without its deploy key, the Skills workflow says so and does nothing.

## Secrets and keys

Set in this repository's settings (Secrets and variables → Actions), the same as agentic-status-bar's:

| Secret | What it is |
|---|---|
| `DEVELOPER_ID_CERT_P12` | The Developer ID Application certificate with its private key, as a `.p12`, base64 |
| `DEVELOPER_ID_CERT_PASSWORD` | The `.p12`'s password |
| `NOTARY_API_KEY_P8` | An App Store Connect API key for notarization (`.p8`), base64 |
| `NOTARY_API_KEY_ID` | That key's ID |
| `NOTARY_API_ISSUER_ID` | Its issuer ID |
| `HOMEBREW_TAP_DEPLOY_KEY` | The private half of a deploy key with write access to `solo-swe/homebrew-tap` (one per repository: GitHub doesn't share a deploy key between repositories) |
| `SKILLS_DEPLOY_KEY` | The private half of a deploy key with write access to `solo-swe/skills` (the public repo, created once, empty) |

The release workflow reads the signing secrets only in the macOS build jobs; only its last job, which builds nothing,
can write to the repository (its releases). The Homebrew workflow reads only the tap's deploy key, the Skills
workflow only the skills repo's.
