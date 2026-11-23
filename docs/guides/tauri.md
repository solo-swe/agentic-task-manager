# Tauri v2 guide

atm's own rules for the atm app, the desktop distribution proposed in
[better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md). The layers it builds on are in
[layered-architecture.md](../layered-architecture.md); the threat model in
[security-architecture.md](../security-architecture.md). Its Rust follows the [Rust guide](rust.md), its UI the
[Svelte guide](svelte.md).

## Version and builds

- **Tauri 2.11.1 or later** (CVE-2026-42184).
- **Single instance** (`tauri-plugin-single-instance`): opening atm again focuses the running window. The `atm` CLI is
  a separate process and unaffected.

## Commands

- **Commands are thin mappings to atm-lib**, one per atm-lib function ([layered-architecture.md](../layered-architecture.md)
  §5.1). They hold no domain logic: no validation, no queries, no business rules.
- Commands are `async` and receive the user's `Workspaces`, opened once, from `tauri::State`. They mirror the REST
  API's v2 routes and answer the same, which a test checks per read (ADR-003 §5).
- Arguments and results are serde DTOs. Errors are typed and serialized as a tagged enum the UI can match on, never as
  plain strings.
- **TypeScript types are generated** with `tauri-specta`: commands and DTOs are exported to a generated file that the
  UI's typed client wrapper builds on. Generated files are never edited by hand.

## Streams

- **One `tauri::ipc::Channel` per subscription** for live data, delivering messages in order, and an `unsubscribe`
  command to end it. A subscription is atm-lib's (`Workspace::subscribe`, `Member::subscribe`: the stores' own,
  `Store::subscribe`, T-555), held until it ends; the app watches no folder itself and doesn't know whether a store
  pushes or polls. It says only what changed (a workspace, a project); the page reads it again.
- No global events for data changes.
- **The page reacts, it doesn't poll** (T-506): in the app (`dashboard.live`) the root layout subscribes to the
  workspace and the open project (`lib/api/changes.ts`'s `watchChanges`), reloads on a message, and ends the
  subscriptions when it moves to another workspace or project. A change the watcher can't see (a document outside the
  task folder, a missed event) shows when the window regains focus, and on the Reload button. The refresh settings are
  the browser's only (`dashboard.polls`), until `atm serve` pushes changes too (T-507).
- **A page that goes away doesn't say so:** sending on its channel doesn't fail, the message only finds no callback.
  So the window's page loading again (`on_page_load`, `PageLoadEvent::Started`: a reload, ⌘R) ends every
  subscription (`AppState::unwatch_all`); the new page makes its own.

## The dashboard in the window

- **One transport switch** (`dashboard/src/lib/api/http.ts` `browserFetch`): in the app (`isTauri()`), the dashboard's
  own requests go to `api/ipc.ts`'s `ipcFetch`, which maps each URL (the v1 routes as `default`'s, the v2 ones) to its
  command and answers with a `Response`; nothing above the client layer knows which it got.
- **Images come through the commands too**, as `data:` URLs (`api/images.ts`), which the page's CSP already allows;
  no `blob:` or custom protocol. A document's relative images show their alt text in the app for now.
- **One window**, switching workspaces in place (Hakan, T-457).
- **Deep links** (`atm://…`, T-459) are addresses for the page's router: the app checks them (`src/links.rs`), the
  page checks them again (`lib/api/links.ts`) and navigates; never fetched or run. On macOS the scheme is in
  `Info.plist` and the links come as `RunEvent::Opened`, without a plugin; Windows and Linux need the deep-link plugin
  when the app ships there.
- **External links open in the user's browser** (T-551): the webview opens none itself (a `target="_blank"` link
  does nothing). One click listener on the page (`routes/+layout.svelte`, with the new-tab clicks; `lib/external-links.ts`)
  takes every click on an `https:`, `http:` or `mailto:` link, before the link's own handlers, and hands the URL to
  the app's `open_url` (`api/shell.ts` `openUrl`); the menu's links go there too (`menu-actions.ts` `pageActions.open`).
  The app checks the URL again (`src/external.rs`: those three schemes, a host for the web ones, a typed error for
  anything else) and opens it with the opener plugin from Rust. The page has none of the plugin's permissions and no
  `@tauri-apps/plugin-opener`, and the plugin's own click script is off (`open_js_links_on_click(false)`). Links within
  the page (its addresses, `?task=`, `#…`, a document's relative links) stay its own. A new external link needs
  nothing: an anchor with `target="_blank" rel="noopener noreferrer"`, as in the browser, works in both.

## Security

- **Capabilities are least privilege:** each window gets only the commands and plugin permissions it uses.
- **Rendered task text never calls the app:** Tauri counts every scheme the app registers as local, so a window's
  capability (`"local": true`) covers CompMD's frames (`compmd:`) and atm's files (`atm:`) as well as the app's page:
  leaving them out of a capability doesn't keep them out. The app's invoke handler is wrapped in
  `navigation::guarded`, which answers only a call whose `Origin` is the app's page and refuses one from the frames or
  the files (ATM-677; `a_call_from_compmds_frames_is_refused_though_the_capability_covers_it` shows both). Register a
  new command through that handler, never around it
  ([compmd-rendering-design.md](../compmd-rendering-design.md)).
- **Webviews stay on the app's origins:** `navigation::plugin` guards every webview's navigation (the windows made
  from the configuration too): the app's page, CompMD's frames, atm's files and `about:`, what the policy's
  `frame-src` and `script-src` load; anything else is refused, and a link elsewhere opens in the browser through
  `open_url`. A new origin the page loads goes in its list and the policy together.
- **Strict CSP for the shell:** the dashboard server's policy ([security-architecture.md](../security-architecture.md)
  I13, I26: own scripts by hash, no remote or inline scripts, no `blob:`, `data:` or `'self'` in `script-src`,
  `frame-ancestors 'none'`), with CompMD's two folders in `script-src` and `connect-src` and the frames' origin in
  `frame-src` (below), and `connect-src` otherwise only Tauri's IPC. What Tauri does to the policy (T-502):
  - it puts it on **every HTML file its own protocol serves**, so CompMD's files don't come from it (below);
  - it adds a nonce to `style-src`, which makes the browser ignore `'unsafe-inline'` (every `style` attribute
    refused): `dangerousDisableAssetCspModification: ["style-src"]` leaves it as written;
  - **every debug build** (`cargo run`, `cargo tauri dev`, the tests) uses `devCsp`, not `csp`: the same policy plus only
    the CLI's live-reload socket. A test keeps the two in step.
- **The dashboard is embedded once, before the crate compiles** (ATM-559): `generate_context!` compresses its files
  into a cache in the build folder, writing a file in place when it is missing, and `cargo test` and
  `clippy --all-targets` compile the library twice at once, so one could embed the page half-written. `build.rs` runs
  Tauri's code generation first (`fill_tauri_asset_cache`), so the macro finds every file whole. It needs the same
  `tauri-codegen` as tauri-macros: when Tauri is updated, move the workspace's `tauri-codegen` pin to tauri-macros'
  version (a test fails while there are two in `Cargo.lock`).
- **CompMD from two schemes** ([ADR-006](../adr/ADR-006-compmd-0-7-bundle-one-build-for-both-hosts.md), CompMD 0.7.0's
  [Tauri chapter](https://compmd.dev/guides/developers/a-tauri-app/)):
  - **CompMD's bundle** (the sandbox's frames under their hash, the packs by their place) from CompMD's own plugin
    (crates.io, pinned to the `@compmd/*` version: bump them together), on `compmd://localhost/`
    (`http://compmd.localhost` on Windows and Android). The dashboard's build writes the bundle
    (`vite.compmd.config.ts`, `out` and `source`), `build.rs` embeds it (`tauri_plugin_compmd::build::bundle`, before
    Tauri's step), and `with_plugins` registers `tauri_plugin_compmd::init(include_bundle!())`, which serves it by
    CompMD's contract and gives the page the origin (`__COMPMD__`): the page names none, and fetches no pack;
  - **what atm ships for the page** (`compmd.js` and its WebAssembly) from the app's own `atm:` scheme
    (`src/assets.rs`; `atm://localhost`, `http://atm.localhost` on Windows and Android), at the paths `atm serve` uses
    and with its headers for scripts (`tests/scripts/compmd-host-headers.json`). `build.rs` embeds them from the
    dashboard's build, and fails without it. The page gets that origin from `convertFileSrc('', 'atm')`
    (`compmdOrigin` in `lib/compmd.ts`).

  The window's policy names those two folders in `script-src` and `connect-src`, never an origin. Don't serve CompMD
  from Tauri's own protocol, and don't loosen the window's policy for it (`blob:` in `script-src` is CVE-2026-95626's
  case). On macOS and Linux a sandboxed frame's `'self'` matches nothing on a custom scheme (WebKit takes it from the
  URL only for http), which is why the frames are CompMD's: their policies name their scheme.
- **Nothing CompMD does fails silently:** `CompMd.svelte` shows a note when CompMD can't load or a component draws
  nothing, and logs the cause; keep both when changing it.
- **Check the app in a window, both ways:** `cargo tauri dev` (the CLI's server) and `cargo run -p atm-app`
  (`tauri://localhost`, as the bundle), before a task that changes the window is done.
- **`cargo tauri dev` follows the dashboard's sources:** it starts `pnpm run watch:page` (`beforeDevCommand`,
  `dashboard/build/watch-page.ts`), which builds the page again a few seconds after a change in `dashboard/src/`, the
  schemas or `docs/help.md`; the CLI's server serves `dashboard/dist/` and reloads the window when the page is
  written. No restart for a page change; a Rust change, or CompMD's runtime (`pnpm run build`, embedded by
  `build.rs`), still needs one. `cargo run -p atm-app` shows the page as it was last built. (The command finds the
  dashboard with `git rev-parse`, in `sh`: on Windows, run `pnpm run watch:page` in `dashboard/` yourself.)

## Platform features

- Platform features (notifications, files, …) are host services: traits in atm-lib, implemented in the app with Tauri
  plugins.
- Any new plugin is a new dependency and is proposed to the user first ([Rust guide](rust.md)). A plugin is used from
  Rust, behind the app's own commands, and the page gets none of its permissions (the notifications, the updater, the
  opener).
- Installing the agent skills is the same code as `atm agents install`
  ([better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md) §4); the app's button calls it, never a second
  implementation.

## Tests

- **Every case, not a coverage number** (AGENTS.md, Testing conventions): each command with each class of result
  it can give, refusals included.
- **Domain behaviour** is tested in atm-lib, not through Tauri.
- **Command wiring** gets tests with Tauri's mock runtime: each command reaches the right atm-lib call, and errors
  serialize as expected.
- **End to end:** the UI runs under Playwright with mocked Tauri IPC (`tests/e2e/helpers.ts` `asApp()`, the internals
  `@tauri-apps/api/mocks` sets, backed by the e2e fixture's data and atm's own task views; `tests/e2e/app.spec.ts`),
  and the transport's mapping has unit tests on `@tauri-apps/api/mocks` (`tests/unit/ipc.test.ts`). WebDriver (`tauri-driver`) doesn't support macOS. The real app
  is checked with a manual QA checklist.
