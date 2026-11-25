+++
id = "01a21008-28c0-74dd-9626-ccca15f14fa1"
number = 182
title = "CompMD 0.10.0, measured before and after"
status = "approved"
tasks = ["ATM-780"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-25T00:08:19Z
updated = 2026-11-25T00:21:59Z
+++

atm on CompMD 0.10.0, as ATM-703 did 0.9.0, from the published packages and crates and the public repository at `v0.10.0`, never the local checkout.

## The pins
- `dashboard/package.json`: `@compmd/processor`, `render`, `sandbox`, `vite` at 0.10.0 (the lockfile updated on purpose; `@compmd/host` and `@compmd/registry` 0.10.0 with them). `pnpm-workspace.yaml`'s `minimumReleaseAgeExclude` names 0.10.0: released yesterday.
- `Cargo.toml`: `tauri-plugin-compmd` and `compmd-host` at `=0.10.0` (`compmd-pack` 0.10.0 in `Cargo.lock`).
- CompMD's runtime builds into `dist/assets/compmd/0.10.0/`.
- `docs/agent/compmd-llms.txt` (`atm docs compmd`): `spec/llms.txt` at `v0.10.0`, byte for byte (checked out from the tag, its blob `64e534e6` verified), under atm's comment line.

## What changed for a host
The spec's drafts 17 and 18: type in a box, image boxes, a document's own fonts, gradients, shadows, no-code components, `lang` and `dir` set by the renderer, a pack's own files. **No code change in atm:** everything builds, type-checks and passes. A document's own fonts show their fallback (atm's page policies allow no font source); the security doc records each point.

## One test made to wait for the page
`reviews.spec.ts` `openDiffs`: 0.10.0 is still drawing a file's code a moment after its line numbers show, so *the + dragged over three lines* failed 3 runs in 5 (it measured line 4, then the rows moved). It waits now for every CompMD block of the file to be `data-compmd="ready"`: 8 runs in 8 pass, and the whole spec.

## Measured before and after
`docs/review-performance.md`'s new section, medians of three: the timed tests unchanged within their spread; the real review (CompMD's CR-1) **opens about twice as fast** in both engines (Chromium first drawn 400 → 201 ms, blocked 183 → 100; WebKit 401 → 235, 413 → 217), WebKit's jumps block half as long. Sizes: WebAssembly +3.1%, `compmd.js` +17.7%, the page unchanged.

## Checks
`pnpm run check`, the unit tests (2136), the functional e2e suite, `cargo test`, clippy, `cargo deny check`, the CLI suite (926). Not run here: the Linux screenshots (the release task's).
