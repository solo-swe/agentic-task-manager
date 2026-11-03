# Opening referenced documents in a CompMD viewer (T-336)

A task points at documents: a description links `docs/layered-architecture.md`, its `files` list
`docs/compmd-rendering-design.md` and `src/app.ts`, its `requirements` name `SR-7.6`. Today the dashboard shows those
as text: a relative link isn't a link (T-284's link rules keep only `https:`, `http:`, `mailto:` and `#`), and nothing
reads a project's files. This design opens them inside the dashboard, rendered with CompMD as task text is
(`docs/compmd-rendering-design.md`).

Status: **approved by Hakan** (2026-11-03, after three review rounds). §8 has what was decided; the work is in the
backlog tasks of §9.

## 1. Requirements

- **V1. What opens.** Any file of the task's project, shown by what it is (§4a): a Markdown document rendered; source
  code as code; an image as an image; other text as text; a binary file says it is binary. atm only wraps a file
  into a CompMD document (§4a); CompMD renders all of it, code highlighting included.
  - **V1a.** A relative link in task text (`[the design](docs/x.md)`, `docs/x.md#3-routes`, `src/app.ts#L40`),
    resolved from the **project root** (task text has no folder of its own).
  - **V1b.** A task's `files` entries: the Files list shows each as a link.
  - **V1c.** A relative link inside an open document, resolved from **that document's folder**, as Markdown viewers
    and GitHub do (`../v4/data-model.md` from `docs/x.md`).
  - **V1d.** A reference to a requirement, a use case or any place in a document opens the document at that text.
    Decided while planning the sprint: no ID index. References are relative links with a
    [text fragment](https://web.dev/articles/text-fragments#text_fragments)
    (`[SR-2.1](docs/sprints-and-releases-design.md#:~:text=SR%2D2.1)`), in task text and as `requirements` /
    `use_cases` entries; the viewer opens the first match and marks it. The skills teach agents to write them, and one
    sweep converts the open tasks' references (T-391, T-399).
- **V2. Where.** The document opens in a viewer window over the details panel, the task's panel still under it (§2):
  movable by its title bar, resizable from a corner handle, scrolling with the dashboard's styled scrollbar. Back
  (the button, Escape, the browser's Back) returns to the task, or to the previous document. The URL says what is
  open (`#doc=<path>`, with `:L<n>` for a line, beside the task's), so a link can be shared and Reload keeps it.
- **V2a. Line numbers** on the left, as in an editor: every line of code and text; for a Markdown document, the
  source line each block starts on, beside the block. A `#L<n>` link scrolls to the line. Which source lines a
  rendered block came from is CompMD's to say: its renderer is getting an API for it (Hakan, while building T-394),
  so the Markdown half waits for that release (T-400).
- **V2b. The reader keeps their place.** The open document refreshes on the dashboard's poll; a change re-renders it
  where the reader is (the block at the top of the window stays there), never back at the top.
- **V3. Only the project's own files, read-only.** The server reads a file only from inside the project's folder,
  through no symlink, under a size cap, and never a hidden file, a file git ignores, or a key file (§4); no file
  outside the project, no folder listing. Nothing is ever written.
- **V4. Documents are untrusted, as task text is** (anyone who can change the repository wrote them): rendered with
  T-284's settings, with no remote data, no packs but the plugin's (Mermaid), no component code from the document,
  no inputs that change anything.
- **V5. Images render** (`![](./diagram.png)`, common in design documents): CompMD renders them as it renders any
  image; the dashboard points a relative one at the files route (§4), so it loads from this server. An image from
  anywhere else still shows its alt text (V7).
- **V6. Fallbacks say what happened:** a missing file ("not in the project"), one the server won't show ("can't be
  shown here": hidden, ignored, a key file, over the cap), a project whose folder moved (the registry's missing
  project, as today).
- **V7. Nothing leaves localhost.** Files and images come from this server; the page's CSP doesn't change
  (`img-src 'self' data:` already allows this server's images).

## 2. The viewer window (decided: A)

Stacked over the details panel (option A of the first draft; B, a full page, and C, side by side, were set aside):
- **Placement:** opens as wide as the details panel, slightly offset so the task's edge shows beneath.
- **Movable** by its title bar (pointer and keyboard: arrow keys on the focused title bar), kept inside the window.
- **Resizable** from a handle on its bottom-right corner (a visible grip, the cursor changing over it), with a
  minimum size; the keyboard resizes with Shift and the arrow keys.
- **Where it was** (position and size) is kept for the page's next document, in the dashboard's local preferences.
- **Scrollbar:** thin and themed, like the rest of the dashboard's scrolling panels (light and dark).
- On a narrow window it fills the window, without moving or resizing.
- **Title bar** (decided while planning): the title, the path, **Copy path** (with `#L<n>` for a selected line) and
  **Open on GitHub** (the file at HEAD in `meta.repository`'s web view, shown only for a file git tracks).

## 3. Resolving a reference

- A link in **task text** is resolved from the project root: `docs/x.md`, `./docs/x.md` and `/docs/x.md` are the
  same. A `..` that leaves the project is refused (V3) and shows as text.
- A link in a **document** is resolved from the document's folder; the result must still be inside the project.
- `#fragment` opens the document at that heading (the same slug rule as `#` links in task text); `#L<n>` (and
  `#L<n>-L<m>`, highlighted) at that line.
- **`files` entries** are paths from the project root already; each becomes a link.
- The dashboard sends the resolved path; the server checks it again (V3) and never trusts the dashboard's.

## 4. How the server serves files

The server works out what a file is and gives the dashboard something to render, so the dashboard holds no list of
file types. Two read-only routes of the REST API, through atm-lib (`read_file(project, path)`), which asks the store
for the project's root and the vcs whether git ignores the path:

- `GET api/v1/projects/<id>/documents?path=docs/x.md`: what the viewer shows, as JSON (§4a).
- `GET api/v1/projects/<id>/files?path=docs/diagram.png`: an image's bytes, for an `<img>` (V5): only the image
  types below, with their own `Content-Type`, `nosniff`, CORP same-origin, and `Content-Security-Policy: sandbox` so
  even an SVG opened on its own runs nothing (in an `<img>` it never does).

Both answer with an ETag (the file's size and mtime, as the logo route does), so an open document refreshes on the
dashboard's 5-second poll and costs a 304 when unchanged.

### 4a. What a file is

| Kind | How the server decides | The CompMD it gives |
|---|---|---|
| `markdown` | `.md`, `.markdown`, `.comp.md` | the file as it is |
| `code` | an extension in its map (`.ts` → `ts`, `.py` → `python`, `.json`, `.sh`, `.svelte`, … ~60 entries) | the file in a fence of that language, the fence longer than any backtick run in it |
| `image` | `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, and the bytes match | `![name](files URL)` |
| `binary` | a NUL byte or invalid UTF-8 in its first 8 KB | a line saying it is binary, with its size |
| `text` | anything else | the file in a plain fence |

```json
{ "path": "src/app.ts", "kind": "code", "language": "ts", "text": "…", "document": "```ts\n…\n```\n",
  "line_offset": 1, "lines": 120, "bytes": 3214, "modified_at": "2026-11-03T10:00:00Z" }
```

**Line numbers stay the file's, whatever the wrapping** (V2a). The wrapping only ever adds whole lines before the
file's first line and after its last, never inside it: a fence's opening line for `code` and `text` (`line_offset`
1), nothing for `markdown` (0). So:
- `text` is the file as it is; the gutter counts its lines (`lines`), never the document's.
- A position in the `document` (CompMD's node positions, a diagnostic) is the file's line `line − line_offset`.
- `kind` says how the gutter numbers: a number per line for `code` and `text`, the start line of each block for
  `markdown`, none for `image` and `binary`.
- Code and text lines never wrap on screen (the block scrolls sideways), so line *n* of the gutter sits beside line
  *n* of the code at the same line height; a test checks that the gutter and the code are as tall as each other.

### 4b. Checks

Each with a test in `serve.test.mjs` and `tests/lib/`:
- `path` is relative, `/`-separated, normalized; no segment is `..`, `.`, empty, or hidden (`.git`, `.env`,
  `.agentic-task-manager`'s own files too); no NUL or backslash; at most 512 characters.
- Every segment from the project root down is a real directory or the file itself: no symlink anywhere (as the task
  folder's checks do).
- **Not a file git ignores** (`git check-ignore`; new in this round, §8): that is where secrets usually are
  (`.env.local`, `config/credentials.json`, build output). Untracked files that aren't ignored show, so a design
  being written does (decided: not only tracked files).
- **Not a key file:** `*.pem`, `*.key`, `*.p12`, `*.pfx`, `id_rsa*`, `id_ed25519*`, `*.keystore`, refused whatever
  git says.
- A regular file of at most **1 MB** for documents, 5 MB for images (413 above).
- The project is a registered one; a missing project is 404, as for its tasks.

**Alternatives weighed:** serving files raw for everything (`p/<id>/…`) would reopen the raw-file routes T-374
removed and leave type handling to the dashboard; reading through the vcs (`git show HEAD:path`) would show the last
commit rather than the working tree, which a reader editing a design doesn't expect. JSON through atm-lib keeps the
layered architecture's rule that the server reads nothing itself.

## 5. Rendering

- The `document` is processed as its own CompMD document (`path` its name), with the request T-284 builds minus the
  task: the plugin's packs (`@compmd/vite`'s request: Mermaid, ambient), no allowed origins, no `host:` data, and
  **only this file** in `files`, so its imports of other files and data files fail and show their fallbacks. A
  Markdown document's frontmatter is its own (`md.doc`), unlike task text.
- **Images:** a relative image URL becomes the files route's URL for the resolved path (§3); `data:` stays; any other
  image shows its alt text (`safeTree`'s rule for task text, which stays as it is for task text).
- **Links:** a relative link becomes a viewer link (opened by the viewer's click handler, never by the browser);
  web links as in task text.
- **Line numbers (V2a):** CompMD's tree keeps each node's source position. For `markdown`, the gutter shows each
  top-level block's start line, aligned with the block's rendered top (measured after `rendered`, again when the
  window resizes); for `code` and `text`, a number per line of `text` beside the code block. Lines count from the
  file's first line (a document's own frontmatter included), with the server's `line_offset` taken off (§4a).
- **Refreshing in place (V2b):** before re-rendering, the viewer notes the top block's source line and its offset
  in the window; CompMD's renderer updates the page in place, and the viewer scrolls that line back to the same
  offset. A 304 changes nothing at all.
- Diagnostics aren't shown (R2a of the rendering design).

## 6. In the dashboard

- `lib/documents.ts`: resolving references (§3), the URL hash (`#doc=`, `:L<n>`), fetching through
  `src/lib/api/http.ts`, the refresh with its ETag.
- `DocumentViewer.svelte`: the window (§2: move, resize handle, kept geometry, scrollbar), its title (the
  document's first heading, else its path), Back, the line gutter, the fallbacks of V6, and the document in
  `CompMd.svelte` (which takes a document's request parts as well as a task's).
- The details panel's Files list links every entry (V1b); task text's relative links open the viewer (V1a).

## 7. Security document

`docs/security-architecture.md`, with the routes: I11's list (the documents and files routes and their checks), I12
(what of a project the server reads now: its files under its folder, except hidden, ignored and key files, not only
task data), and the untrusted-document rendering (V4, V5: images from this server only). The CSP is unchanged (V7).

## 8. Review

Decided in the first round (2026-11-03):
1. **Requirement and use-case IDs (V1d):** a requirement, built by its own backlog task.
2. **Only files git tracks?** No: the working tree, drafts included.
3. **Placement:** A, movable, resizable with a corner handle, with a styled scrollbar; line numbers on the left.
4. **Images:** rendered, from this server (V5).
5. **Not only Markdown:** the server decides what a file is and gives CompMD to render (§4a).
6. **Refreshing keeps the reader's place** (V2b).

New, to check: **with every file kind open, the server refuses files git ignores and key files** (§4b), beside the
hidden-segment rule. Without it, a task text linking `config/credentials.json` would show its contents in the
dashboard: only to this user on this machine, who can read the file anyway, but in a page an agent may screenshot.

## 9. Follow-up tasks (in the backlog: T-392 to T-396, and T-391)

1. T-392: atm-lib `read_file` (the kinds of §4a, the checks of §4b) and the two REST routes, with their tests; the
   security document (§4, §7).
2. T-393: The viewer window: `lib/documents.ts`, `DocumentViewer.svelte` (move, resize, scrollbar, kept geometry), the
   `#doc=` hash, Back; e2e tests (§2, §6).
3. T-394: Rendering documents: line numbers, refreshing in place, images from the files route (§5); e2e tests.
4. T-395: Links: relative links in task text and documents, `files` entries (§3); e2e tests.
5. T-396: Help and the skills: tell agents that relative links to files open in the dashboard, with `#L<n>` (a line in
   `/atm:create-tasks`).
6. T-391: text-fragment links open the document at the text, and the skills say how to write them (V1d); T-399: one
   sweep of the open tasks' references.
