# Svelte / SvelteKit guide

atm's own rules for the dashboard in `dashboard/`, served by `atm serve` and later by the atm app
([better-way-to-distribute-atm.md](../better-way-to-distribute-atm.md)). Its routes and data are in
[multi-project-api.md](../multi-project-api.md), the layers behind them in
[layered-architecture.md](../layered-architecture.md) §6, and the commands and test conventions in
[AGENTS.md](../../AGENTS.md).

## Basics

- **Svelte 5 with runes** and TypeScript in strict mode. No legacy `$:` statements and no `export let`; props use
  `$props()`.
- **SvelteKit with the static adapter** (SPA fallback), one build for `atm serve` and later the atm app. SvelteKit 3
  takes its options in the `sveltekit({ … })` plugin call in `vite.config.ts`; there is no `svelte.config.js`.
  `tsconfig.json` extends `$app/tsconfig`.
- ESLint, Prettier and `svelte-check` pass with no warnings.

## Toolchain

- **pnpm, installed with its [standalone installer](https://pnpm.io/installation).** Not through corepack, which
  Node.js stops shipping from Node 25 on, and not through a Node version manager.
- **Every `package.json` with a lockfile pins pnpm and Node.js** in `devEngines`, with `"onFail": "download"`:

  ```json
  "devEngines": {
    "packageManager": { "name": "pnpm", "version": "12.14.0", "onFail": "download" },
    "runtime": { "name": "node", "version": "24", "onFail": "download" }
  }
  ```

  pnpm switches to the pinned pnpm and downloads the pinned Node.js, recording both in `pnpm-lock.yaml`. Scripts run
  on that Node.js, locally and in CI.
- A bare `node` typed in a shell may still be another Node.js, e.g. one from nvm. Run tools through `pnpm`
  (`pnpm exec`, `pnpm <script>`).

## Where logic lives

- **No domain logic in the UI.** Validation, queries, ordering, search and what atm derives from tasks (stages,
  readiness, waiting, sprint progress) belong to atm-lib and reach the dashboard through the v1 API
  ([layered-architecture.md](../layered-architecture.md) §6.2). The UI shows data and sends user intent.
  **One exception:** filtering, the filter counts, sorting and search over the loaded tasks stay in the dashboard
  ([ADR-001](../adr/ADR-001-filters-and-search-stay-in-the-dashboard.md)); the values they compare come from atm.
- **The typed client wrapper is the only path to the server** (`src/lib/api/`). It is built on the types generated
  from atm's schemas (`src/lib/generated/`, [layered-architecture.md](../layered-architecture.md) §4), which are never
  edited by hand. Components never call `fetch` directly.
- **Live data comes from a rune-based helper** in a `.svelte.ts` module that loads from the API, keeps the view up to
  date and stops when its component is destroyed. Components never re-fetch whole lists.
- Shared UI state lives in `.svelte.ts` modules with `$state`, not in global stores.

## Navigation

- **Views are SvelteKit routes,** so back, forward and deep links work: the all-projects overview, a project and its
  tabs, a task's details and a document in the viewer each have an address.
- Opening a task from elsewhere (the overview, search, a link in task text) navigates to its project with the task
  open.

## Components and styling

- **Small components with typed props.** One component per file, in `PascalCase.svelte`, grouped by area under
  `src/lib/` (e.g. `kanban/`, `graph/`, `overview/`, `details/`, `dialogs/`).
- **Menus, dialogs, popovers and other interactive primitives use Bits UI** (headless and accessible), styled with our
  tokens. They are not rebuilt by hand.
- **Scoped `<style>` per component.** Colors, spacing, radii and type are CSS custom properties in one tokens file,
  which the themes in `src/themes/` set. No CSS framework and no inline colors.
- **Icons** come from lucide's Svelte package.

## Accessibility

- Real `<button>`, `<a>`, `<input>` and `<label>` elements. No click handlers on `div` or `span`.
- Every icon-only button has an `aria-label`.
- Keyboard: everything reachable by Tab, menus and dialogs follow the Bits UI patterns, and Esc closes overlays.
- Text contrast of at least 4.5:1.

## Device-local state

- View preferences (theme, filters, layout) are kept in `localStorage`, with every read and write wrapped in
  `try/catch`, and the UI works without it.
- Nothing else is stored in the browser: tasks and project settings live in `tasks.json`, changed only through atm.

## Tests

- **Vitest** for live-data helpers, state modules and components (with Testing Library for Svelte).
- **Playwright** for end-to-end flows, against the API played by the `tasksServer` fixture (AGENTS.md, Testing
  conventions), and with mocked Tauri IPC for the app ([Tauri guide](tauri.md)).
- Tests use the generated types, so a changed API breaks the build, not the user.
