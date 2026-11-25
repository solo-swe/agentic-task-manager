+++
id = "01a20ef6-0b8d-7516-89ae-99dbe3f53821"
number = 171
title = "Documents: the listing route and the app's command"
status = "approved"
tasks = ["ATM-765"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T19:08:55Z
updated = 2026-11-24T19:16:47Z
+++

The documents' listing route and the app's command (ATM-765, [the documents design §5](docs/documents-design.md#5-api-and-the-apps-command)), on top of ATM-764's atm-lib listing.

**The route (`atm-api` `routes/documents.rs`):**
- `GET …/projects/<p>/documents` with no `path` (and no `at`) answers the listing as atm-lib gives it: `{documents, submodules, more}`. `?q=` names some. v1 and v2.
- **The `ETag`** is the listing's stamp (`Member::documents_stamp`) with the query. Asked again with it, the answer is a 304 and the listing isn't made; a project outside git has no stamp and isn't tagged.
- **What atm-lib refuses** keeps its status (a query over 200 characters is a 400).
- **`path=&at=<rev>`** still names a document: a bad revision there is refused as before, not listed.
- **`ProjectSource`** gains `documents` and `documents_stamp`, implemented by `Member`, with fakes in `test_support`.
- **One behaviour change:** `documents` with an empty `path` was a 400 and is now the listing. `files` with none is still a 400.

**The app:**
- `project_documents(workspace, project, q?)` is the same atm-lib call (I27). It's registered in `lib.rs`, `build.rs` and the main window's capability.
- `ipc.ts` maps the route without a path to the command, and the route with `at` and no path stays `document`.

**Docs:** `multi-project-api.md` §2.6 and the route tables; `security-architecture.md` I11 (the new read entry point and its rules) and I27.

**Tests:**
- atm-api:
  - the listing for each form of the query, tagged by stamp and query;
  - a 304 without the listing being made, until the stamp or the query changes;
  - no stamp, no tag;
  - each refusal, of the stamp or of the listing.
- The app:
  - `the_documents_listing_answers_what_its_rest_route_does` (every one, a query, nothing found, a query too long refused by both, an unknown project);
  - the capability test calls `project_documents`.
- `ipc.test.ts`: the listing's command, with and without `q`, and `at` still a document.
- `serve.test.mjs`, a new test on a real project:
  - only the Markdown files git lists, tracked or new: not the ignored one, nor one through a symlinked folder, nor code;
  - v2 the same;
  - the ETag and its 304, and a new document changing it;
  - `q`, and a query too long refused;
  - refused from another site, not found for an unregistered project, 405 for a POST;
  - the old empty-path case moved to `files`.
- `cargo test -p atm-api` (83) and `-p atm-app`, clippy, `pnpm run check`, the unit suite (2088 passed), the serve tests (54 passed), the functional e2e suite (627 passed).
