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
  today. Help and What's new render with CompMD too since `markdown.ts` was removed (in the same sprint, after
  T-286): the same component, with no task, so no `host:` data, loaded the first time one of them opens. Tooltips and
  card excerpts, which can't show markup, take the marks away (`plain-text.ts`) and render nothing.
- **R2. CompMD only, no second renderer** (decided in review). Until CompMD has loaded (the first panel opened in a
  page, some tens of milliseconds) the text's place is empty (`data-compmd="loading"`; `ready` once drawn). If CompMD
  can't load, or a
  document fails to process, the text shows as it is written, escaped, with its line breaks (`white-space:
  pre-wrap`): readable, not rendered. That failure is logged to the console, not shown to the reader.
- **R2a. Readers never see CompMD's diagnostics** (warnings and errors about the text). Agents, who write the text,
  should: §3a.
- **R3. The dashboard stays one file;** its guardrail (550 KiB) may be raised for this (decided in review), by what it
  measures. T-284 didn't need it: the page grew from 543 to 547 kB, the loader and the panel component; the sandbox's
  host runtime is in the served module. CompMD, the sandbox's frames and Mermaid ship as separate files beside
  it, loaded only when a details panel first opens (Mermaid only for a text with a diagram).
- **R4. Nothing leaves localhost,** whatever the task text says: no remote data, no pack downloads, no images or
  scripts from elsewhere.
- **R5. Task text runs nothing of its own:** no raw HTML beyond a short allowlist, no scripts, no component code
  from the text, no `host:` data but the task's own, no inputs that change anything, no links other than `https:`,
  `http:`, `mailto:` and `#`. The only code that runs for it is atm's choice, shipped with the plugin: CompMD, and
  Mermaid's pack in a strict frame (§3c).
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

- **Files** (as built). A second Vite build (`vite.compmd.config.ts`, run by `npm run build`) bundles
  `dashboard/compmd/entry.ts`, which re-exports `init`, `process` and `createSandbox` from the pinned packages
  (exact versions, `npm ci`), into `plugin/assets/compmd/<version>/`: `compmd.js` (59 kB: the processor's glue, the
  renderer and the sandbox's host side) and `compmd_wasm_bg.wasm` (2.8 MB) beside it. Copying the packages' files
  as they are doesn't work: `@compmd/sandbox` imports `@compmd/render` by package name, which a browser can't
  resolve without an import map. It is an app build, not library mode, which would inline the WebAssembly as a
  `data:` URL that `connect-src 'self'` refuses; with `base: './'` the glue finds the `.wasm` beside the module.
  The files are committed build artifacts, like `tasks-dashboard.html`. `npm run check` (so CI) builds them again
  into a temporary folder and fails unless every file matches (`scripts/check-compmd-assets.mjs`); the pre-commit
  hook and CI's workflow are unchanged.
- **Route.** atm serve serves `plugin/assets/` at `/assets/` (REST API layer, `api/server.py`; it reads plugin files,
  not task data, so it doesn't go through atm-lib): GET and HEAD only; the path normalized and required to stay
  under the folder (`..`, absolute paths and encoded separators refused); regular files only, no symlinks; content
  types by extension (`.js` `text/javascript`, `.wasm` `application/wasm`, else 404); the usual security headers;
  `Cache-Control: no-cache` with an ETag, so the browser asks again and gets a 304 (`asset_file` in
  `api/server.py`: each segment a plain name, no `.`, `..`, hidden file or percent-encoding).
- **Versioned path.** The build writes into a folder named for the CompMD version (`plugin/assets/compmd/0.3.0/`),
  removing older ones, and the page gets that version from the build (`virtual:atm-compmd`). The dashboard asks for
  `/assets/compmd/<version>/compmd.js`, so a page left open across an update that changes CompMD gets a 404 (its
  texts show as written, then the reload offer) rather than mixing an old page with new modules (T-388's case).
- **Loading.** `src/lib/compmd.ts` loads the module with a dynamic `import()` of its served URL (marked
  `/* @vite-ignore */`), then `init()` the processor, which fetches its `.wasm` beside it. The dashboard's own `fetch`
  rule (only `src/lib/api/http.ts` calls `fetch`) still holds: the dashboard calls none here; the glue's one fetch is
  inside the served module. The module is loaded once per page, on the first panel open, and kept; a failure is kept
  too.
- **Dev and tests.** The dev server forwards `/assets/` to the real API (`build/dev-api.ts`, with `api/`, `p/` and
  `__atm/`); the e2e static server (`tests/e2e/serve.mjs`) serves `plugin/assets/` beside the page by the same
  rules, so the e2e suite loads the real module under the real CSP.

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
| **1. The asset folder** (recommended) | `<hashes> 'wasm-unsafe-eval' http://localhost:8765/assets/compmd/` | only the files this plugin ships, under that folder |
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

Each document is processed alone, as the dashboard's frontmatter (§3b) followed by the stored task text:

```ts
process({
  path: 'task.comp.md',
  files: { 'task.comp.md': frontmatter + text },   // no other files: imports of files fail (a diagnostic)
  now: new Date().toISOString(),
  tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
  allowedOrigins: [],                // no remote data (R4)
  host: { task },                    // the current task only (§3b)
  packs: { '@comp-markdown/mermaid': [mermaidRelease] },   // shipped with the plugin (§3c)
  policy: { imports: 'listed', packs: { '@comp-markdown/mermaid': { level: 'strict' } } },
  // no `remote` or `inputs`: nothing is fetched, no input values
});
```

and rendered by `@compmd/sandbox`'s view in host mode (§3c), with no trusted components, no `host` functions and no
`set` handler:

| Task text uses | What happens |
|---|---|
| raw HTML | the renderer's allowlist (`details`, `summary`, `sub`, `sup`, `kbd`, `br`, `img`, `a` with safe URLs); every other tag shows as text; no attributes beyond `src`, `alt`, `href` |
| an image | before rendering, `safeTree` turns any image that isn't a `data:` image (Markdown's, a reference's, raw `<img>`) into its alt text, so none is ever requested; `img-src 'self' data:` would refuse one anyway, but with a policy violation in the console each time |
| a link | `safeTree` turns a link to anything else into its text before rendering; after each render, a pass keeps only `https:`, `http:`, `mailto:` and `#` links (`safeHref`, as today; CompMD also allows relative ones, which T-336 will define); others become text. External links get `target="_blank" rel="noopener noreferrer"`; `#` links scroll within the document |
| a host-language component written in the text (JSX, Svelte, …) | the text has no files to import it from, so there is none to mount: its fallback shows |
| `from: https://…`, a file, another `host:` source | refused by the processor (no allowed origin, no files, only `host:task` given): a diagnostic, and the fallback |
| a pack other than Mermaid | refused (`imports: listed`, only Mermaid listed): F026, and the fallback |
| a ` ```mermaid ` block | drawn by the Mermaid pack in a strict frame (§3c) |
| an input | shown, but with no `set` handler, changing it changes nothing |

**Diagnostics** are not shown in the panel (R2a). How agents see them is §3a.

## 3b. The frontmatter the dashboard adds, and the task's data

Task text is a document's **body** only, stored as written: nothing is added to `tasks.json`. When the dashboard
renders a text, it puts one frontmatter in front of it, built in one place (`src/lib/compmd.ts`), the same for
every text (decided in review). So the settings are atm's, defined once, and agents never write `md:` keys:

```yaml
---
md:
  version: 1
  data:
    - from: host:task
      as: task
---
<the task text>
```

- **`md.data.task`** is the task the text belongs to: its fields as `atm show --json` gives them (`id`, `name`,
  `type`, `status`, `priority`, `sprint`, `labels`, `dependencies`, `files`, `commit`, `pr`, `notes`, the actors and
  times) and atm's derived values for it (`kind`, `ready`, `waiting_for`, the effective review mode and release, the
  stage), which the v1 route already gives the dashboard. The request passes it as `host: {task: …}`.
- **No `schema`** (found in T-284): in CompMD 0.3.0 a `host:` source with a schema is typed as a list of records,
  so `md.data.task.dependencies` fails (F024, "list(map(string, dyn)) has no fields"), while the task is one
  record. Without one its values are untyped and checked when evaluated: fields, `each` over a list, `md.list()` and
  `size()` all work. A schema for a single record would need a CompMD change; then it can be written from atm's task
  and view schemas (`atm-cli/atm/common/schema/`), so it can't drift.
- Only the current task: never another task's or another project's data. It is data the reader sees in the same
  panel; values inserted into the text are escaped like any text.
- Examples an agent might write: `{{md.data.task.status}}`; a list, which `{{…}}` can't write, with
  `:value[its dependencies]{of=(md.list(md.data.task.waiting_for))}` or `::: each {in=(md.data.task.dependencies)
  as=d}`; the task's commit with `{{md.data.task.commit}}`. A field the task lacks shows the expression as written.
- **Text that starts with a frontmatter of its own** would follow ours and render as a line and a heading. The
  skills say the dashboard provides the frontmatter (T-286); today's task text is plain Markdown, so nothing
  existing changes.
- **One builder:** the frontmatter and request are built by one function, which a later checker would share;
  diagnostics' line numbers are given relative to the task text, without the frontmatter's lines.

## 3c. Mermaid (T-285): the pack in the sandbox, or built into the dashboard

| | **P. `@comp-markdown/mermaid` in CompMD's strict sandbox** (recommended) | **D. Mermaid built into the dashboard** |
|---|---|---|
| How | the pack is ambient (`compmd.config.json`); `@compmd/sandbox` runs its component in a strict frame: an opaque origin with no network, reaching only its params; the dashboard renders in host mode (`page: "host"`), so the text is in the dashboard's page and each diagram in its frame | the `mermaid` npm package, run in the dashboard's page through the renderer's `code: { mermaid }` option |
| Mermaid fed untrusted text | in a frame that can't reach the dashboard, its API or the network: a Mermaid XSS (it has had several) stays in the frame | in the dashboard's page, guarded by Mermaid's `securityLevel: 'strict'` and the page's CSP |
| Theme | Mermaid's own default or dark theme, following the page's light or dark mode | Mermaid's `themeVariables` from the dashboard's tokens: matches every dashboard theme |
| Files and size | the pack's one module (7.0 MB, 1.6 MB gzipped; every Mermaid diagram type, with ELK), the sandbox's frame documents and runtime (`buildFrames`: about 1.9 MB with React, Svelte and a compiler atm doesn't use), loaded only for a text with a diagram | Mermaid's own build, loaded only for a text with a diagram; it can't be inlined in the single file (megabytes), so it is served from `/assets/` either way |
| Server and CSP | `frame-src` for the frame documents (the asset folder, as for scripts); the frame documents served with `Content-Security-Policy: sandbox allow-scripts`; the pack's module with `Access-Control-Allow-Origin: *`, since the frames' opaque origin loads it cross-origin, and checked against its hash | no frames; Mermaid's module under the existing `script-src` asset source |
| Beyond Mermaid | the same path for any later pack (charts, a document viewer's components, T-336): a pack is an npm install and a line in `compmd.config.json` | each further renderer is dashboard code again |
| For CompMD | atm uses CompMD as its documents and other hosts do, and tests the sandbox in a real host | a host-specific path other hosts don't share |
| Moving parts | more: frames, their headers, CORS on the pack folder, the sandbox's protocol | fewer |

**Decided in review: P now; D (Mermaid built in at build time) to be reconsidered later.** CompMD's own example of
a pack drawing Mermaid is `editor/examples/diagrams/diagrams.comp.md` in the CompMD repository: a pack's component
is one built module the sandbox loads by URL, run only if it matches its hash.

**Its runtime cost.** In host mode each strict component gets a frame of its own, so each diagram's frame loads
Mermaid's module: from the browser's cache after the first, but parsed and compiled again per frame (7 MB of
JavaScript; tens to a few hundred milliseconds each, more on a slow machine), and held in memory per frame while the
panel shows it. A task with one or two diagrams is fine; a note list with many would be slow. Measure it in T-285
(an e2e timing for a task with one and with five diagrams), and if it is too slow, the options are: the sandbox's
`page: "frame"` mode (one strict frame per document, sharing one Mermaid; the text then renders inside the frame,
so the theme's stylesheet is given to it with `view.styles`), giving the module as code so frames share its bytes
(`modules: () => ({ code })`), or D.

**Why P:** the decisive point is that Mermaid, a large library with a history of XSS bugs, renders text
anyone with access to `tasks.json` writes; P runs it where a bug can't reach anything. P is also the path every
later CompMD capability takes. Its costs are one-time (the frame headers and their tests). Its one visible
difference, Mermaid's own light and dark themes instead of the dashboard's colours, can change later through the
pack's `theme` param, in CompMD.

### As built (T-285)

- **Through `@compmd/vite`, and nothing else** (decided in review). `dashboard/compmd.config.json` lists
  `@comp-markdown/mermaid` at version `0.1.1`, **ambient** (every text gets `mermaid` with no import line, so the
  frontmatter keeps only atm's own settings), with `imports: "listed"` and the default strict trust. The version is
  pinned there, in `package.json` (exact) and by the lockfile, from which the plugin takes the pack's integrity. The
  asset build (`vite.compmd.config.ts`) runs `compmd()`: it checks the configuration and the pack, writes the
  sandbox's frame documents and the pack's built module into `plugin/assets/compmd/<version>/compmd/`, and gives
  `virtual:compmd`, which the served module re-exports: `request` (the packs, `ambient`, the policy) and `sandbox`
  (where the frames are, and each built module's URL with its hash). The build's `base` is
  `/assets/compmd/<version>/`, where atm serve serves the folder, since the plugin makes those URLs from it.
- **The dashboard adds only its own parts:** `compmdRequest(request, text, task)` spreads the plugin's request and
  sets the text, the frontmatter, `host: { task }` and `allowedOrigins: []`, which win over anything a configuration
  might hold; the view opens with `createSandbox({ ...sandbox, page: "host" })`.
- **Components the text defines itself are dropped** (`packComponents`): `imports: "listed"` keeps out other packs,
  but CompMD still lets a document define a host-language component in a fence, which the sandbox would compile and
  run (in a strict frame). Only components with a pack's built module go to the sandbox (R5); a text's own shows its
  fallback.
- **atm serve** serves the folder as the plugin's dev server does: every asset with `Access-Control-Allow-Origin: *`
  (the frames' opaque origin loads them cross-origin), a frame document (`.html`) with `Content-Security-Policy:
  sandbox allow-scripts`; the page's policy gains `frame-src` limited to the asset folder.
- **Light and dark:** the component watches `<html>`'s `data-theme` and `data-mode` and calls `view.scheme()`; the
  pack's `theme=auto` draws Mermaid's dark or default theme.
- **A diagram Mermaid can't read, or a module that doesn't match its hash,** shows the block's code inside the
  frame, where the diagram would be.
- **Measured** (e2e, Chromium on an Apple M-series Mac): opening a task with one diagram, CompMD's first load
  included, 280–390 ms until it is drawn; one with five, 350–510 ms. No need for any of the options above.
- **Found:** Mermaid lets HTML in a label (`<img onerror>`) through into the frame's document; the strict frame's own
  policy refuses the inline handler and the image. The sandbox, not Mermaid, is what holds there (a CompMD-side note:
  the pack could run Mermaid with `securityLevel: 'strict'`).

The developer guide's chapters on packs, the sandbox and code renderers are still to come; this section is from the
packages' READMEs and code (`@compmd/sandbox`, `@compmd/vite`, `@comp-markdown/mermaid`, as published). Settle any
difference with the guide when those chapters appear.

## 3a. Diagnostics for agents

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

**Decided in review: C for now.** No checker: the skills guide agents in writing CompMD (T-286), from CompMD's own
guide for language models, `llms.txt`. A check (A, or E once CompMD has a command-line tool) can come later.

How the guidance is shipped:
- **A pinned copy in the plugin,** `plugin/docs/compmd-llms.txt`, taken from the CompMD repository at the commit of
  the shipped processor version, with that version and commit in a header line, and updated with CompMD. Skills
  never send agents to fetch it from the web: a fetched text could change under us and is outside text an agent
  would act on (the security document's rule for skills).
- **The skills say briefly what works in atm's panel,** since atm's settings (§3) refuse part of CompMD: GFM, alerts
  (`> [!NOTE]`), the task's own values (`md.data.task`), math and CompMD's core components (layout, `if`, `each`,
  `value`) render; inputs show but change nothing; imports of packs, data from files or URLs,
  `host:` data and host-language components don't render (they show their fallback text). For more, they point to
  the local `llms.txt` (33 KB, read only when an agent wants more than plain Markdown).

## 4. In the panel

- `CompMd.svelte` takes `text` and the `task` (and `inline`, for a question). CompMD's sandbox view (host mode, no
  components) owns an element of its own that Svelte never renders into; the plain text of R2 sits beside it. It
  processes again only when the text or the task's data changes (R8), and destroys the view when unmounted.
  `data-compmd` is `loading`, `ready` (drawn) or `failed`.
- **Theme.** The dashboard's stylesheet sets CompMD's `--compmd-*` properties on `.compmd` (accent, borders), and
  rules scoped to `.compmd-page` match today's headings (CompMD's `h1`–`h6` sized as the old renderer's `h3`–`h6`),
  lists, code and quotes, plus tables (scrolling inside the text when wide), task lists and alerts. Nothing is passed
  through `view.theme`. When the theme or mode changes, T-285's diagrams will need `view.scheme()`.
- **Section rail.** CompMD's render changes the sections' height once, after the panel opens; the rail measures
  them when an entry is used. The scroll tests open a task once its description is drawn (`data-compmd="ready"`).
- **Fragment links.** CompMD writes headings without ids, so a `#` link is resolved when clicked: the heading in the
  same text whose slug matches (as `Markdown.svelte`'s ids were made), scrolled into view. Two texts with the same
  heading don't collide, and CompMD's DOM is never changed.

## 5. Security document

`docs/security-architecture.md`, in the same PR:
- I11's route list: `/assets/` (plugin files only, read-only, the checks above);
- I13: the CSP's new `script-src` sources, and why each;
- a section on CompMD as the renderer of untrusted task text: the settings of §3, and the tests that prove them.

## 6. Tests

As written in T-284:
- **atm-cli** (`tests/scripts/serve.test.mjs`): `/assets/` serves `compmd.js` and the `.wasm` with their types, a 304
  for an unchanged file, HEAD, and 405 for POST; it 404s a missing file, a symlinked file, a file through a symlinked
  folder, another extension, `..`, percent-encoded `..`, a hidden file, an empty segment and the folder itself. The
  page's CSP has exactly the new sources, with the origin of the Host the page was asked for.
- **Unit, the real processor** (`compmd-processor.test.ts`, Node, the shipped WebAssembly): GFM with no
  diagnostics; `md.data.task` with atm's derived values, `each` over its lists; a frontmatter written in the text is
  only text; only `host:task` is given; an unknown component shows its fallback; `safeTree` on the real tree;
  diagnostics' lines.
- **Unit** (`compmd.test.ts`): the asset URL; loading once, a failure kept; the task's data, the frontmatter and the
  request, exactly; the link pass for each kind of link. **Components** (`DetailPanel.component.test.ts`, the real
  CompMD in jsdom): GFM, HTML outside the allowlist as text, `md.data.task`, a `#` link scrolling to its heading,
  notes, the review note and questions.
- **e2e** (`detail.spec.ts`, the real module from `/assets/` under the real CSP, every test guarded against requests
  outside localhost and CSP violations): GFM and the task's derived data; raw `<img onerror>`, `<script>`,
  an `onclick` and a `javascript:` link run nothing; an outside image shows its text and is never requested; web
  links open in a new tab; notes and questions; the text as written when `/assets/` answers 404. `security.spec.ts`:
  the policy's `script-src`.
- **Policy:** `security-headers.test.ts` keeps the dev page's policy the server's plus Vite's needs.
- **Build:** `npm run check` rebuilds the module and compares it with the committed files.
- **Dev:** `dev-api.test.mjs` fetches the module through the dev server.

## 7. Costs and open points

- **Plugin size:** about 3 MB more per installed plugin version (the WebAssembly), and the same in the repository's
  history each time CompMD is updated. The page itself grows only by the loader and the panel component.
- **First open:** loading and compiling the module takes some tens of milliseconds from localhost; the text's place
  is empty meanwhile. Later panels render at once.
- **Decided in review:** CompMD only, no second renderer (R2); notes, the review note and questions render with it
  (R1); readers don't see diagnostics (R2a); the CSP's module source is the asset folder only (§2).
- **Agents and diagnostics:** guidance only for now, from a pinned copy of CompMD's `llms.txt` (§3a, T-286).
- **The dashboard adds one frontmatter when it renders a text** (never stored in `tasks.json`); **the task's data is
  `md.data.task`** (§3b), decided in review.
- **The size guardrail** may be raised for this (decided in review). T-284 didn't need it (547 of 550 KiB); T-285
  and T-342 decide.
- **Mermaid is the `@comp-markdown/mermaid` pack in the strict sandbox** (§3c), decided in review, with its
  runtime cost measured in T-285; building Mermaid into the dashboard (D) is to be reconsidered later.
- **Math** showed as its TeX source until CompMD 0.4.0 (decided with Hakan during T-285): `@compmd/sandbox` typeset
  math with Temml inside its frames, but its host mode passed no `math` to the page. CompMD 0.4.0 typesets it in host
  mode too, with the same Temml, loaded into the page the first time a text has math; atm took it in T-398 with no
  atm-specific code, and the page's CSP is unchanged (MathML, no inline styles or scripts).
- **Code highlighting** is CompMD's from 0.4.0 (Shiki, GitHub's light and dark themes, token colours set through the
  CSSOM, so no `style-src` change), in task text and the document viewer alike; atm adds no highlighter.
- **Nothing is open.**
