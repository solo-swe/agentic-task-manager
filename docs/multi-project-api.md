# The multi-project server: routes and formats

**Status:** agreed format for phase 15 (T-165) · **Design:** [the proposal](multi-project-dashboard-proposal.md)

One `atm service` per user serves the dashboard and every registered project. This page fixes what the server answers
and what it stores, so atm-cli (the registry and the server) and the dashboard (which reads them) can be built in
parallel. The e2e `tasksServer` fixture and the dev server (`dashboard/build/dev-data.ts`) implement the same routes.

## 1. Rules for every request

- The server binds to `127.0.0.1` only.
- **Host check.** A request whose `Host` header isn't `localhost:<port>` or `127.0.0.1:<port>` (the server's own port)
  gets `403` with `{"error": "unexpected Host header"}`, whatever the path. This blocks DNS rebinding.
- Only `GET` and `HEAD`. Anything else gets `405`.
- Every response has `Cache-Control: no-store`, except a project's logo (§2.3). No CORS headers are sent.
- Error responses are JSON, `{"error": "<what went wrong>"}`, with `Content-Type: application/json`.
- **Conditional requests.** Responses marked *ETag* below carry a strong `ETag`. A request with a matching
  `If-None-Match` gets `304` with no body. The dashboard always sends the last ETag it got.

## 2. Routes

| Route | Answer | ETag |
|---|---|---|
| `GET /` and the dashboard's other pages (§2.7) | the dashboard (`text/html`), from the plugin, held in memory since the server started | no |
| `GET /api/projects` | the registered projects with their summaries (§3) | yes |
| `GET /api/search?q=<text>` | the tasks matching a search, in every project (§2.4) | no |
| `GET /p/<id>/claims.json` | that project's live claims, from every worktree (§2.1) | yes |
| `GET /p/<id>/worktrees.json` | that project's git worktrees and their changed tasks (§2.2) | yes |
| `GET /p/<id>/logo` | that project's logo image (§2.3) | yes |
| `GET /api/v1/projects/<id>/tasks` | that project's tasks, each with what atm derives for it (§2.5) | yes |
| `GET /api/v1/projects/<id>/documents?path=<p>` | a file of that project, for the document viewer (§2.6) | yes |
| `GET /api/v1/projects/<id>/files?path=<p>` | an image file of that project, its bytes (§2.6) | yes |
| `GET /__atm/whoami` | who this server is (§4) | no |
| `GET /_app/version.json` | `{"version": …}`, the page's own version: SvelteKit's check for a new deployment, which it makes when the tab regains focus (atm's reload offer comes from whoami) | no |
| `GET /api/v2/workspaces` | the workspaces (§2.8) | no |
| `GET /api/v2/workspaces/<w>/projects` | that workspace's projects with their summaries (§3); opens it | yes |
| `GET /api/v2/workspaces/<w>/search?q=<text>` | the tasks matching a search, in its projects (§2.4) | no |
| `GET /api/v2/workspaces/<w>/projects/<id>/tasks` | as `api/v1/projects/<id>/tasks`, in that workspace | yes |
| `GET /api/v2/workspaces/<w>/projects/<id>/documents?path=`, `files?path=` | as the v1 file routes (§2.6) | yes |
| `GET /api/v2/workspaces/<w>/projects/<id>/claims`, `worktrees`, `logo` | as `/p/<id>/claims.json`, `worktrees.json`, `logo` | yes |
| anything else | `404` | |

The routes without a workspace (`api/projects`, `api/search`, `api/v1/…`, `/p/…`) are the `default` workspace's
(ADR-003 §5), until the dashboard has moved to the v2 ones (T-457).

- `<id>` is a project id from `/api/projects` (§3.1). An unknown id gets `404` with `{"error": "unknown project"}`.
- Only those names are answered for a project: `claims.json` and `worktrees.json`, computed through atm-lib, and
  `logo`. The task files themselves (`tasks.json`, `tasks-archive.json`) are never served: their tasks come through the
  v1 route (§2.5), read by atm-lib (T-374; before, the server sent the files as stored). There is no other name, no
  directory listing, no path segments after the name; anything else gets `404` with `{"error": "not found"}`.
- A missing project (§3.1 `missing: true`), or one whose task folder atm refuses (a symlink, docs/security-review.md
  F7, F8), answers `404` for all of these.
- An ETag is a hash of what the answer depends on (the store's stamps: its files' sizes and nanosecond mtimes, and the
  query), so two atm changes in the same second are two ETags.
- Query strings are ignored, except `q` on `api/search`, the filters on the v1 route and `path` on the file routes.
- The Projects Overview reads every listed project's tasks and archive (the v1 route, with `?archived=true` for the
  archive, T-355) and `claims.json` while it is shown, conditionally on each refresh (an unchanged answer is `304`),
  for its views across projects: the Agents tab (T-333), the title bar's Activity (T-328) and the Progress tab (T-329).
  There is no route of their own.

### 2.1 `claims.json`: claims from every worktree

The claims of all a clone's worktrees are in one store, in the git common directory (`.git/atm/claims.json`; outside
git, `claims-store.json` in the task folder). The server answers from it, as `{generated_at, claims: [...]}` sorted by
task, so an agent at work in a linked worktree shows on the project's board. Its ETag changes with the store. Before
the first claim there is no store yet, and the answer is an empty list.

### 2.2 `worktrees.json`: the project's git worktrees

```json
{
  "generated_at": "2026-10-26T04:30:00Z",
  "worktrees": [
    { "path": "/Users/me/workplace/agentic-task-manager", "branch": "main", "main": true },
    {
      "path": "/Users/me/workplace/atm-agent-2",
      "branch": "agent-2",
      "main": false,
      "tasks": [
        {
          "task": { "id": "T-179", "status": "in_progress", "...": "the task as that worktree's tasks.json has it" },
          "derived": { "kind": "active", "ready": false, "waiting_for": [], "...": "as the v1 tasks route's (§2.5)" }
        }
      ]
    }
  ]
}
```

- The worktrees are those `git worktree list` shows, main first, read from the git directory (`.git/HEAD`,
  `.git/worktrees/*/gitdir` and `HEAD`). A linked worktree whose folder is gone is left out. `branch` is the checked-out
  branch, the short SHA of a detached HEAD, or `null`. Outside git the list is empty.
- Each linked worktree lists the tasks of its own `tasks.json` that differ from the main worktree's copy, or that only
  it has; the main worktree's tasks are `tasks.json` itself. A task blocked or in review there but already decided in
  another worktree (the claims store's `resolved`) is left out: that copy is stale until atm runs there next.
- Each is a view, `{task, derived}`, like the v1 tasks route's (`TaskView`, §2.5): the derived values worked out on
  that worktree's own data, as `atm loop next` there would (a dependency done in another worktree counts once the branches
  merge; T-387). The dashboard takes them as they are and applies no rule of its own to a copy.
- The ETag changes when a worktree is added or removed, switches branch, its `tasks.json` changes, or the claims
  store does.
- The dashboard shows all of them on one board: each task from the worktree where it got furthest (by status kind:
  todo, blocked, active, review, then done or skipped; the main worktree's copy on a tie), with that worktree's branch
  on its card.

### 2.3 `logo`: the project's logo

- **Which file.** `meta.logo` in the project's `tasks.json`: a path relative to the project root, set with
  `atm project logo set <path>` (and removed with `atm project logo unset`). Without it, the first usable file of `logo.svg`,
  `logo.png`, `.github/logo.*`, `public/favicon.*`, `assets/logo.*` (each `.*` tried as `.svg`, `.png`, `.webp`,
  `.ico`). A `meta.logo` that is unusable means no logo: nothing is searched for then.
- **Usable** means an SVG, PNG, WebP or ICO file (by its extension), inside the project folder once symbolic links are
  followed, and at most 512 KB. `atm project logo set` refuses anything else; the server answers `404` with
  `{"error": "no logo"}` for a project without a usable logo.
- `Content-Type` is the image type (`image/svg+xml`, `image/png`, `image/webp`, `image/x-icon`). Unlike the other
  routes it has `Cache-Control: no-cache`: the browser may keep it but asks again, and gets `304` while it is unchanged
  (the ETag is the file's size and mtime). It also has `Content-Security-Policy: default-src 'none';
  style-src 'unsafe-inline'; sandbox` and `X-Content-Type-Options: nosniff`, so an SVG opened on its own never runs a
  script.

### 2.4 `api/search`: tasks across projects

For the All projects board's search box (T-258): `{ "<project id>": ["T-001", ...] }`, the IDs (in file order) of
each project's `tasks.json` tasks, not the archive's, in which every word of `q` appears, case-insensitively, as the
dashboard's own search finds them: in the ID, name, description, type, labels or note texts, or the project's name
(so a project's name finds all its tasks). Projects without a match are left out; an empty `q` gives `{}`.

Only the first 200 characters of `q` count, and at most 200 IDs come back per project. Like every route it is
read-only and answers only on loopback with a matching Host (§1); other parameters are ignored. The Vite dev server
answers the same.

### 2.5 `api/v1/projects/<id>/tasks`: tasks with what atm derives

The first of the v1 routes (docs/layered-architecture.md §6.2), answered by atm-lib (`task_views`): the dashboard
takes atm's rules from here instead of applying them itself. `{ "generated_at", "tasks": [{ "task", "derived" }] }`,
typed by `api.schema.json` (`TaskViewList`, generated into `dashboard/src/lib/generated/types.ts`): `task` as stored,
`derived` with its `kind`, `ready`, `waiting_for` (a to-do task's unfinished dependencies), `waiting_on` (a blocked
task's), `priority` (or the default), `effective_review`, `effective_release`, `stage` (a done task's: `done`,
`in_pr`, `merged`) and `archived`; what doesn't apply is left out.

- **A malformed task file is shown, not refused** (T-377): the route reads it tolerantly, as far as it can be read,
  where every `atm` command refuses it (`atm task list --derived --tolerant` prints the same). An entry that isn't a task
  is left out; a task with an ID atm doesn't allow is kept, one whose `dependencies` aren't a list of IDs with the IDs
  that can be read; a vocabulary `meta` can't use falls back to its default (left out of the `meta` sent). Each
  problem is said, in `atm doctor`'s words: a task's in its `derived.problems` (it isn't `ready` then), the file's in
  the response's `problems`. The dashboard shows a notice and each task's problems in its details. A file that isn't
  a task file at all still gets an error.
- **The archive** (T-372): `?archived=true` gives the archive's tasks, as views; the dashboard loads its archive from
  here, no longer from `tasks-archive.json` (a project without one: `404`, an empty archive). Without it, the response
  carries `archive_hint` when atm suggests archiving (its `bytes`, `finished`, `archivable`, `auto` and `text`, as
  `atm project status --json` has it): the dashboard's footer notice.
- **Filters**, as `atm task list` takes them: `status`, `label` and `sprint` repeatable (`?status=todo&status=blocked`);
  `type`, `priority`, `agent`, `release`, `component`; the flags `backlog`, `ready`, `blocked`, `in_pr`, `with_user`
  and `archived` (`true` or `1`). Other parameters are ignored. A filter naming no such sprint or release gets `400`.
- The ETag changes with the project's `tasks.json`, its archive and the query. A `tasks.json` over the size limit gets
  `413`, as the raw file does; an unknown project `404`.

### 2.6 `documents` and `files`: a project's files for the document viewer

The document viewer's routes (T-392, docs/compmd-viewer-design.md §4), answered by atm-lib (`lib/documents.py`), which
reads the working copy through the version control. `path` is relative to the project's root, `/`-separated.

- `documents?path=` answers `{path, kind, language, text, document, line_offset, lines, bytes, modified_at, tracked,
  commit, submodule}`. `kind` is `markdown`, `code` (with its fence `language`), `image`, `binary` or `text`; `document` is the
  CompMD document the dashboard renders: a Markdown file as it is, code and text in a fence (`line_offset` 1, the
  lines the wrapping adds before the file's first), an image as a Markdown image of its own name, a binary file as a
  line saying so. `text` is the file as it is (`null` for an image or a binary file), `lines` its line count.
  `tracked` and `commit` (the HEAD commit) are `null` outside git. A file in a git submodule (T-408) is judged by the
  submodule's own repository (`tracked`, and whether it is ignored), and `submodule` is `{path, commit, repository}`:
  the submodule's folder, its own HEAD and its origin's web address (`null` when it isn't one); `null` for any other
  file. The ETag is the file's size, mtime and the commit.
- `files?path=` answers an image's bytes (PNG, JPEG, GIF, WebP, SVG whose bytes match its name) with its type,
  `Cache-Control: no-cache`, `nosniff` and a sandboxing CSP; any other file `403`. The ETag is the size and mtime.
- Refused: a malformed path (`..`, absolute, empty parts, a backslash, over 512 characters) `400`; a hidden file or
  folder, a key file (`*.pem`, `id_rsa*`, …) or one git ignores `403`; a missing file, or one reached through a
  symlink, `404`; over 1 MB (5 MB for an image) `413`. Each with `{"error": …}`.

### 2.7 The dashboard's pages

The dashboard's views have addresses (T-429), and the server answers each with the same page, with the same
Content-Security-Policy, as `/`; the page reads its address and shows that view (`dashboard/src/lib/address.ts`,
SvelteKit routes in `dashboard/src/routes/`). The server sends nothing else under these paths: an ID or a tab in one is
text the page reads, never a file it looks up.

| Path | The page shows |
|---|---|
| `/` | the landing page: the overview with two or more projects, the project with one (whose address it then takes) |
| `/projects` | the overview's project cards; `/board`, `/progress`, `/agents` its other tabs |
| `/projects/<id>` | a project, on its saved tab; `/projects/<id>/<tab>` on a tab (`kanban`, `list`, `graph`, `progress`, `agents`, `project`) |
| `/w/<w>`, `/w/<w>/…` | any of the above in workspace `<w>` (§2.8, T-457): `/w/<w>` its landing page. Without `/w/<w>`, `default`'s; `/` itself opens the workspace opened last |

What is open over a page is in the query: `?task=T-1` a task's details, `?doc=docs/x.md:L40` (or `#heading`, encoded) a
document in the viewer, `?sprint=active` the Sprint filter, `?about` and `?help` the dialogs. The server ignores the
query. The paths are `PAGE_PATH` in `api/server.py` (a tab is lowercase letters; nothing deeper than a tab), any other
path gets `404`. An old link from before T-429 (`/#project=<id>&view=list&doc=…`) still opens: the page reads the
old parameters from its hash and moves to the new address.

### 2.8 `api/v2/workspaces`: the workspaces (ADR-003, T-456)

```json
{"generated_at": "2026-11-08T12:00:00Z", "workspaces": [{"id": "default", "projects": 3}, {"id": "work", "projects": 1}], "recent": "work"}
```

- `workspaces`: every workspace in the state folder (§5), `default` first and always there, with how many projects
  it lists; `recent`: the one opened last (`recent.json`), else `default`.
- **Opening a workspace** is reading its `projects` (`api/projects` for `default`): the server records it as the last
  opened, the one write a read makes (a choice of view, never task data; written only when it changes). The
  dashboard's `/` opens it (T-457), and a switcher next to the project selector lists the workspaces when there is
  more than one. The dashboard reads `default` from the v1 routes and any other workspace from its v2 ones.
- A workspace's routes answer as the v1 ones do, for its own projects: an unknown workspace gets `404` with
  `{"error": "unknown workspace"}`, an unknown project `{"error": "unknown project"}`. A project's ID is unique within
  a workspace; its logo is at `api/v2/workspaces/<w>/projects/<id>/logo` (in `default`, `p/<id>/logo`).
- The app's commands answer the same (without `generated_at`): `list_workspaces`, `list_projects`, `search`,
  `project_tasks`, `claims`, `worktrees`, `document`, `file` and `logo`, and `subscribe_workspace`,
  `subscribe_project` and `unsubscribe` for live changes (docs/guides/tauri.md).

## 3. `GET /api/projects`

```json
{
  "generated_at": "2026-10-25T21:51:38Z",
  "projects": [
    {
      "id": "1f3a9c2e",
      "name": "Agentic Task Manager",
      "path": "/Users/me/workplace/agentic-task-manager",
      "missing": false,
      "summary": {
        "total": 183,
        "done": 152,
        "skipped": 3,
        "percent": 84,
        "active_sprint": { "id": "One dashboard", "state": "active", "due": "2026-10-31", "release": "0.17.0", "done": 0, "total": 17, "percent": 0, "goal": "All projects on one page" },
        "recent_sprints": [
          { "id": "Themes", "state": "closed", "due": null, "release": null, "done": 10, "total": 10, "percent": 100 },
          { "id": "Agents", "state": "closed", "due": null, "release": null, "done": 9, "total": 9, "percent": 100 },
          { "id": "Archive", "state": "closed", "due": null, "release": null, "done": 12, "total": 12, "percent": 100 },
          { "id": "One dashboard", "state": "active", "due": "2026-10-31", "release": "0.17.0", "done": 0, "total": 17, "percent": 0 }
        ],
        "agents_working": 1,
        "working": [{ "task": "T-188", "name": "One title bar for every page", "agent": "claude:e5ea9db0", "branch": "dev/phase-17" }],
        "next": [{ "id": "T-190", "name": "Project card: a long phase name stays on one line" }],
        "last_done": { "id": "T-189", "name": "Status bar pinned to the bottom on All projects", "completed_at": "2026-10-26T05:53:00Z" },
        "git": { "branch": "dev/phase-17", "pr": { "number": 12, "url": "https://github.com/solo-swe/agentic-task-manager/pull/12", "state": "merged" } },
        "needs_you": { "review": 1, "blocked_on_user": 0 },
        "needs_you_tasks": [{ "id": "T-186", "name": "Update notice: shown when the page is already the new build", "reason": "review" }],
        "last_activity": "2026-10-25T21:51:38Z",
        "completions_14d": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 41, 28],
        "board": {
          "todo": { "count": 1, "cards": [{ "id": "T-190", "name": "Project card: a long phase name stays on one line", "type": "polish", "sprint": "One dashboard", "priority": "P2", "labels": ["overview"] }] },
          "active": { "count": 1, "cards": [{ "id": "T-188", "name": "One title bar for every page", "type": "feature", "sprint": "One dashboard", "priority": "P1", "labels": [], "by": "claude:e5ea9db0", "since": "2026-10-20T09:12:00Z" }] },
          "review": { "count": 1, "cards": [{ "id": "T-186", "name": "Update notice: shown when the page is already the new build", "type": "bugfix", "sprint": "One dashboard", "priority": "P2", "labels": [], "since": "2026-10-25T21:40:02Z" }] },
          "blocked": { "count": 0, "cards": [] },
          "done": { "count": 28, "cards": ["… 28 cards, each with completed_at"] }
        },
        "stats": { "started": "2026-08-12T09:30:00Z", "tasks": 236, "agent_minutes": 4440 }
      },
      "worktrees": [{ "path": "/Users/me/workplace/agentic-task-manager", "branch": "main", "main": true }],
      "logo": "p/1f3a9c2e/logo"
    }
  ]
}
```

The ETag changes when the registry, any listed project's `tasks.json`, claims, logo or `.git/HEAD`, or its worktrees
change (the server combines their size/mtime stamps and the worktree lists). Projects are listed in registry order. The dashboard sorts them.

### 3.1 Project fields

| Field | Meaning |
|---|---|
| `id` | the project's `meta.id` (8 hex characters, which `atm db init` adds). A registered project without one gets `p` plus the first 7 hex characters of the SHA-1 of its `path`, stable as long as the path is, until `meta.id` is set |
| `name` | `meta.project`, or the folder's name when it has none |
| `path` | the project root (the main worktree's folder) |
| `missing` | `true` when the folder or its `.agentic-task-manager/tasks.json` is gone (or unreadable); the project then has `"summary": null` |
| `summary` | §3.2, or `null` when `missing` |
| `worktrees` | its git worktrees, main first, as in §2.2 without `tasks`; empty when `missing` or outside git |
| `logo` | `p/<id>/logo` (relative to the dashboard) when it has a usable logo (§2.3), else `null`; `null` when `missing` |

### 3.2 Summary fields

Counted from `tasks.json` only (not the archive; `stats` excepted), by status **kind** (the project's vocabulary), with atm-cli's own
logic (`query.py`). Task names in `working`, `next`, `last_done` and `needs_you_tasks` are capped at 80 characters
(the 80th is "…"), so the response stays small with many projects.

| Field | Meaning |
|---|---|
| `total` | all tasks |
| `done` | done-kind tasks |
| `skipped` | skipped-kind tasks |
| `percent` | `round(100 × done / (total − skipped))`, an integer. When `total − skipped` is 0 (no tasks, or only skipped ones): `100` if there are skipped tasks, `0` if there are no tasks |
| `active_sprint` | the active sprint (SR-7.7): `{ id, state, due, release, done, total, percent, goal }`, its tasks counted as above (skipped ones left out of `total`); `null` when none is active |
| `recent_sprints` | up to 4 sprints with tasks for the overview's rings, in the project's order: the active and planned ones, then the most recently closed (by `end`) to fill up to 4. Each is `{ id, state, due, release, done, total, percent }`. `null` for a project without sprints (the card then shows one ring for all its tasks). Servers before 0.24 sent `current_phase` and `recent_phases` instead |
| `agents_working` | live claims (lease not expired) in the claims store |
| `working` | those claims, oldest first: `{ task, name, agent, branch }` (the task's name from this `tasks.json`; `branch` as the claim recorded it, `null` outside git) |
| `next` | up to 2 tasks `atm loop next` would hand out, in its order, by the same code (`loop_order` in `claims.py`): tasks back from review first, then to do; dependencies done, not claimed, not done or waiting in another worktree, not done together with the user; in the loop's default selection, the active sprint's or else every task; most urgent priority, lowest ID. `{ id, name }`. The project card lists them; the project's Kanban marks the first *Next up* (T-321), so the dashboard never applies the loop's rule itself. `atm project summary` prints this whole summary for the project it runs in |
| `last_done` | the done-kind task with the latest `completed_at`: `{ id, name, completed_at }`, or `null` |
| `description` | `meta.description` (an optional line about the project), trimmed and capped at 200 characters; `null` without one. The card shows it only as the name's tooltip |
| `git.branch` | the main worktree's checked-out branch (from `.git/HEAD`; a detached HEAD's short SHA), or `null` outside git |
| `git.pr` | the latest PR recorded on the tasks (`atm vcs pr`; the highest number): `{ number, url, state }`, `url` as recorded (or `null`), `state` `"merged"` once a task of it has `merged_at` (`atm vcs pushed`), else `"open"`; `null` without one. No forge is asked, and there is no CI state |
| `needs_you.review` | review-kind tasks, but those a reviewer approved or asked changes for (back with the agent) |
| `needs_you.blocked_on_user` | blocked-kind tasks with `waiting_on: "user"` |
| `needs_you.together` | to-do tasks with `with: "user"` the user can take up now (T-347): in the active sprint (in any sprint or the backlog without an active sprint), their dependencies done (an archived one counts as done) |
| `needs_you_tasks` | those same tasks, in file order: `{ id, name, reason }`, `reason` being `"review"`, `"blocked"` (on a question for the user) or `"together"`. The card's "needs you" chip opens them |
| `last_activity` | the newest of all tasks' `updated_at`, `started_at`, `completed_at`, `review_requested_at`, `blocked_at` and `skipped_at`, and of note times; `null` without any |
| `completions_14d` | 14 integers, oldest first, ending with today: done-kind tasks by the UTC day of `completed_at` (tasks without `completed_at` aren't counted) |
| `board` | the cards for the All projects board (§3.3) |
| `board_active_sprint` | the same columns, of the active sprint's tasks only, with their own counts: the board's default view (T-340, §3.3); `null` without an active sprint. Absent before 0.26, when the dashboard shows `board` |
| `stats` | the overview card's line (T-255), archived tasks included (a task in both files counted once, from `tasks.json`; an unreadable archive left out): `started`, the earliest `created_at` (else `meta.created`, else `null`); `tasks`, how many; `agent_minutes`, the agent time as the Project tab counts it: the work (`started_at` until the review request or completion, until now while active) of tasks an agent (`claude:…`) or nobody recorded started, each agent's spans merged, added up. Computed when the summary is (a change to `tasks.json` or the claims), so a running task's time grows between changes only then |

### 3.3 The board

`board` has one column per kind (docs/all-projects-kanban-proposal.md §3.1), each `{ count, cards }`: `count` is how
many tasks follow the column's rule, `cards` the first of them (so the dashboard can say *+ N more*).

| Column | Rule | Order | At most |
|---|---|---|---|
| `todo` | every to-do task in the board's scope (T-349; `board_active_sprint` holds the active sprint's tasks, no more, no less, its `done` every finished one, not the last 24 hours'); a card done with the user has `with_user: true`, one waiting on dependencies `waiting_for` (their IDs) | the ready ones the loop takes, in its order; then the ready ones done with the user; then those waiting on dependencies (each by priority, then ID) | 10 |
| `active` | every active-kind task | longest running first (`started_at`) | 30 |
| `review` | every review-kind task | longest waiting first | 30 |
| `blocked` | every blocked-kind task | blocked on the user first, then longest waiting | 30 |
| `done` | done-kind tasks with `completed_at` in the last 24 hours | newest first | 30 |

A card is what a project's Kanban card shows: `{ id, name, type, sprint, priority, labels }` (`sprint` `null` in the backlog; the name capped as
above, the priority the project's default when a task has none), plus `by` (the claim's agent, else `started_by`) on
`active`; `since` (`started_at`, `review_requested_at` or `blocked_at`, else `updated_at`, else `null`) on `active`,
`review` and `blocked`;
`waiting_on` and `questions` (how many) on `blocked`; `completed_at` on `done`.

`board_active_sprint` follows the same rules over the tasks in the active sprint only (T-340), so its counts and
*+ N more* are the active sprint's: the board shows it by default (*Active sprints*) and `board` with *All tasks*.

## 4. `GET /__atm/whoami`

```json
{ "atm": true, "pid": 41235, "port": 8765, "version": "0.15.0", "build": "3f9a1c0d", "home": "9c1e0f44a2b7d813" }
```

`home` identifies the atm folder (`ATM_HOME`) the server serves: the first 16 hex digits of the SHA-256 of its real
path, never the path itself (T-390). `atm service status` and `stop` look at the 20 ports from `--port` (8765 by
default) for servers of their own folder that `server.json` doesn't name, a server whose record was lost or
overwritten: `status` reports them (`unrecorded` in `--json`), `stop` stops them. A server of another folder, or one
too old to give `home`, is never touched. A record counts as the folder's only while the server on its port gives the
record's `pid` **and** this folder's `home` (T-501: the record moved from `~/.agentic-task-manager` names the server an
older atm still runs for that folder); any other record is removed. For the platform's state folder, `status` also
names an older atm's servers on those ports, those whose `home` is `~/.agentic-task-manager`'s (`older_atms` in
`--json`).

`version` and `build` are the `atm-dashboard-version` and `atm-dashboard-build` meta tags of the dashboard the server
holds, read from the very bytes it serves at `/`, so a page loaded from it always has the same `build`. atm compares
them with its own plugin's dashboard to decide whether to restart the server (`serve --refresh`): it restarts a server
with an older version, or with the same version but another build of the **same** plugin folder (`plugin_root` in the
record; the dashboard was rebuilt). Another install of the same version (the plugin cache, a development checkout, a
worktree) is left alone, or two installs would restart the server in turn and every open page would offer a reload.
The open dashboard compares `build` with its own to offer a reload.

The old per-project servers (0.10.0–0.14.0) answer `{atm, folder, pid}`. The `folder` key identifies one, which is how
`atm db migrate` recognises and stops it.

## 5. Files in atm's state folder

The state folder ([ADR-003](adr/ADR-003-workspaces.md) §2) is `ATM_STATE_DIR` when set, else `ATM_HOME` (its older
name), else the platform's app data folder for `com.solo-swe.atm`: `~/Library/Application Support/com.solo-swe.atm/`
(macOS), `$XDG_DATA_HOME/com.solo-swe.atm/` or `~/.local/share/com.solo-swe.atm/` (Linux),
`%APPDATA%\com.solo-swe.atm\` (Windows). Without either variable, the first run moves `~/.agentic-task-manager/` there
and leaves `MOVED.txt` in its place (T-490). Both files are written atomically (temporary file, then rename) under the
folder's `.lock`, atm's usual lock directory.

**An older atm beside it** (T-501): an atm from before 0.34.0 still uses `~/.agentic-task-manager/`, and makes its
`projects.json` again there. Whenever the `default` workspace is read, the projects that file lists, with a readable
task file, are added to `default` (never removed from it), when the file changed since it was last read:
`legacy-adopted.txt` in the state folder holds its size and time. Only the platform's state folder does this, never
one set with `ATM_STATE_DIR` or `ATM_HOME`; and it never waits for the folder's lock (the next read tries again).

### 5.1 `workspaces/default.json`, the registry

It was `projects.json` before T-490: an old one is renamed into place when atm first reads the folder. T-491 turns it
into the `default` workspace (ADR-003 §1).

```json
{
  "version": 1,
  "projects": [
    {
      "id": "1f3a9c2e",
      "name": "Agentic Task Manager",
      "path": "/Users/me/workplace/agentic-task-manager",
      "added_at": "2026-10-25T21:51:38Z",
      "seen_at": "2026-10-25T21:51:38Z"
    }
  ]
}
```

- Keyed by `path`, absolute, the main worktree's folder: one entry per path.
- `id` and `name` are copied from `meta.id` and `meta.project` when atm registers the project, and updated when they
  change. `id` follows §3.1 when `meta.id` is missing.
- `seen_at` is the last time atm ran there, updated at most once a day.
- An unreadable file is renamed to `default.json.damaged-<UTC time, e.g. 20261025T215138Z>` and replaced by an empty
  registry.

### 5.2 `server.json`, the running server

```json
{
  "pid": 41235,
  "port": 8765,
  "version": "0.15.0",
  "build": "3f9a1c0d",
  "plugin_root": "/Users/me/.claude/plugins/cache/solo-swe-plugins/atm/0.15.0",
  "home": "9c1e0f44a2b7d813",
  "started_at": "2026-10-25T21:51:38Z"
}
```

Written by the server once it listens, and removed when it stops. A record is trusted only if `whoami` on its port
answers with the same `pid`. Otherwise it is stale and is removed, as today.

T-390: `atm service` starts a server under `ATM_HOME/server.lock`, held across "is one running?", the start and its
record, so two at once (two sessions' hooks) start one server and the second reports it. `stop` sends SIGTERM, then
SIGKILL if the server outlives five seconds; the record is removed only once that server is gone, and only if it is
still that server's: a running server is never forgotten.
