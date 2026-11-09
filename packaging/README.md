# Releasing atm: the CLI and the app

How the `atm` binary and the atm app reach users ([better-way-to-distribute-atm.md](../docs/better-way-to-distribute-atm.md) §1):
release builds on a tag, the macOS ones signed and notarized, every one with its checksum; the Homebrew formula and
cask in `solo-swe/homebrew-tap` (the tap agentic-status-bar publishes to), so users install and update with
`brew install solo-swe/tap/atm` (the CLI) or `brew install --cask solo-swe/tap/atm-app` (the app, with its own `atm`
on PATH), and `brew upgrade`. The app will also bring its own updates, through Tauri's updater (T-483;
agentic-status-bar, not a Tauri app, uses Sparkle).

**The app and the formula are one or the other** (T-460): the app's DMG bundles the `atm` CLI built with it
(`atm.app/Contents/MacOS/atm`), and its cask links that onto PATH, so the CLI agents run and the atm inside the app are
always the same build. Someone with the formula uninstalls it before installing the app (`brew uninstall
solo-swe/tap/atm`), as the cask's caveats say: a cask can declare a conflict only with another cask (the
[Cask Cookbook](https://docs.brew.sh/Cask-Cookbook#stanza-conflicts_with); Homebrew refuses `conflicts_with formula:`,
which 0.34.0's first cask had). Without Homebrew, the DMG alone gives the app; its CLI is inside it.

**An atm from before 0.34.0 on the same machine** (another install, a server still running from it) keeps using
`~/.agentic-task-manager/`, which 0.34.0 moved to the state folder (T-490): its dashboard shows only the projects it
listed since. 0.34.0 adds those to its own `default` workspace and names that older atm's server in
`atm service status` (T-501). `atm version` in a shell says which one is on PATH; upgrade the other, or stop its
server with its own `atm service stop`.

## What's here

| File | What it does |
|---|---|
| `release.sh <version> [--push]` | Checks the version is `Cargo.toml`'s and that `main` is clean and pushed, then tags `v<version>` (and pushes the tag with `--push`) |
| `keychain-macos.sh` | Imports the Developer ID certificate into a keychain of the release job's own; the macOS jobs run it first |
| `sign-macos.sh <binary>` | Signs a macOS build with the Developer ID (hardened runtime) and notarizes it; the release workflow runs it |
| `notarize-dmg.sh <dmg>` | Signs the app's DMG, notarizes it and staples the ticket to it (Tauri's bundler has signed and notarized the app inside) |
| `homebrew-formula.sh <version> <SHA256SUMS>` | Prints `Formula/atm.rb` for a release, from its checksums; the Homebrew workflow writes it to the tap |
| `homebrew-cask.sh <version> <SHA256SUMS>` | Prints `Casks/atm-app.rb` for a release: the universal DMG, the app's `atm` on PATH, caveats saying to remove the formula first and how to connect agents (nothing is installed into them) |
| `skills-export.sh <version> <checkout>` | Puts this version's `skills/atm-*` and the README's atm section into a checkout of `solo-swe/skills`, leaving other apps' parts alone ([ADR-002](../docs/adr/ADR-002-atm-public-skills-repo.md)); the Skills workflow commits, tags `atm-v<version>` and pushes |

The app's release build is `pnpm run app:bundle` in `dashboard/` (Tauri's CLI on `app/src-tauri/`, with
`tauri.release.conf.json`: the bundle, the DMG and the bundled CLI, which it expects at
`app/src-tauri/binaries/atm-<target>`; kept out of `tauri.conf.json`, as Tauri's build checks that file exists on
every `cargo build`). Locally, without signing: `cargo build --release -p atm-cli`, copy `target/release/atm` to
`app/src-tauri/binaries/atm-aarch64-apple-darwin`, then `pnpm run app:bundle --bundles app`.

## A release

1. The release PR is merged to `main`; the version is in `Cargo.toml`, with its `CHANGELOG.md` and `WHATS-NEW.md` entries
   (`check-versions.mjs`).
2. `packaging/release.sh 0.33.0 --push` tags `v0.33.0` and pushes the tag.
3. `.github/workflows/release.yml` builds atm for macOS (arm64, x86_64), Linux (x86_64, arm64, on Ubuntu 22.04 for an
   older glibc) and Windows (x86_64), signs and notarizes the macOS builds; builds the app as one universal DMG
   (arm64 and x86_64, its CLI a universal binary made with `lipo`), signed and notarized, its ticket stapled; and
   drafts a GitHub release with `atm-<version>-<target>.tar.gz` (`.zip` for Windows),
   `atm-app-<version>-universal.dmg` and `SHA256SUMS`.
4. Check the draft, then publish it. `.github/workflows/homebrew.yml` writes the formula and the cask to the tap, and
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

The release workflow reads the signing secrets only in the macOS jobs (the CLI's and the app's); only its last job,
which builds nothing, can write to the repository (its releases). The Homebrew workflow reads only the tap's deploy
key, the Skills workflow only the skills repo's. The app needs no new secret: it is signed and notarized with the
CLI's.
