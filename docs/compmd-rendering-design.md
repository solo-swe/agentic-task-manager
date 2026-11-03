# CompMD in the task details (T-284)

The details panel renders a task's description and notes with CompMD instead of `src/lib/markdown.ts`. This document
settles how CompMD reaches the page, how it is configured for text nobody vouches for, and what stays as it is. T-285
(Mermaid diagrams) and T-336 (opening referenced documents) build on it.

What was checked: `@compmd/processor` and `@compmd/render` 0.3.0 as published on npm (their READMEs, types and
`dist/`), the dashboard's details panel, and atm serve's page policy (`page_csp` in `atm-cli/atm/api/server.py`).

## 1. Requirements

- **R1. What renders:** all task text in the details panel renders with CompMD, and only with CompMD: the
  description, every note, the review note shown above the description, and a blocked task's questions. That's
  CommonMark and GFM, so tables, task lists, strikethrough and footnotes render, which `markdown.ts` shows as text
  today. Help, What's new and the card tooltips (`markdownPlainText`) are not task details and keep `markdown.ts`
  for now (moving them is a follow-up, so `markdown.ts` can go).
- **R2. CompMD only, no second renderer** (decided in review). Until CompMD has loaded (the first panel opened in a
  page, some tens of milliseconds) the text's place is held at its height, empty. If CompMD can't load, or a
  document fails to process, the text shows as it is written, escaped, with its line breaks (`white-space:
  pre-wrap`): readable, not rendered. That failure is logged to the console, not shown to the reader.
- **R2a. Readers never see CompMD's diagnostics** (warnings and errors about the text). Agents, who write the text,
  should: §3a.
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

**What `script-src` does.** It lists where the page may run JavaScript from: inline `<script>` blocks (by hash), and
script URLs, which include every `import()` and every module those modules import. Today it has only the page's own
inline script hashes, so the page can't load any script file at all: an injected `<script src="/anything">` or
`import("/anything")` is refused by the browser. That is the second line of defence. The first is that task text
can't put a script into the page in the first place (Svelte escapes it; CompMD's raw HTML allowlist has no
`script`). The CSP matters only if the first line ever fails, through a bug in a renderer or a new route.

To load CompMD, `script-src` must name some script URLs, and allow compiling WebAssembly (`'wasm-unsafe-eval'`,
which allows that and nothing else: not `eval`, not `new Function`). The question is how wide the URLs are.

| Option | `script-src` | Allows |
|---|---|---|
| **1. The asset folder** (recommended) | `<hashes> 'wasm-unsafe-eval' http://localhost:8765/assets/compmd/0.3.0/` | only the files this plugin version ships, under that folder |
| **2. `'self'`** | `<hashes> 'wasm-unsafe-eval' 'self'` | any URL on this server that answers with a JavaScript content type |
| 3. Hashes of each file | — | not possible: hashes allow external scripts only with an `integrity` attribute, and `import()` and the imports inside a module carry none |
| 4. `'strict-dynamic'` with a nonce | `'nonce-…' 'strict-dynamic'` | whatever a trusted script loads, transitively, from anywhere; wider than 2 |

**Why `'self'` is the riskier one.** It trusts every route, today's and future ones, to never answer a script
request with something runnable. Today that holds: the routes answer JSON, the page, and the project's logo (an image
type, with `nosniff`, so the browser refuses to run it as a script). But the guarantee rests on every future route
getting its content type right. Examples of how that could break:
- **T-336** plans a route serving a project's documents to the CompMD viewer. If it served a project's `.js` file
  as `text/javascript`, then under `'self'` a malicious repository's `docs/x.js` becomes a runnable script on the
  dashboard's origin. Combined with any injection bug (say, a future allowlisted tag that takes a `src`), an
  attacker who can only get a file into a repository you open would run code in your dashboard. That code could
  read every project's tasks through the API and show you a convincing fake page.
- A route that **echoes part of the request** (an error message quoting the path, a search echoing the query) with
  a JavaScript-compatible type would become a script gadget: `import("/api/search?q=alert(1)//")`.
- A **dev-only route** or a debugging aid added later, serving files with types guessed from their extension.

With option 1, none of these runs: the browser runs only files whose URL is under the versioned asset folder, which
holds only what the build copied from the pinned packages.

**What option 1 costs,** which is why the first draft named a fallback:
- **The origin is per request.** A path can't be written without its scheme, host and port: `http://localhost:8765`
  is a different source from `http://127.0.0.1:8765`. The server already builds the CSP for each page response; it
  takes the host from the request's `Host` header, which it already requires to be a loopback name (I10), so the
  page opened as `127.0.0.1:8766` gets `http://127.0.0.1:8766/assets/compmd/0.3.0/`.
- **The dev server's origin is Vite's** (`localhost:5173`, forwarding `/assets/` to the API): `build/security-headers.ts`
  builds it from the dev server's address, and `security-headers.test.ts` keeps the two in step.
- **Path matching rules:** a source ending in `/` matches everything under it; one without matches that file only.
  After a redirect, browsers ignore the path part (so that a redirect can't reveal where it went); the asset route
  never redirects, and a test checks it answers directly.
- **Browser support:** paths in sources are CSP level 2, supported by Chrome, Firefox and Safari for years. The e2e
  suite runs in Chromium; the risk was only that something in this setup behaves differently, which the e2e run on
  the real CSP shows before merging.

None of these is a reason to fall back to `'self'`; each is handled once, with a test. **Decided in review: option 1,
with no fallback.**

The rest of the policy needs no change: `connect-src 'self'` allows the glue's `.wasm` fetch, `img-src 'self' data:`
refuses images from elsewhere, and `style-src 'unsafe-inline'` allows CompMD's stylesheet.

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

**Diagnostics** are not shown in the panel (R2a). How agents see them is §3a.

## 3a. Diagnostics for agents (open: to decide in review)

Agents write task text through atm (`atm add`, `edit`, `note`, `review`, `block`). They should learn there that what
they wrote won't render as meant (an unknown component, an unclosed container, a remote import that will be refused).
The difficulty: the processor is WebAssembly, and atm is Python with the standard library only, which can't run it.
The dashboard can, but it is read-only by design (I11: nothing changes state over HTTP), and agents don't read it.

| Option | How | For | Against |
|---|---|---|---|
| **A. atm checks on write, with Node when present** | atm's write commands (and `atm check`) run `plugin/scripts/compmd-check.mjs` with `node` when it is on PATH: the same WebAssembly files the dashboard uses (`plugin/assets/compmd/…`), `initSync` with the bytes, the same request as §3. Warnings print after the command's output (`--json`: a `compmd` list); they never block a write. Without Node: skipped silently, one line in `atm check` | the agent sees them at the moment it writes, in the output it already reads; one processor and one configuration, so it reports exactly what the panel will do | Node becomes an optional second runtime (atm's rule is Python only: here it's optional, nothing breaks without it); about 50–100 ms per write to load the module |
| **B. On demand only** | the same check, only in `atm check` (and the end-of-task steps of the skills call it) | no cost on every write | found later, after the text is committed |
| **C. Guidance only** | T-286 tells agents what renders; no check | nothing new to run | mistakes go unnoticed; readers just see odd text |
| **E. The Rust crate, natively** | atm runs a `compmd` command-line tool built from the `compmd` crate (the same processor the WebAssembly is built from) | no Node; starts in milliseconds; the same results when the versions match | no such tool exists yet: the crate is a library (0.3.0, no binary, no GitHub releases). It needs a CLI in the CompMD repository and builds for each platform (macOS arm64 and x86-64, Linux x86-64 and arm64, Windows), several MB each (the crate carries ICU's locale data). atm can't ship them all in the plugin (every install would download every platform's binary), and must not download executables at run time; so it would use one only when the user installed it (`cargo install`, Homebrew). And a native binary runs with the user's full rights, where the WebAssembly runs sandboxed (it reaches nothing but what the glue gives it) |
| **D. The dashboard records them** | the panel would report diagnostics back to atm serve, which stores them for `atm status` | no Node | breaks the read-only dashboard (I11) and the tenet it comes from; the server would write task data | 

Recommended: **A**, with B's `atm check` included. A Node check is how a project already using Node would see it;
for one without, nothing changes but the missing warnings.

## 4. In the panel

- `CompMd.svelte` takes `text` and an id prefix. Until CompMD is ready it holds the text's place (R2); then it
  creates a page in its element and renders the processed tree; it processes again only when `text` changes (R8),
  and destroys the page when unmounted. A question renders as a one-paragraph document, its paragraph's margins
  trimmed so it sits in its list item as today.
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
- **Unit:** the request `compmd.ts` passes (§3), exactly; the link pass (each scheme); the plain text shown when the
  import fails; processing skipped when the text is unchanged.
- **e2e** (against the build, with the fixture serving `plugin/assets/`): a table and a task list render; raw
  `<img onerror>` and `<script>` show as text and run nothing; a `javascript:` link isn't a link; an `https:` image
  and a `from: https://…` component make no request (the guardrail's "no request leaves localhost"); the text
  shows as written when `/assets/` answers 404; light and dark; the section rail's hints after the swap;
  the details panel's screenshot baselines updated.
- **Build:** the copied asset list is exactly §2's; the guardrail still passes.
- **Dev:** `dev-api.test.mjs` checks `/assets/` is forwarded.

## 7. Costs and open points

- **Plugin size:** about 3 MB more per installed plugin version (the WebAssembly), and the same in the repository's
  history each time CompMD is updated. The page itself grows only by the loader and the panel component.
- **First open:** loading and compiling the module takes some tens of milliseconds from localhost; the text's place
  is held empty meanwhile. Later panels render at once.
- **Decided in review:** CompMD only, no second renderer (R2); notes, the review note and questions render with it
  (R1); readers don't see diagnostics (R2a); the CSP's module source is the asset folder only (§2).
- **Still open:** how agents see diagnostics (§3a).
