# The multi-project server: routes and formats

**Status:** agreed format for phase 15 (T-165) · **Design:** [the proposal](multi-project-dashboard-proposal.md)

One `atm serve` per user serves the dashboard and every registered project. This page fixes what the server answers
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
| `GET /` | the dashboard (`text/html`), from the plugin, held in memory since the server started | no |
| `GET /api/projects` | the registered projects with their summaries (§3) | yes |
| `GET /api/search?q=<text>` | the tasks matching a search, in every project (§2.4) | no |
| `GET /p/<id>/tasks.json` | that project's `.agentic-task-manager/tasks.json`, as stored | yes |
| `GET /p/<id>/claims.json` | that project's live claims, from every worktree (§2.1) | yes |
| `GET /p/<id>/tasks-archive.json` | that project's archive, as stored | yes |
| `GET /p/<id>/worktrees.json` | that project's git worktrees and their changed tasks (§2.2) | yes |
| `GET /p/<id>/logo` | that project's logo image (§2.3) | yes |
| `GET /__atm/whoami` | who this server is (§4) | no |
| anything else | `404` | |

- `<id>` is a project id from `/api/projects` (§3.1). An unknown id gets `404` with `{"error": "unknown project"}`.
- Only those names are served for a project: `tasks.json` and `tasks-archive.json` from its
  `.agentic-task-manager/` folder, `claims.json` and `worktrees.json` computed, and `logo`. There is no other file, no directory
  listing, no path segments after the name. A file that doesn't exist (no archive yet, no claims yet) gets `404` with
  `{"error": "not found"}`. The dashboard treats a missing archive, claims or worktrees as empty.
- A missing project (§3.1 `missing: true`) answers `404` for its files.
- The ETag of a project file is its size and nanosecond mtime, as today's `json_etag` (`"<size hex>-<mtime_ns hex>"`).
- Query strings are ignored, except `q` on `api/search`.

### 2.1 `claims.json`: claims from every worktree

The claims of all a clone's worktrees are in one store, in the git common directory (`.git/atm/claims.json`; outside
git, `claims-store.json` in the task folder). The server answers from it, as `{generated_at, claims: [...]}` sorted by
task, so an agent at work in a linked worktree shows on the project's board. Its ETag is the store's size and mtime.
Without a store yet, the server falls back to the task folder's own snapshot (`claims.json`, which atm writes there).

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
      "tasks": [{ "id": "T-179", "status": "in_progress", "...": "the task as that worktree's tasks.json has it" }]
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
- The ETag changes when a worktree is added or removed, switches branch, its `tasks.json` changes, or the claims
  store does.
- The dashboard shows all of them on one board: each task from the worktree where it got furthest (by status kind:
  todo, blocked, active, review, then done or skipped; the main worktree's copy on a tie), with that worktree's branch
  on its card.

### 2.3 `logo`: the project's logo

- **Which file.** `meta.logo` in the project's `tasks.json`: a path relative to the project root, set with
  `atm logo set <path>` (and removed with `atm logo unset`). Without it, the first usable file of `logo.svg`,
  `logo.png`, `.github/logo.*`, `public/favicon.*`, `assets/logo.*` (each `.*` tried as `.svg`, `.png`, `.webp`,
  `.ico`). A `meta.logo` that is unusable means no logo: nothing is searched for then.
- **Usable** means an SVG, PNG, WebP or ICO file (by its extension), inside the project folder once symbolic links are
  followed, and at most 512 KB. `atm logo set` refuses anything else; the server answers `404` with
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
| `id` | the project's `meta.id` (8 hex characters, which `atm init` adds). A registered project without one gets `p` plus the first 7 hex characters of the SHA-1 of its `path`, stable as long as the path is, until `meta.id` is set |
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
| `next` | up to 2 tasks `atm next` would hand out, in its order, by the same code (`loop_order` in `claims.py`): tasks back from review first, then to do; dependencies done, not claimed, not done or waiting in another worktree, not done together with the user; in the loop's default selection, the active sprint's or else every task; most urgent priority, lowest ID. `{ id, name }`. The project card lists them; the project's Kanban marks the first *Next up* (T-321), so the dashboard never applies the loop's rule itself. `atm summary` prints this whole summary for the project it runs in |
| `last_done` | the done-kind task with the latest `completed_at`: `{ id, name, completed_at }`, or `null` |
| `description` | `meta.description` (an optional line about the project), trimmed and capped at 200 characters; `null` without one. The card shows it only as the name's tooltip |
| `git.branch` | the main worktree's checked-out branch (from `.git/HEAD`; a detached HEAD's short SHA), or `null` outside git |
| `git.pr` | the latest PR recorded on the tasks (`atm pr`; the highest number): `{ number, url, state }`, `url` as recorded (or `null`), `state` `"merged"` once a task of it has `merged_at` (`atm merged`), else `"open"`; `null` without one. No forge is asked, and there is no CI state |
| `needs_you.review` | review-kind tasks |
| `needs_you.blocked_on_user` | blocked-kind tasks with `waiting_on: "user"` |
| `needs_you_tasks` | those same tasks, in file order: `{ id, name, reason }`, `reason` being `"review"` or `"blocked"` (on a question for the user). The card's "needs you" chip opens them |
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
| `todo` | ready to start (the tasks `next` takes from: to do, dependencies done, not claimed) in the loop's default selection: the active sprint's, or the backlog's in a project without sprints | the loop's | 10 |
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
{ "atm": true, "pid": 41235, "port": 8765, "version": "0.15.0", "build": "3f9a1c0d" }
```

`version` and `build` are the `atm-dashboard-version` and `atm-dashboard-build` meta tags of the dashboard the server
holds, read from the very bytes it serves at `/`, so a page loaded from it always has the same `build`. atm compares
them with its own plugin's dashboard to decide whether to restart the server (`serve --refresh`): it restarts a server
with an older version, or with the same version but another build of the **same** plugin folder (`plugin_root` in the
record; the dashboard was rebuilt). Another install of the same version (the plugin cache, a development checkout, a
worktree) is left alone, or two installs would restart the server in turn and every open page would offer a reload.
The open dashboard compares `build` with its own to offer a reload.

The old per-project servers (0.10.0–0.14.0) answer `{atm, folder, pid}`. The `folder` key identifies one, which is how
`atm migrate` recognises and stops it.

## 5. Files in `ATM_HOME`

`ATM_HOME` is `~/.agentic-task-manager` unless the environment sets it. Both files are written atomically (temporary
file, then rename) under `ATM_HOME/.lock`, atm's usual lock directory.

### 5.1 `projects.json`, the registry

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
- An unreadable file is renamed to `projects.json.damaged-<UTC time, e.g. 20261025T215138Z>` and replaced by an empty
  registry.

### 5.2 `server.json`, the running server

```json
{
  "pid": 41235,
  "port": 8765,
  "version": "0.15.0",
  "build": "3f9a1c0d",
  "plugin_root": "/Users/me/.claude/plugins/cache/solo-swe-plugins/atm/0.15.0",
  "started_at": "2026-10-25T21:51:38Z"
}
```

Written by the server once it listens, and removed when it stops. A record is trusted only if `whoami` on its port
answers with the same `pid`. Otherwise it is stale and is removed, as today.
