# Opening referenced documents in a CompMD viewer (T-336)

A task points at documents: a description links `docs/layered-architecture.md`, its `files` list
`docs/compmd-rendering-design.md`, its `requirements` name `SR-7.6`. Today the dashboard shows those as text: a
relative link isn't a link (T-284's link rules keep only `https:`, `http:`, `mailto:` and `#`), and nothing reads a
project's documents. This design opens them inside the dashboard, rendered with CompMD as task text is
(`docs/compmd-rendering-design.md`).

Status: **draft for review with Hakan.** The open questions are in §8; nothing is built until it is approved.

## 1. Requirements

- **V1. What opens.** A Markdown document of the task's project: `.md`, `.markdown` and `.comp.md` files.
  - **V1a.** A relative link in task text (`[the design](docs/x.md)`, `docs/x.md#3-routes`), resolved from the
    **project root** (task text has no folder of its own).
  - **V1b.** A task's `files` entry that is a Markdown document: the Files list shows it as a link.
  - **V1c.** A relative link inside an open document, resolved from **that document's folder**, as Markdown viewers
    and GitHub do (`../v4/data-model.md` from `docs/x.md`).
  - **V1d (later, §8 Q1).** A `requirements` or `use_cases` ID (`SR-7.6`, `UC-001`) opens the document that defines
    it, at its place.
- **V2. Where.** The document opens in a viewer stacked over the details panel, the task's panel still under it:
  Back (the button, Escape, the browser's Back) returns to the task, or to the previous document. The URL says what
  is open (`#doc=<path>`, beside the task's), so a link to a document can be shared and Reload keeps it.
- **V3. Only the project's own documents, read-only.** The server reads a document only from inside the project's
  folder, through no symlink, of a Markdown extension, under a size cap; nothing else (no `.env`, no binary, no file
  outside the project, no folder listing). Nothing is ever written.
- **V4. Documents are untrusted, as task text is** (anyone who can change the repository wrote them): rendered with
  T-284's settings and `safeTree`, with no remote data, no packs but the plugin's (Mermaid), no component code from
  the document, no inputs that change anything.
- **V5. Images in documents** (`![](./diagram.png)`, common in design documents) show their alt text, as in task
  text, in the first version (§8 Q4).
- **V6. Fallbacks say what happened:** a missing file ("not in the project"), a file over the cap or not Markdown
  ("can't be shown here"), a project whose folder moved (the registry's missing project, as today).
- **V7. Nothing leaves localhost**, and the page's CSP doesn't change: documents arrive as JSON from this server.

## 2. Where the viewer opens

| Option | For | Against |
|---|---|---|
| **A. Stacked over the details panel** (recommended) | the task stays one Back away; reading a design next to the task that cites it is the common case; the panel's width suits prose | two layers of overlay; on a narrow window it covers the task |
| B. A full page (a new view, `#view=doc`) | the most room for long documents | leaves the board; the task isn't one step away |
| C. Side by side with the panel | task and document both visible | the panel is already wide; two columns of prose are cramped below ~1400 px |

**Recommended: A**, as wide as the details panel and slightly offset so the task's edge shows beneath. A wide window
could later show A side by side (C) with no change to the rest.

## 3. Resolving a reference

- A link in **task text** is resolved from the project root: `docs/x.md` and `./docs/x.md` are the same; `/docs/x.md`
  (rooted) means the same too. A `..` that leaves the project is refused (V3) and shows as text.
- A link in a **document** is resolved from the document's folder; the result must still be inside the project.
- `#fragment` opens the document at that heading (the same slug rule as `#` links in task text).
- A link to anything that isn't a Markdown document (`src/app.ts`, `logo.png`) shows as text, as now.
- **`files` entries** are paths from the project root already; those ending in a Markdown extension become links.
- The dashboard sends the resolved path; the server checks it again (V3) and never trusts the dashboard's.

## 4. How the server serves documents

A new read-only route of the REST API: `GET api/v1/projects/<id>/documents?path=docs/x.md`, through atm-lib
(`read_document(project, path)`), which asks the store for the project's root and the vcs for whether the file is
tracked (§8 Q2). It answers JSON:

```json
{ "path": "docs/x.md", "text": "…", "bytes": 18234, "modified_at": "2026-11-03T10:00:00Z" }
```

with an ETag (the file's size and mtime, as the logo route does), so an open document refreshes on the dashboard's
5-second poll and costs a 304 when unchanged.

**Checks** (each with a test in `serve.test.mjs`):
- `path` is relative, `/`-separated, normalized; no segment is `..`, `.`, empty, or hidden (`.git`, `.env`, and
  `.agentic-task-manager`'s own files too); no NUL or backslash; at most 512 characters.
- The extension is `.md`, `.markdown` or `.comp.md` (case-insensitive); anything else is 404.
- Every segment from the project root down is a real directory or the file itself: no symlink anywhere (as the task
  folder's checks do).
- A regular file of at most **1 MB** (413 above), read as UTF-8 (a file that isn't is 415).
- The project is a registered one; a missing project is 404, as for its tasks.

**Alternatives weighed:** serving documents as raw files (`p/<id>/docs/…`, `text/markdown`) would reuse nothing and
reopen the raw-file routes T-374 removed; reading them through the vcs (`git show HEAD:path`) would show the last
commit rather than the working tree, which a reader editing a design doesn't expect. JSON through atm-lib keeps the
layered architecture's rule that the server reads nothing itself.

## 5. Rendering

- The document is processed as its own CompMD document (`path` its name), with the request T-284 builds minus the
  task: the plugin's packs (`@compmd/vite`'s request: Mermaid, ambient), no allowed origins, no `host:` data, and
  **only this file** in `files`, so its imports of other files and data files fail and show their fallbacks. Its own
  frontmatter is its own (`md.doc`), unlike task text.
- `safeTree` and the link pass apply, extended for V1c: a relative link to a Markdown document becomes a viewer link
  (opened by the viewer's click handler, never by the browser), any other relative link text.
- Diagnostics aren't shown (R2a of the rendering design).

## 6. In the dashboard

- `lib/documents.ts`: resolving references (§3), the URL hash (`#doc=`), fetching through `src/lib/api/http.ts`.
- `DocumentViewer.svelte`: the stacked panel, its title (the document's first heading, else its path), Back, the
  fallbacks of V6, and the document in `CompMd.svelte` (which takes a request's parts instead of a task, or a small
  sibling component).
- The details panel's Files list links Markdown entries (V1b); task text's relative links open the viewer (V1a).

## 7. Security document

`docs/security-architecture.md`, with the route: I11's list (the documents route and its checks), I12 (what of a
project the server reads now: Markdown documents under its folder, not only task data), and the untrusted-document
rendering (V4, V5). The CSP is unchanged (V7).

## 8. For review

1. **Requirement and use-case IDs (V1d):** leave them for later (recommended: atm has no map from an ID to the
   document that defines it, and building one means indexing every document for ID patterns), or include a simple
   version now: a `meta.documents.requirements` setting naming the documents to search, and the viewer opening the
   first document whose text holds the ID, at that line?
2. **Only files git tracks?** Recommended: **no**, the working tree as it is (a design being written isn't committed
   yet), with the extension allowlist, the hidden-segment rule and the cap keeping secrets out (a secret in a tracked
   or untracked `.md` file is equally visible on disk to the same user). Yes would also hide drafts.
3. **Placement:** stacked over the details panel (A, recommended), a full page (B), or side by side (C)?
4. **Images in documents:** alt text only for now (recommended), or a follow-up for local images (a route for a
   project's image files, with `img-src 'self'` already allowing them; SVG served as an image, never as a document)?

## 9. Follow-up tasks (proposed once approved, through `/atm:create-tasks`)

1. atm-lib `read_document` and the REST route, with its checks and tests; the security document (§4, §7).
2. The viewer: `lib/documents.ts`, `DocumentViewer.svelte`, the `#doc=` hash, Back; e2e tests (§6).
3. Links: relative links in task text and documents, Markdown `files` entries (§3, §5); e2e tests.
4. Help and the skills: tell agents that relative links to documents open in the dashboard (a line in
   `/atm:create-tasks`).
5. If approved: requirement IDs (Q1), local images (Q4).
