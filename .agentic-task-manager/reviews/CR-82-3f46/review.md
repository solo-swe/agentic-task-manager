+++
id = "01a1f8f8-7bc0-75f7-a4bc-0f86a1893f46"
number = 82
title = "CompMD 0.9.0, measured before and after"
status = "approved"
tasks = ["ATM-703"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-20T12:39:56Z
updated = 2026-11-20T15:51:59Z
+++

atm moves to CompMD 0.9.0, released today. It measures the same as 0.8.0 on the review pages, with two improvements: a reload of the Progress tab (67 → 52 ms, every run) and, in WebKit, a jump to the end of the real review (107 → 51 ms blocked).

## Change
- **The pins:** `@compmd/processor`, `render`, `sandbox` and `vite` at 0.9.0 (`dashboard/package.json`, the lockfile); `tauri-plugin-compmd` and `compmd-host` at `=0.9.0` (`Cargo.toml`), with `compmd-pack` 0.9.0 in `Cargo.lock`. **No new package.** `pnpm-workspace.yaml`'s `minimumReleaseAgeExclude` names the 0.9.0 packages, released today by CompMD's author, as for 0.8.0.
- **What 0.9.0 changes for a host** (from each package's README; there is no changelog file at the tag):
  - The frames no longer highlight code. They ask the host's page over the sandbox protocol's version 2, and a page in frame mode loads the frames' `highlight.js`. atm's policies already allow the frames' scripts everywhere, so they need no change.
  - The frames' own policies drop `'wasm-unsafe-eval'`.
  - The processor lists only the components a document uses: a text without a diagram loads no Mermaid. **One unit test changed** for this (`compmd-processor.test.ts`): a text that defines its own component now gives the sandbox nothing at all, where it gave Mermaid before. A new test checks that Mermaid is given only to a text that uses it.
  - "Every origin" for a host's own documents is new; atm doesn't use it.
- **CompMD's guide for agents** (`atm docs compmd`): `spec/llms.txt` is unchanged at `v0.9.0`. Only the header of our copy names the new version.
- **Docs:** the 0.42.0 entry of the security doc, and the measurements in `docs/review-performance.md` (*CompMD 0.9.0*).

## Measured (0.8.0 → 0.9.0)
**The timed tests** (Chromium, one worker, medians of three):

| | 0.8.0 | 0.9.0 |
|---|---|---|
| A jump's restyles: sheets / styles read | 0 / 0 | 0 / 0 |
| The long review: first file drawn / settled | 36 / 70 ms | 37 / 71 ms |
| The long review: slowest frame after settling | 24 ms | 23 ms |
| The long review: DOM nodes after down and back | 22,116 | 22,116 |
| A file far down a large review | 85 ms | 87 ms |
| The pointer: median / slowest | 33 / 34 ms | 33 / 34 ms |
| Tab switches: Kanban / Progress / Agents / Graph / List | 21 / 47 / 68 / 79 / 79 ms | 24 / 48 / 67 / 79 / 78 ms |
| A reload: Graph / Kanban / Progress | 60 / 51 / 67 ms | 53 / 51 / **52** ms |

All within their run-to-run spread, except the Progress tab's reload (66–68 ms on 0.8.0, 52–54 on 0.9.0).

**The real review** (CompMD's CR-1, one run per engine). This ran as part of the full e2e suite, through `real-review.local.json`. It reads CompMD's review files through atm and reports only timings. The 0.8.0 run was before ATM-693's jump fix, so the jump rows compare both changes:

| | Chromium | WebKit |
|---|---|---|
| Opening: blocked | 83 → 83 ms | 102 → 92 ms |
| Scrolled to the end: the last file drawn | 2,048 → 1,929 ms | 2,022 → 2,017 ms |
| A jump to the top: blocked | 0 → 0 ms | 74 → 70 ms |
| A jump to the end: blocked (long frames) | 0 → 0 ms | 107 → 51 ms (2 → 1) |

**Sizes:** CompMD's WebAssembly 2,960,441 → 2,967,257 bytes (+0.2%); `compmd.js` 98,693 → 99,956 bytes; the frames' runtime 81,004 → 82,429 bytes; the dashboard's page unchanged (1,084,296 bytes, within budget).

## Checks
- the checks;
- the unit tests: 1,822;
- the whole e2e suite: 570, Mermaid diagrams in the app and the browser included;
- `cargo test` (920), clippy, `cargo deny` (advisories, bans, licences and sources ok).

The Linux screenshots are left to the release task, as usual.
