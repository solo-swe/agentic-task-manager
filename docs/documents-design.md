# Documents: every document of a project, to read (ATM-728)

**Status:** design, agreed with Hakan (CR-136); §6 records the decisions. Part of [the sidebar overhaul](sidebar-overhaul-design.md) (its
Documents item, §1); the build becomes tasks of the sprint *Sidebar overhaul* once agreed.

## 1. In short

- **Documents lists a project's documents**, the Markdown files in its repository, as a tree by folder with each
  one's title, and opens one in the document viewer to read, in place in the tab.
- **The listing is atm's** (atm-docs, beside the viewer's reads), with the viewer's rules: only files the viewer
  would show (never git's or atm's folders, environment or key files, or what git ignores).
- **Nothing new to read a document:** the viewer, its address (`?doc=path`), its links and its CompMD rendering are
  as they are today. This page is how you find one.

## 2. Which files

- **Markdown:** `*.md` and `*.markdown`, the files CompMD renders as documents. (Code and text files open in the
  viewer from links, as now, but aren't listed: a repository's code is not its documents.)
- **Where git has them:** tracked files (`git ls-files`) and untracked ones git doesn't ignore
  (`git ls-files --others --exclude-standard`), so a document being written shows before its commit. In a project
  without git: none (the page says the project needs git for its documents).
- **Refused as the viewer refuses** (atm-docs' `document_path`): `.git/`, `.agentic-task-manager/`, environment
  files, key files. Hidden folders otherwise listed (`.github/`), as the viewer shows them.
- **Submodules are included** (Hakan, CR-137): a submodule checked out in the project is listed as a folder, marked
  as a submodule, holding its documents by the same rules, found in the submodule's own repository (its tracked
  files and its untracked ones its own rules don't ignore), as the viewer already reads a file in one (T-408). Nested
  submodules too. One that isn't checked out (`git submodule status` says `-`) shows as an empty folder saying so.
- **At most 500 documents** (Hakan, CR-137); past that, the first 500 by path and a line saying how many more there
  are. The search (§4) then asks the route (`?q=`), which matches before it cuts, so every document can be found.

## 3. What each entry says

| Field | From |
|---|---|
| `path` | the file's path from the project's root |
| `title` | its first `# ` heading (ATX, or a Setext `===` heading) in its first 8 KB, else the file's name without `.md` |
| `bytes`, `modified_at` | the file's size and modification time |
| `tracked` | whether git tracks it (an untracked one is marked *new*) |

Reading each title costs a read of the file's start: the answer is cached by the listing's stamp (git's index, `HEAD`
and the listed files' sizes and mtimes), as the other routes' `ETag`s are.

## 4. The page (Documents item)

```
┌──────────────────────────────────────────────────────────────┐
│ [ Search documents…                ]   Tree · Recent         │  in the tab's header line
├──────────────────────────────────────────────────────────────┤
│ ▾ docs                                                   42  │
│   ▾ adr                                                   7  │
│       ADR-003 Layered crates           ADR-003-….md   3 d    │
│   The sidebar overhaul               sidebar-overhaul…  2 h  │
│   Tabs, pop-ups, Back and Forward    tabs-and-navig…    5 d  │
│ ▸ packaging                                               1  │
│   README.md  Agentic Task Manager                       1 w  │
│   CHANGELOG.md  Changelog                       new     2 h  │
└──────────────────────────────────────────────────────────────┘
```

- **Two views:** *Tree* (folders, open or closed, the root's files after its folders) and *Recent* (every document by
  last change, newest first). The view and the search are in the address
  (`/projects/<id>/documents?list=recent&q=sidebar`; `list`, as `view` is the address's own name for a project's
  page, ATM-766); the open folders are kept per project in the preferences
  (they would make the address long).
- **Each row** is its title, its file name, and how long ago it changed; an untracked one says *new*.
- **The search** filters by title and path as you type, in the page (the list is already loaded; when the route cut
  it at 500, by asking the route with `?q=` instead), in the tab's header
  line where the other searches are (the sidebar design §6).
- **A click opens the document in the viewer, in place**: `?doc=docs/x.md` over the Documents page, a step in the
  tab's history, so Back closes it and returns to the list where it was. ⌘-click opens it in a new tab.
- **Refreshed** as the other pages are (the dashboard's poll, by `ETag`).

## 5. API and the app's command

| Route | Answer |
|---|---|
| `GET …/projects/<id>/documents` (no `path`; `?q=` matches titles and paths, at most 200 characters, before the cap) | `{documents: [{path, title, bytes, modified_at, tracked}], submodules: [{path, checked_out}], more: n}` (a submodule's documents among `documents`, by their path from the project's root); `ETag` |

- The same route's `?path=` answers one document, as now; without it, the list. `…` is `api/v2/workspaces/<w>` (and
  the v1 form for the default workspace).
- **Read-only, a new entry point:** it lists paths of the project, so it keeps I11 and I12's rules: the Host check,
  a GET refused from another site, only the registered projects, nothing outside the project, through no symlink,
  git run with literal pathspecs (I7). `docs/security-architecture.md` (I11, I12) and `docs/multi-project-api.md`
  §2.6 are updated in the same change.
- **The app's command** `project_documents` mirrors it (I27).
- **Layers:** atm-docs `list_documents` (the rules of §2 and §3), atm-vcs the two `ls-files` calls, atm-lib the call,
  atm-api the route; the dashboard `lib/documents/DocumentsPage.svelte` and `api/documents.ts`.

## 6. Decisions (Hakan, CR-136: the recommendations)

1. **Markdown only** (`*.md`, `*.markdown`).
2. **Untracked documents are listed**, marked *new*.
3. **Tree is the first view.**
4. **Submodules' documents are included** (CR-137), each submodule a folder marked as one.
5. **At most 500 documents** (CR-137), the search finding the rest.

## 7. Tasks it implies

1. **The listing:** atm-docs `list_documents` (which files, titles, the cap), atm-vcs's listing, atm-lib; unit tests
   for each rule (refused paths, ignored and untracked files, submodules, titles, the cap).
2. **The route and the command:** §5, its `ETag`; the security document and `multi-project-api.md`; serve tests.
3. **The page:** Tree and Recent, the search, opening in the viewer in place; component tests and an e2e spec.
