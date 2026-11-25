+++
id = "01a20f37-a278-762d-a8c7-1a5a8559b65d"
number = 173
title = "Documents: the page"
status = "approved"
tasks = ["ATM-766"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T20:20:34Z
updated = 2026-11-24T20:25:35Z
+++

The Documents item's page (ATM-766, [the documents design §4](docs/documents-design.md#4-the-page-documents-item)), in place of its "coming" placeholder, on ATM-765's listing.

**The page (`documents-page/DocumentsPage.svelte`):**
- **Tree** (the default): folders, then the root's files, each by name. A folder shows its count, and a submodule is marked as one ("not checked out" when it isn't).
  - Folders start closed. The open ones are kept per project in the preferences (`documentFolders`).
  - While searching, every folder is open and the submodules aren't shown.
- **Recent:** every document by last change, newest first, with its path.
- **Each row:** its title, file name, how long ago it changed, and *new* when git doesn't track it.
- **The search** is the header line's, where the reviews' is: `ReviewSearch` and its registry now take a placeholder and title. It filters titles and paths in the page. When atm cut the list at 500, it asks atm (`?q=`), which searches every document, and below the list it says how many more there are.
- **The address:** the view and the search are in it, `?list=recent&q=…`. The design said `?view=`, but `view` is already the address's own name for a project's page, so `?view=recent` made `/projects/p1/recent`. The design is corrected.
- **A click** opens the document in the viewer over the page (`?doc=`, a step Back undoes). ⌘-click opens a tab of the app's own, or in the browser the link's own tab.
- **Kept current:** asked again every 10 s while in view, by its `ETag`.
- **Says so:** loading, no documents yet, a project outside git (atm's `git: false`), nothing found, and a listing that can't be read.
- **Its own area folder:** `lib/documents/` is the viewer's, which the boundary rule keeps free of the app's state, so the page is in `lib/documents-page/`.

**The client:** `api/documents.ts` `loadDocuments` (conditional, drops anything unexpected, a missing title becomes the file's name), and `http.ts` `documentsUrl`. The list's rules are a plain module (`document-list.ts`): the address, the search, Recent, the tree and `foldersOf`.

**Tests:**
- `document-list.test.ts`: the address both ways, the search, Recent's order, and the tree (empty, nested, counts, submodules including one not checked out).
- `api-documents.test.ts`: the route and the app's command, parsing, outside git, a 304 and an error.
- `DocumentsPage.component.test.ts`:
  - the tree, folders opening and kept, and kept per project;
  - Recent;
  - the search in the page, and atm's search when the list was cut;
  - a click, and ⌘-click in the browser and in the app;
  - the empty, no-git and unreadable states;
  - asked again by its validators.
- `documents-page.spec.ts`: the tree, a document opened over the page, Back to the list, the folder kept over a reload; the header search, Recent, and both in the address.
- The e2e fixture plays the listing route. `tabs.spec.ts` now finds the page.
- `pnpm run check`, the unit suite (2121 passed; `boundaries.test.ts` timed out once under the suite's load and passes alone), the functional e2e suite (630 passed).
