+++
id = "01a20fc7-4d72-73c9-8103-c3fd8a889ae8"
number = 176
title = "App start: nothing drawn before the page is known; the project's data first; projects listed side by side"
status = "approved"
tasks = ["ATM-776"]
author = "claude:33cba067"
reviewers = []
created = 2026-11-24T22:57:29Z
updated = 2026-11-24T23:01:42Z
+++

The app's start, profiled in the real app (`ATM_PROFILE` timings on both sides, removed after): the page drew at ~0.5 s before it knew anything (tabs named by project ID, the empty Project tab), then waited on `list_projects` (1.5 s in a debug build, 0.47 s in release), which listed every project's stamp and summary one after the other; the active project's own reads were asked only after it.

## The page (`dashboard/`)
- **Nothing drawn before the page is known** (`routes/+layout.svelte`): in the app, `App` is drawn once the router is at the restored tab's address and the route is decided, or the load failed (after 5 s, drawn anyway). The first load starts there, at that address, not from `App`'s `onMount` at the root.
- **The projects kept from the last start** (`lib/kept-projects.ts`): the app keeps each workspace's `list_projects` answer as given, with its tag, and reads it back through the same rule as a fresh one (`load.ts` `projectsFrom`). The first paint names the tabs and shows the page from it; its tag goes with the first ask, so an unchanged list costs atm no summaries (it changes whenever any project's tasks do).
- **The project's data first** (`state.svelte.ts` `fetchAndApplyFor`): from the kept list, the active project's data is asked for and shown at once, the fresh list after it (asked together, they slowed each other down); `refreshProjects` split so the kept list and a fresh one decide the route by one rule (`showRoute`).
- `taskAddress` follows the address (a card's link kept a stale one, without `?sprint=`, once cards drew earlier).

## atm-lib (`workspaces.rs`)
`listing()` reads each project's stamp, working copies, logo and named projects side by side (`par_map`, one project opened once), and `projects()` works out the summaries side by side; the summary cache is locked only to look and to keep. `list_projects` cold: 1508 → 697 ms (debug).

## `Cargo.toml`
Debug builds optimize the dependencies (`[profile.dev.package."*"] opt-level = 2`): `cargo run` / `cargo tauri dev` were ~3× slower than a release. Rebuilding them takes ~6 min once.

## Measured (debug, from the kept list)
First paint 0.55 s, right (was 0.53 s, wrong); the Code Reviews list 0.8 s (2.2); a board's tasks 1.5 s (2.8).

## Tests
`kept-projects.test.ts`; `load.test.ts`; `app.spec.ts`: a review tab as the start (no other page drawn, the list asked once), a first start (nothing drawn while atm lists), a start from the kept list (right tabs and page, tasks before the list, the tag's unchanged answer); atm-lib: `par_map` (order, none, one, many, a panic) and a listing of several projects with one missing.
