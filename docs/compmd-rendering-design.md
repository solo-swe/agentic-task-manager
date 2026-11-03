# CompMD in the task details (T-284)

The details panel renders a task's description and notes with CompMD instead of `src/lib/markdown.ts`. This document
settles how CompMD reaches the page, how it is configured for text nobody vouches for, and what stays as it is. T-285
(Mermaid diagrams) and T-336 (opening referenced documents) build on it.

What was checked: `@compmd/processor` and `@compmd/render` 0.3.0 as published on npm (their READMEs, types and
`dist/`), the dashboard's details panel, and atm serve's page policy (`page_csp` in `atm-cli/atm/api/server.py`).

## 1. Requirements

- **R1. What renders:** a task's description, its notes and the review note shown above the description (it is a
  note too) render with CompMD: CommonMark and GFM, so tables, task lists, strikethrough and footnotes render, which
  `markdown.ts` shows as text today. The blocked task's questions (one line each) keep `markdown.ts`'s inline
  rendering, as do Help, What's new and the card tooltips (`markdownPlainText`).
- **R2. Never blank:** the panel renders with `markdown.ts` at once; CompMD's render replaces it once CompMD is
  loaded. If CompMD fails to load, or a document fails to process, `markdown.ts`'s render stays. A failure is logged
  once, not shown.
- **R3. The dashboard stays one file within its guardrail** (550 KiB; 543 kB today). CompMD ships as separate files
  beside it, loaded only when a details panel first opens.
- **R4. Nothing leaves localhost,** whatever the task text says: no remote data, no pack downloads, no images or
  scripts from elsewhere.
- **R5. Task text runs nothing:** no raw HTML beyond a short allowlist, no scripts, no component code, no `host:`
  data, no inputs that change anything, no links other than `https:`, `http:`, `mailto:` and `#`.
- **R6. It looks like today:** headings, lists, code, quotes and tables follow the theme and light/dark mode through
  the dashboard's CSS tokens (`no-hardcoded-colours.test.ts` still passes); links open as today (external ones in a
  new tab with `noopener noreferrer`, `#` ones scroll within the text).
- **R7. The section rail** (T-280) keeps working: its ▲/▼ hints account for content that changes height when CompMD's
  render replaces the first one.
- **R8. Cheap on refresh:** the dashboard reloads every 5 seconds. A document is processed again only when its text
  changed; an unchanged one keeps its page.

## 2. Shipping CompMD

### Options

| Option | For | Against |
|---|---|---|
| **A. Inline it in the single-file dashboard** | one file, as today | the processor's WebAssembly is 2.8 MB (808 KB gzipped); inlined as base64 it is about 3.7 MB, seven times the guardrail, loaded with every page even when no panel opens |
| **B. Separate files served by atm serve** (chosen) | the page stays within its guardrail; CompMD loads only when needed, from localhost | a new read-only route; the page's CSP must allow loading a module and compiling WebAssembly; the plugin grows by about 3 MB |
| **C. Process on the server** | no WebAssembly in the page | atm is Python with the standard library only; the processor is Rust/WebAssembly; no way to run it there without a new runtime |

### The chosen way (B)

- **Files.** The build copies, from the pinned npm packages (exact versions, `npm ci`), only what the browser loads:
  - `plugin/assets/compmd/<version>/processor/`: `dist/index.js` (the typed API), `pkg/compmd_wasm.js` (the glue),
    `pkg/compmd_wasm_bg.wasm`, and the glue's `snippets/` (time zones);
  - `plugin/assets/compmd/<version>/render/`: `dist/*.js` without `bindings/` (the React and Svelte bindings import
    packages a browser can't resolve, and atm uses neither).

  Both packages import only their own files by relative paths, so they load as plain ES modules, unbundled. The
  files are committed build artifacts, like `tasks-dashboard.html`: the build writes them, the pre-commit hook
  stages them, and CI fails when they are stale. A test checks the copied set is exactly the list above (no
  bindings, no source maps pointing elsewhere).
- **Route.** atm serve serves `plugin/assets/` at `/assets/` (REST API layer, `api/server.py`; it reads plugin files,
  not task data, so it doesn't go through atm-lib): GET and HEAD only; the path normalized and required to stay
  under the folder (`..`, absolute paths and encoded separators refused); regular files only, no symlinks; content
  types by extension (`.js` `text/javascript`, `.wasm` `application/wasm`, else 404); the usual security headers;
  an ETag and a long `Cache-Control`, since a file's name changes with the plugin version (below).
- **Versioned path.** The build copies into a folder named for the CompMD version (`plugin/assets/compmd/0.3.0/
  processor/…`), removing older ones, and writes that version into the page. The dashboard asks for
  `/assets/compmd/<version>/…`, so a page left open across an update that changes CompMD gets a 404 (and keeps its
  `markdown.ts` render, then the reload offer) rather than mixing an old page with new modules (T-388's case).
- **Loading.** `src/lib/compmd.ts` loads both modules with a dynamic `import()` of the served URLs (marked
  `/* @vite-ignore */`), then `init()` the processor, which fetches its `.wasm` beside it (`new URL(…,
  import.meta.url)` in the glue). The dashboard's own `fetch` rule (only `src/lib/api/http.ts` calls `fetch`) still
  holds: the dashboard calls none here; the glue's one fetch is inside the served module. The module is loaded once
  per page, on the first panel open, and kept.
- **Dev and tests.** The dev server forwards `/assets/` to the real API (`build/dev-api.ts`, with `api/`, `p/` and
  `__atm/`); the e2e `tasksServer` fixture serves `plugin/assets/` the same way.

### The page's CSP

Today: `script-src` has only the page's inline script hashes; nothing allows WebAssembly. It becomes:

```
script-src <the inline hashes> 'wasm-unsafe-eval' http://localhost:<port>/assets/compmd/<version>/
```

- `'wasm-unsafe-eval'` allows compiling WebAssembly, and nothing else (not `eval`).
- The module source is the asset folder only, not `'self'`: a JSON route can never be loaded as a script, and
  `nosniff` already refuses that. The server builds it from the request's `Host`, which it already requires to be
  loopback (I10). If the browsers' path matching proves unreliable in the e2e run, the fallback is `'self'`
  (decided in review, and written in the security document).
- `connect-src 'self'` already allows the `.wasm` fetch; `img-src 'self' data:` already refuses images from elsewhere;
  `style-src 'unsafe-inline'` already allows CompMD's stylesheet.
- `build/security-headers.ts` mirrors the change for the dev server (`security-headers.test.ts` keeps them in step).

## 3. Configuring CompMD for untrusted text

Each document is processed alone:

```ts
process({
  path: 'task.comp.md',
  files: { 'task.comp.md': text },   // no other files: imports of files fail (a diagnostic)
  now: new Date().toISOString(),
  tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
  allowedOrigins: [],                // no remote data (R4)
  policy: { imports: 'listed', packs: {} },   // no packs; T-285 lists its own Mermaid pack as ambient
  // no `remote`, `host`, `packs` or `inputs`: nothing is fetched, no host data, no input values
});
```

and rendered with `createPage(element, { tokens })`, with no `components` and no `onSet`:

| Task text uses | What happens |
|---|---|
| raw HTML | the renderer's allowlist (`details`, `summary`, `sub`, `sup`, `kbd`, `br`, `img`, `a` with safe URLs); every other tag shows as text; no attributes beyond `src`, `alt`, `href` |
| an image | `img-src` allows only this server and `data:`, so an outside image doesn't load (and makes no request) |
| a link | after each render, a pass keeps only `https:`, `http:`, `mailto:` and `#` links (`safeHref`, as today; CompMD also allows relative ones, which T-336 will define); others become text. External links get `target="_blank" rel="noopener noreferrer"`; `#` links scroll within the document |
| a host-language component (JSX, Svelte, …) | never mounted: with no `components`, the renderer shows its fallback |
| `from: https://…`, `host:` data, a pack | refused by the processor (no allowed origin, no host data, policy `listed` with none listed): a diagnostic, and the component's fallback |
| an input | rendered without `onSet`, so changing it changes nothing |

**Diagnostics** are not shown in the panel: the reader isn't the author, and an agent writing task text doesn't see
them. They are logged to the console in development builds only.

## 4. In the panel

- `CompMd.svelte` takes `text` and an id prefix. It renders `Markdown.svelte` (R2) until CompMD is ready, then
  creates a page in its element and renders the processed tree; it processes again only when `text` changes (R8),
  and destroys the page when unmounted.
- **Theme.** `tokens` map CompMD's `--compmd-*` properties to the dashboard's CSS custom properties (text, muted,
  border, accent, code background, spacing); a few rules in `app.css` scoped to `.compmd` match today's headings,
  lists, code, quotes and tables. When the theme or mode changes, the panel calls `page.scheme()` (for T-285's
  diagrams; CSS-coloured content follows by itself).
- **Section rail.** The panel already measures its sections; CompMD's render changes their height once, so the rail
  re-measures on a `ResizeObserver` of the sections (if it doesn't already).
- **Fragment links and ids.** `#` links scroll to the heading with that id inside the same document, as
  `Markdown.svelte`'s `idPrefix` does today, so two notes with the same heading don't collide.

## 5. Security document

`docs/security-architecture.md`, in the same PR:
- I11's route list: `/assets/` (plugin files only, read-only, the checks above);
- I13: the CSP's new `script-src` sources, and why each;
- a section on CompMD as the renderer of untrusted task text: the settings of §3, and the tests that prove them.

## 6. Tests

- **atm-cli** (`tests/scripts/serve.test.mjs`): `/assets/` serves a `.js` and a `.wasm` with their types; refuses
  `..`, encoded separators and a symlink out of the folder; 404s a missing file and an unknown extension; refuses
  POST; the page's CSP has the new sources.
- **Unit:** the request `compmd.ts` passes (§3), exactly; the link pass (each scheme); the fallback when the import
  fails; processing skipped when the text is unchanged.
- **e2e** (against the build, with the fixture serving `plugin/assets/`): a table and a task list render; raw
  `<img onerror>` and `<script>` show as text and run nothing; a `javascript:` link isn't a link; an `https:` image
  and a `from: https://…` component make no request (the guardrail's "no request leaves localhost"); the
  `markdown.ts` render stays when `/assets/` answers 404; light and dark; the section rail's hints after the swap;
  the details panel's screenshot baselines updated.
- **Build:** the copied asset list is exactly §2's; the guardrail still passes.
- **Dev:** `dev-api.test.mjs` checks `/assets/` is forwarded.

## 7. Costs and open points

- **Plugin size:** about 3 MB more per installed plugin version (the WebAssembly), and the same in the repository's
  history each time CompMD is updated. The page itself grows only by the loader and the panel component.
- **First open:** loading and compiling the module takes some tens of milliseconds from localhost; the panel shows
  `markdown.ts`'s render meanwhile, then swaps once. Later panels render at once.
- **For review:**
  1. The CSP module source limited to the asset folder, with `'self'` as the fallback: agreed?
  2. The review note above the description renders with CompMD too (it is a note); questions stay one-line
     `markdown.ts`: agreed?
  3. Diagnostics not shown to the reader: agreed, or a small "some of this text couldn't render" mark?
