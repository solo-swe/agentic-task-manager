# ADR-004: CompMD in a Tauri app, through a protocol of its own

- **Status:** accepted, 2026-11-08
- **Decided by:** Hakan Serce
- **Context:** T-502, T-503, [tauri-compmd-design.md](../tauri-compmd-design.md) (the design this records; its §2
  has the facts, with their sources), [compmd-rendering-design.md](../compmd-rendering-design.md) §2 and §3c,
  [security-architecture.md](../security-architecture.md) I13 and I26

## Problem

The atm app (Tauri) shows the dashboard, which renders task text and documents with CompMD. In the app no Mermaid
diagram draws, and earlier builds showed raw Markdown and a broken layout, without saying why.

CompMD expects each document to have **its own policy**: the page a strict one, each sandbox frame only its own
`<meta>` policy plus `sandbox allow-scripts`, and CompMD's files readable from anywhere, since the frames have an
opaque origin. `atm serve` gives it exactly that. Tauri puts **the window's one CSP on every HTML file it serves**,
CompMD's frames included, edits it at build time, and answers with its own CORS header. Under the window's policy, a
frame's component code (a `blob:` module) is refused. No Tauri setting gives one file a policy different from
another's.

This is the first time CompMD runs in a Tauri app, so the choice is also how any Tauri app that uses CompMD should do
it: straightforward for its developer, with Tauri's documented APIs only, and no failure that shows nothing.

## Decision

1. **CompMD's built files are served on a URI scheme of their own, `compmd:`, by a small Tauri plugin**
   (`tauri-plugin-compmd`), with the headers CompMD needs from any host (the design's §3, H1 and H2):
   `Access-Control-Allow-Origin: *`, `nosniff`, and `sandbox allow-scripts` on frame documents, nothing else on
   them. The plugin serves only the files of the folder it embeds, and answers anything else with a `404` or `405`
   that says why. This is how Tauri ships its own `asset:` and `isolation:` protocols.
2. **The page keeps Tauri's policy, strict,** naming CompMD's origin in `script-src`, `frame-src` and `connect-src`
   (`compmd://localhost`, and `http://compmd.localhost` on Windows and Android). No `blob:`, `data:` or `'self'` in
   its `script-src`; `frame-ancestors 'none'` returns.
3. **The page gives CompMD a full `base`** on that origin (Tauri's `convertFileSrc('', 'compmd')`), which CompMD's
   sandbox already accepts; the browser keeps its relative one. The files keep the paths they have under `atm serve`.
4. **Nothing fails silently:** the page listens for CompMD's `error` event and for `compmd.js` failing to load, and
   shows both at the text, with the cause in the console.
5. **The plugin belongs with CompMD.** atm builds it first as a crate of its own in atm's workspace, with no
   dependency on atm's crates, so it can move to CompMD as it is.
6. **Before the change, a check by hand** (the design's §4.7): WebKit lets the opaque frame load modules from
   `compmd://`, Tauri leaves the scheme's responses alone, and every URL CompMD makes comes from `base`. If WebKit
   refuses, this ADR is reopened.

## Alternatives considered

The design's §5 compares them in full, with who uses each.

- **A response hook on Tauri's protocol** (`on_web_resource_request`): no new origin, no page change. Discarded: it
  exists only on a window built in code, so every app using CompMD would move its window out of `tauri.conf.json`;
  a file's headers would be decided in two places, Tauri's and the hook's, which is hard to trace when a frame fails;
  and `cargo tauri dev` serves the files from the CLI's server, which the hook never sees.
- **`blob:` in the window's policy:** one config line. Discarded: it loosens the page, not only the frames, and is
  the case CVE-2026-95626 describes (`blob:` in `script-src` cancels Tauri's nonce protection).
- **No `csp` in Tauri, a `<meta>` policy per file:** discarded, as the page would be built per origin or allow
  `'self'`, Tauri's hashes and nonces would go, and CompMD's files still couldn't get CORS `*`.
- **`dangerousDisableAssetCspModification: true`:** discarded, since it doesn't stop the policy going on every file.
- **The page from a local server, Mermaid outside the sandbox, Tauri's isolation pattern:** discarded, as they
  reverse T-457's one-window IPC design, put untrusted markup in the page, or don't touch the frames' policy.

## Risks

- **WebKit refuses the opaque frame's module loads from a custom scheme.** It works over `http:` in every browser;
  WKWebView with a custom scheme is unchecked. Mitigation: the check comes first (decision 6); no alternative here
  avoids a custom scheme on macOS, so the answer reopens this ADR rather than a quiet workaround.
- **A new entry point:** a protocol any page in the webview can request. Mitigation: it serves only the embedded
  folder's files, by an allow-list of paths and types, `GET` only; the security document lists it (I26); a test
  checks that `compmd:` can't call the app's commands.
- **The plugin and `atm serve` drift apart** on the headers. Mitigation: both are tested against the same table (H1,
  H2).
- **WebKit caches custom-scheme responses** (macOS, iOS): a CompMD file atm rebuilds at the same path (the same CompMD
  version, other packs or configuration) could keep running its old version after an upgrade, the page and its frames
  then on different frame protocol versions. Mitigation: T-508 checks it in the app, and if so makes the plugin's
  default content-hashed paths (or validators WebKit honours); the result is recorded here.
- **CompMD changes what it loads** (a new file type, a URL not from `base`). Mitigation: the plugin's tests and the
  by-hand check in both builds, which the Tauri guide makes part of any task that changes the window.

## Consequences

- A Tauri developer using CompMD adds the plugin, one origin in three CSP lines, and the `base`; windows, Tauri's own
  policy handling and `cargo tauri dev` stay as they are.
- In atm: a new crate, `crates/tauri-plugin-compmd/`; the app's CSP and `devCsp` change (the latter only adds the
  CLI's live-reload socket); `dashboard/src/lib/compmd.ts` gives the app a full `base`; `CompMd.svelte` shows
  CompMD's errors, in the browser too.
- T-503 builds it, after Hakan's review of the design; T-502's config changes stay where the design's §2.4 says.
- If CompMD takes the plugin, atm depends on CompMD's crate instead, and this ADR's decision 5 is done.
