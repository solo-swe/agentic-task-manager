+++
id = "01a20ea0-61d0-7188-ad2d-860cdb6d7544"
number = 167
title = "App: answer unchanged instead of re-sending an unchanged answer"
status = "approved"
tasks = ["ATM-768"]
author = "claude:ba530ebb"
reviewers = []
created = 2026-11-24T17:35:21Z
updated = 2026-11-24T17:36:39Z
+++

In the app, an unchanged reload now carries no body, as a `304` doesn't in the browser (ATM-768, from [the memory analysis §3b](docs/memory-growth-analysis.md#3b-what-a-reload-does-in-the-app-the-code-cr-140)). Before, every change message reloaded the whole answer: 1.16 MB of task views for this repo, plus the project list.

**The app (`commands/state.rs`, `commands/reads.rs`):**
- `Fresh {tag, body}`: the reload commands' answer. `tag` is what the answer depends on, hashed the same way atm serve hashes its `ETag`. `body` is null when the page's `known` tag still matches, and in that case it isn't worked out at all.
- `list_projects`, `project_tasks`, `claims` and `worktrees` take `known` and depend on what their routes' `ETag`s depend on:
  - the project list: the listing's stamp;
  - the tasks: the task data, the archive, the filters and the other projects their tasks name;
  - the claims: the claims store and the claims held now;
  - the worktrees: the working copies and the claims.
- Task data that's too large is still refused first. The plain `*_of` reads are unchanged; the REST-agreement test still compares them with the routes.

**The page (`api/ipc.ts`, `api/http.ts`):**
- `ipcFetch` takes the request's options. For a reload command it sends the `If-None-Match` as `known`, and answers the tag as the `ETag`, with no body as a `304`. The client handles that as it does in the browser.
- `browserFetch` passes the options on in the app.

**Not covered here:** the page asks for claims without validators, in the browser too (`claims.ts`), so their reload still carries its body (a few hundred bytes). The command is ready for when the page asks conditionally.

**Tests:**
- `a_reload_answers_its_body_unless_the_page_has_it`: the tag, a matching one (with the body not worked out), stale ones, the body's error.
- `a_reload_with_nothing_new_carries_no_body`, on a real state folder for all four reads:
  - none known, then each tag given back;
  - a stale tag, and other filters;
  - a task added, which changes the tasks and the project list;
  - an unknown project.
- `ipc.test.ts`: `known` taken from `If-None-Match` (a record or `Headers`), the `ETag` and the `304`, the client keeping the tag and then getting unchanged, and other commands asked as before.
- `app.spec.ts`: the fixture's commands answer as `Fresh`. A change message with nothing new reloads with no body; a real change carries the new tasks.
- `cargo test -p atm-app` (122 passed), clippy, `pnpm run check`, the unit suite (2082 passed), the functional e2e suite (624 passed).
