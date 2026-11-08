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

## Security

- **Capabilities are least privilege:** each window gets only the commands and plugin permissions it uses.
- **Rendered task text is never given a capability:** CompMD's sandbox origin is not listed in any capability
  ([compmd-rendering-design.md](../compmd-rendering-design.md)).
- **Strict CSP for the shell:** the dashboard server's policy ([security-architecture.md](../security-architecture.md)
  I13: own scripts by hash, no remote or inline scripts, `frame-src` only CompMD's sandbox frames), with `connect-src`
  only for Tauri IPC.

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
- **End to end:** the UI runs under Playwright with mocked Tauri IPC (`@tauri-apps/api/mocks`) backed by an in-memory
  project (`open_memory_project`) or recorded fixtures. WebDriver (`tauri-driver`) doesn't support macOS. The real app
  is checked with a manual QA checklist.
