# ADR-006: CompMD 0.7.0's bundle, one build for both hosts, served by its own contract

- **Status:** accepted, 2026-11-13; replaces [ADR-005](ADR-005-compmds-tauri-plugin-and-atms-asset-scheme.md)
- **Decided by:** Hakan Serce ("adopt 0.7.0 properly", T-524)
- **Context:** T-524, ADR-005 (and ADR-004 before it), CompMD 0.7.0's
  [developer guide](https://compmd.hakanserce.com/guides/developers/), chapter 19 (a Tauri app), and its designs
  [hosting](https://github.com/comp-markdown/compmd/blob/main/docs/hosting.md) and
  [the Tauri plugin](https://github.com/comp-markdown/compmd/blob/main/docs/tauri-plugin.md);
  [security-architecture.md](../security-architecture.md) I13 and I26

## Problem

CompMD 0.7.0 changes how a host serves it. Every host now shares four layers (the configuration, the **bundle**, the
pack store and the **serving contract**) and writes only an adapter: where the origin is, how a request reaches
`respond`, how the page learns the origin. For atm that changes three things ADR-005 decided:

- `tauri-plugin-compmd` 0.7.0 **no longer brings frames of its own**: the app's build embeds the bundle the page's build
  wrote (`build::bundle` in `build.rs`, `init(include_bundle!())`), and the plugin serves its frames and its packs on
  `compmd:` and gives the page the origin (`globalThis.__COMPMD__`);
- the **packs' modules load by their place** (`packs/<name>@<version>/<path>`) from that origin, checked by their hash,
  so the page no longer fetches them and hands them over as code (ADR-005 decision 4);
- the bundle has **one layout and one contract** for every host: `frames/<hash>/` and `packs/<place>`, `GET` only, a
  `404` with why for anything else (the manifest included), and fixed headers (`sandbox allow-scripts`, CORS `*`,
  `Access-Control-Expose-Headers: *`, `nosniff`, cached for good since the paths name their content), in
  `compmd-host`'s `respond`.

## Decision

1. **One build, one bundle, for `atm serve` and the app.** `dashboard/vite.compmd.config.ts` builds `compmd.js` (the
   processor and the sandbox, with its WebAssembly) and writes CompMD's bundle beside it with `out`
   (`dist/assets/compmd/<version>/compmd/`: `manifest.json`, `frames/<hash>/`, `packs/<place>`), built for the scheme
   the plugin serves it on (`source: "compmd://localhost/"`: WebKit reads `'self'` in a sandboxed frame on a custom
   scheme as nothing; on http the source is harmless, as CompMD's design says of Windows). `virtual:compmd` then names
   no origin: the host gives it.
2. **The app embeds that same bundle for CompMD's plugin:** its `build.rs` calls `tauri_plugin_compmd::build::bundle`
   on it, before Tauri's step, and `with_plugins` registers `tauri_plugin_compmd::init(include_bundle!())`. The plugin
   serves it on `compmd:` and sets `__COMPMD__.origin` (`compmd://localhost`, `http://compmd.localhost` on Windows).
3. **`atm serve` answers the bundle with `compmd-host`'s `respond`**, the contract's one implementation, at
   `/assets/compmd/<version>/compmd/`: `atm-api`'s `build.rs` embeds the bundle whole and `assets.rs` hands its paths
   to `respond` (with no store: atm fetches no registry pack). Its other assets (`compmd.js`, the WebAssembly) are
   served as before.
4. **The page gives the sandbox only the origin, and only in the browser:** `sandboxOptions` is
   `virtual:compmd`'s `sandbox` with `page: "host"`, plus `origin` (`/assets/compmd/<version>/compmd/`) under
   `atm serve`; in the app the sandbox reads the plugin's `__COMPMD__`. `fetchPackModules`, the module code map and
   `sandboxBase` go.
5. **`atm:` stays for what atm ships its page, now only `compmd.js` and its WebAssembly** (ADR-005 decision 2; the
   plugin serves frames and packs only), at the paths `atm serve` gives them, with its headers.
6. **The window's policy is unchanged:** it already names what `cspFor` gives (`frame-src` the `compmd:` origin,
   `script-src` and `connect-src` its `frames/`, never its `packs/`, `frame-ancestors 'none'`), with `atm:`'s folder for
   `compmd.js`.

## Alternatives considered

- **A second build for the app** (`compmd.js` and a bundle of its own, `out: "app/src-tauri/compmd"`), as CompMD's
  example does with one page per app. Tried first: it doubled the build and the WebAssembly on disk, and the browser's
  bundle would still be served by atm's own rules. One bundle with the origin given by the host is what CompMD's
  design means by `out` (hosting, decision 9).
- **The bundle in the web layout (no `out`) for `atm serve`**, served by atm's own file route: no manifest in the
  output, so `atm serve` couldn't use `respond`, and its headers (`no-cache`, no `Access-Control-Expose-Headers`) would
  differ from the contract's.
- **The app's own files by path** (`tauri://localhost/assets/…` in `script-src`): rejected again, as in ADR-005.

## Risks

- **`compmd-host` is a new dependency of `atm-api`** (CompMD's own crate, MIT, the plugin already brought it): pinned
  `=0.7.0` with the packages and the plugin, bumped together.
- **The frames' policies name `compmd://localhost/` under `atm serve` too.** On an http origin CompMD's frames still
  allow their own origin, so the extra source matches nothing and changes nothing; the browser e2e suite draws the
  diagrams under it.
- **A stale bundle in the app:** the frames' path names their content, so a rebuild is a new path; packs are named by
  their version.

## Consequences

- `atm serve` sends the contract's headers for the bundle (`tests/scripts/compmd-host-headers.json`'s `bundle`
  rows, checked in `serve.test.mjs`); the app's `atm:` its `files` rows (checked in the app's tests).
- `dashboard/src/lib/compmd.ts` loses the code ADR-005 decision 4 added; `api/http.ts` its `assetFetch`.
- `docs/security-architecture.md` I26 and `docs/guides/tauri.md` describe this.
