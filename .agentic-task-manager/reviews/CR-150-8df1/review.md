+++
id = "01a20c5d-1e62-7295-a269-6291645a8df1"
number = 150
title = "Inbox: REST routes and the app's commands"
status = "approved"
tasks = ["ATM-761"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T07:02:38Z
updated = 2026-11-24T16:37:45Z
+++

The inbox's REST routes and the app's commands (ATM-761, inbox design §7), with the security doc in the same change.

**Routes** (`crates/atm-api/src/routes/inbox.rs`), under `…/workspaces/<w>/projects/<p>/`:
- `GET inbox?state=&archived=`: the items, the open ones by default, with the counts. It has an `ETag` taken from what it holds, so a poll of an unchanged inbox is a 304. v1's `api/v1/projects/<p>/inbox` too.
- `POST inbox` `{text}`, `PATCH inbox/<IN>` `{text}`, and `POST inbox/<IN>/decisions` `{decision: discarded, reason}` or `{decision: restored}`. Writes are v2 only, as the review routes' are.
- **Guards: exactly the review routes'.** `serve_request` now lets a write through on the inbox's paths as well, behind the same check: Host, the server's own Origin, the session token, JSON, at most 1 MB. Each body is read with `deny_unknown_fields`. Each route is one atm-lib call, made as the user at the machine, so atm-lib's refusal of an agent adding or restoring holds.
- **No inject route**, and an `injected` decision is refused.
- **Added beyond the task:** a request body must be an object. serde fills a struct from a JSON list in order, so `["x"]` was accepted as `{text: "x"}`. The review routes have the same gap; I left them alone (tell me if you want a task).

**atm-lib:** `NewItem`, `ItemEdit`, `ItemDecision` (`discarded` or `restored`; a discard needs a reason, a restore takes none), `decide`, `listing_of` (shared with the CLI).

**The summary's `inbox: {open}`**, for the Inbox badge. **Please note:** the store's stamp now includes `inbox.json`, and so does the summary's cache key (`summary_stamp`). Without them, a new item wouldn't have moved the count or the `ETag` of `api/projects`. The serve test caught the second one: the list kept answering 304 after an item was added. `scenarios/inbox.json` is only re-formatted here (it went in with ATM-760 unformatted).

**The app's commands:** `project_inbox`, `add_inbox_item`, `edit_inbox_item` and `decide_inbox_item`, each its route's atm-lib call as the user. They're in `build.rs`'s manifest and the main window's capability.

**Docs:**
- `docs/security-architecture.md`: I11 (the inbox's write routes behind the review routes' checks, no inject, and its read) and I27 (the app's commands).
- `docs/multi-project-api.md`: §2.10 (the routes) and the summary's `inbox` field.

**Tests:**
- atm-api `routes::inbox` against a fake atm-lib: every route and its call; the state and archive queries; bad queries refused before atm-lib is asked; the `ETag` and 304; every refusal's status; bodies that aren't the request (an unknown field, a list, an injection); 404 and 405 before the project is opened. Also the v2 and v1 read in the dispatch table.
- `serve.test.mjs` "inbox (ATM-761)": each route against a real server, as the user; refused without the token, with another token, from another origin, as a form, with an unknown field, as a list, as an injection, too large; and no change on v1's path.
- atm-lib: `decide`, the request types, `listing_of`.
- The app: `the_inbox_commands_answer_what_the_page_reads_and_only_from_the_apps_page`.
- clippy, `cargo test` and the CLI suite pass.
