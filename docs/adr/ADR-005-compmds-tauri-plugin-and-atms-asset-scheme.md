# ADR-005: CompMD's Tauri plugin for the frames, atm's own scheme for what atm ships

- **Status:** accepted, 2026-11-09; replaces [ADR-004](ADR-004-compmd-in-tauri-through-its-own-protocol.md)
- **Decided by:** Hakan Serce
- **Context:** T-503, T-508, ADR-004 (the problem, its check and its alternatives, which still hold), CompMD's
  [Tauri plugin design](https://github.com/comp-markdown/compmd/blob/main/docs/tauri-plugin.md),
  [security-architecture.md](../security-architecture.md) I13 and I26

## Problem

ADR-004 served the dashboard's whole CompMD build on a `compmd:` scheme, from a plugin atm wrote for CompMD to take.
Its check found that WebKit reads `'self'` in a sandboxed frame on a custom scheme as matching nothing, so the frames
couldn't load their runtime: a fix only CompMD could make. CompMD 0.6.0 makes it, and ships the plugin itself,
`tauri-plugin-compmd` 0.6.0, but in a different shape:

- it **embeds its own frames**, built when CompMD is released, at a path named by a hash of their content
  (`compmd://localhost/frames/<hash>/`), served for good (`max-age=31536000, immutable`), and their policies name
  that scheme;
- it gives the page their `base` itself (an initialization script), and serves **nothing else**: not atm's
  `compmd.js` (the processor and the sandbox), its WebAssembly, or the packs' built modules (the Mermaid pack).

Those still need a place the window's policy allows. Tauri's own protocol isn't one: `script-src` has no `'self'`
(I13, I26), and Tauri puts the window's policy on every HTML file it serves.

## Decision

1. **The sandbox's frames are CompMD's:** `tauri_plugin_compmd::init()`, the crate from crates.io at the version of
   `@compmd/sandbox` (they are released together; a page of another version gets the sandbox's `error` naming both).
   The page creates the sandbox with no `base`. atm's own crate, `crates/tauri-plugin-compmd/`, goes.
2. **What atm ships for its page comes from a scheme of atm's own, `atm:`** (`app/src-tauri/src/assets.rs`), at the
   paths `atm serve` gives it (`/assets/compmd/<version>/…`): `compmd.js`, its WebAssembly and the packs' built
   modules, embedded by the app's `build.rs`, and nothing else (no HTML), `GET` and `HEAD` only, a `404` or `405`
   that says why, with CORS `*`, `nosniff` and `no-cache`. The deep links share the scheme (T-459) but never meet it:
   macOS hands those to the app, the handler answers only the window's own loads, and no asset address is a page.
3. **The window's policy names folders, not origins:** `script-src` and `connect-src` allow
   `atm://localhost/assets/compmd/` and `compmd://localhost/frames/` (and their `http://….localhost` forms for Windows
   and Android); `frame-src` the `compmd:` origin. Still no `blob:`, `data:` or `'self'`; `frame-ancestors 'none'`.
4. **The packs' modules reach the frames as code:** the page fetches each one a text needs from `atm:`, checked
   against the hash `@compmd/vite` took (the fetch's `integrity`), and gives the sandbox the `Blob`
   (`modules: () => ({ code })`), as CompMD's example app does. One that can't be had is logged, and its component
   shows its fallback with the sandbox's `error`. In the browser nothing changes: `atm serve` serves the frames and
   the modules, and the sandbox loads them by URL.
5. **Nothing fails silently,** as ADR-004 decision 4 said; CompMD 0.6.0 now also reports a frame that never loads its
   runtime.

## Alternatives considered

- **The app's own files, by path** (`tauri://localhost/assets/compmd/` in `script-src`): no handler of atm's own, as
  the dashboard's build already holds the files. Not chosen: the page's scripts would come from Tauri's protocol,
  which serves every file of the build, and the policy would name the app's own origin at all.
- **Keep atm's plugin beside CompMD's:** two plugins for one job, and atm's would still serve frames CompMD now
  serves better (hashed paths, policies that work on WebKit).
- **The app builds its own frames** (`Builder::frames_dir`, `buildHashedFrames`): needed only for libraries beyond
  the runtimes or another scheme; atm needs neither, and it would add a Node step and a resource to bundle.

## Risks

- **WebKit's custom-scheme cache** (T-508) no longer reaches the frames, whose paths name their content. It could
  still reach `atm:`'s files, at the paths `atm serve` uses: a pack's module is checked by its hash, so a stale one
  fails visibly; `compmd.js` is at CompMD's version, so a stale one would be the same CompMD rebuilt. T-508 checks it
  in the app, and if needed names `atm:`'s folder by a hash too.
- **A new entry point,** `atm:`: it serves only the embedded files, and a test checks that no page on it can call the
  app's commands (I26).
- **CompMD's plugin and atm's page drift apart:** the crate is pinned to the exact version of the packages
  (`=0.6.0` and `0.6.0`), bumped together.

## Consequences

- atm depends on `tauri-plugin-compmd` (crates.io, MIT, CompMD's); ADR-004's decision 5 is done.
- `crates/tauri-plugin-compmd/` is gone; the app gains `assets.rs` and its `build.rs` embedding; `compmd.ts` gains
  `sandboxOptions` and `fetchPackModules`; the window's CSP and `devCsp` name the two folders.
- The header table `tests/scripts/compmd-host-headers.json` is `atm serve`'s, and the app's `atm:` files are checked
  against its script and WebAssembly rows; its frame rows are `atm serve`'s alone.
