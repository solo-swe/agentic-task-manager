+++
id = "01a1f4f1-a0ed-7296-a91c-b4de7fce7cfb"
number = 59
title = "CompMD 0.8.0, measured before and after"
status = "approved"
tasks = ["ATM-648"]
author = "claude:0f56a916"
reviewers = []
created = 2026-11-19T17:53:58Z
updated = 2026-11-20T09:26:40Z
+++

atm moves to CompMD 0.8.0. The review page gets faster, most of all in WebKit (the app), where opening a review no longer freezes for 1.7 s.

## Change
- **The pins:**
  - `@compmd/processor`, `render`, `sandbox` and `vite` at 0.8.0 (`dashboard/package.json`, the lockfile);
  - `tauri-plugin-compmd` and `compmd-host` at `=0.8.0` (`Cargo.toml`), with `compmd-pack` 0.8.0 in `Cargo.lock`.
- **One new package, `@compmd/host` 0.8.0**, CompMD's own, which `@compmd/vite`'s dev server serves its bundle with. `pnpm-workspace.yaml`'s `minimumReleaseAgeExclude` names the 0.8.0 packages and it, as it did for 0.7.1, since they were released today by CompMD's author.
- **No policy changes:** 0.8.0 highlights with Oniguruma's WebAssembly outside Chromium, which needs `'wasm-unsafe-eval'`. The dev server, `atm serve` and the app already allow it.
- **CompMD's guide for agents** (`atm docs compmd`) is its `spec/llms.txt` at `v0.8.0`, from the public GitHub repository. It differs from the copy before only by one link.
- **The jump test** (`reviews.spec.ts`) now holds a jump on a long review at no restyles at all: 0.8.0 shares one style sheet per document.
- **`compmd-runtime.ts`:** only its comment changes (atm's one sandbox and one theme observer stay).
- **Docs:**
  - the 0.42.0 entry of the security doc;
  - the measurements in `docs/review-performance.md` (*CompMD 0.8.0*).
- **The app's Reload test** (`app.spec.ts`) no longer fails when a look lands while the page reloads. It is a race in the test, seen once in the full run; it also exists on 0.7.1.

## Measured (0.7.1 → 0.8.0)
**The real review** (CompMD's CR-1, 94 files):

| | Chromium | WebKit |
|---|---|---|
| Opening: blocked | 167 → 83 ms | **1,712 → 92 ms** |
| A jump to the top: blocked | 0 → 0 ms | **1,594 → 78 ms** |
| A jump to the end: blocked | 0 → 0 ms | 715 → 59 ms |
| Scrolled to the end: the last file drawn | 2,264 → 1,978 ms | 2,188 → 2,013 ms |

**The timed tests** (medians of three):
- a jump's restyles 10 sheets and 28 style reads → **none**;
- the long review settles in 101 → 68 ms;
- its slowest frame is 43 → 23 ms;
- its DOM nodes after down and back are 21,347 → 22,051;
- the pointer is unchanged;
- tab switches and reloads render no CompMD and are within their spread.

**Sizes:** CompMD's WebAssembly is +5% (2.81 → 2.96 MB), `compmd.js` 83 → 99 KB, and the dashboard's page is unchanged.

## Not taken up here
0.8.0 also has `view.lines()` and `view.lineHeights()` for code blocks, which would let the diff viewer's Wrap work in Split (ATM-588). That is a feature of its own, for a later task.

## Checks
- `pnpm run verify`: the checks, unit tests 1,782 (96.7% of lines), the CLI suite 833, the functional e2e 526, and the Linux screenshots 84, none changed;
- `cargo test` 918, clippy, `cargo deny`;
- the timed tests three times on each version, one worker.
