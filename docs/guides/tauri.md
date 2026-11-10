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
  command to end it. A subscription watches the folders the store reports (`Store::watch_folders`, not their
  subfolders) with `notify`, debounced, and says only what changed (a workspace, a project); the page reads it again.
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

## Security

- **Capabilities are least privilege:** each window gets only the commands and plugin permissions it uses.
- **Rendered task text is never given a capability:** CompMD's sandbox origin is not listed in any capability
  ([compmd-rendering-design.md](../compmd-rendering-design.md)).
- **Strict CSP for the shell:** the dashboard server's policy ([security-architecture.md](../security-architecture.md)
  I13, I26: own scripts by hash, no remote or inline scripts, no `blob:`, `data:` or `'self'` in `script-src`,
  `frame-ancestors 'none'`), with CompMD's two folders in `script-src` and `connect-src` and the frames' origin in
  `frame-src` (below), and `connect-src` otherwise only Tauri's IPC. What Tauri does to the policy (T-502):
  - it puts it on **every HTML file its own protocol serves**, so CompMD's files don't come from it (below);
  - it adds a nonce to `style-src`, which makes the browser ignore `'unsafe-inline'` (every `style` attribute
    refused): `dangerousDisableAssetCspModification: ["style-src"]` leaves it as written;
  - **every debug build** (`cargo run`, `cargo tauri dev`, the tests) uses `devCsp`, not `csp`: the same policy plus only
    the CLI's live-reload socket. A test keeps the two in step.
- **CompMD from two schemes** ([ADR-005](../adr/ADR-005-compmds-tauri-plugin-and-atms-asset-scheme.md)):
  - **the sandbox's frames** from CompMD's own plugin, `tauri_plugin_compmd::init()` (crates.io, pinned to the
    `@compmd/sandbox` version: bump them together), on `compmd://localhost/frames/<hash>/` (`http://compmd.localhost`
    on Windows and Android); it gives the page their `base`, so the page names none;
  - **what atm ships for the page** (`compmd.js`, its WebAssembly, the packs' built modules) from the app's own `atm:`
    scheme (`src/assets.rs`; `atm://localhost`, `http://atm.localhost` on Windows and Android), at the paths `atm serve`
    uses and with its headers for scripts (`tests/scripts/compmd-host-headers.json`). `build.rs` embeds them from the
    dashboard's build, and fails without it. The page gets the origin from `convertFileSrc('', 'atm')`
    (`compmdOrigin`), and gives the frames the packs' modules as code it fetched and checked (`sandboxOptions`,
    `fetchPackModules` in `lib/compmd.ts`).

  The window's policy names those two folders in `script-src` and `connect-src`, never an origin. Don't serve CompMD
  from Tauri's own protocol, and don't loosen the window's policy for it (`blob:` in `script-src` is CVE-2026-95626's
  case). On macOS and Linux a sandboxed frame's `'self'` matches nothing on a custom scheme (WebKit takes it from the
  URL only for http), which is why the frames are CompMD's: their policies name their scheme.
- **Nothing CompMD does fails silently:** `CompMd.svelte` shows a note when CompMD can't load or a component draws
  nothing, and logs the cause; keep both when changing it.
- **Check the app in a window, both ways:** `cargo tauri dev` (the CLI's server) and `cargo run -p atm-app`
  (`tauri://localhost`, as the bundle), before a task that changes the window is done.

## Platform features

- Platform features (notifications, files, …) are host services: traits in atm-lib, implemented in the app with Tauri
  plugins.
- Any new plugin is a new dependency and is proposed to the user first ([Rust guide](rust.md)).
- Installing the agent skills is the same code as `atm agents install`
  ([better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md) §4); the app's button calls it, never a second
  implementation.

## Tests

- **Domain behaviour** is tested in atm-lib, not through Tauri.
- **Command wiring** gets tests with Tauri's mock runtime: each command reaches the right atm-lib call, and errors
  serialize as expected.
- **End to end:** the UI runs under Playwright with mocked Tauri IPC (`tests/e2e/helpers.ts` `asApp()`, the internals
  `@tauri-apps/api/mocks` sets, backed by the e2e fixture's data and atm's own task views; `tests/e2e/app.spec.ts`),
  and the transport's mapping has unit tests on `@tauri-apps/api/mocks` (`tests/unit/ipc.test.ts`). WebDriver (`tauri-driver`) doesn't support macOS. The real app
  is checked with a manual QA checklist.
