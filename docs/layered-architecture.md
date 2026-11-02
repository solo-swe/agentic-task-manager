# Layered architecture for atm

**Status:** draft, for review · **Date:** 2026-11-02 · **Task:** T-360 (sprint *Layered Architecture*)

atm grew command by command: each `cmd_*` function parses its arguments, locks and reads `tasks.json`, applies the
rules, writes the file back and prints the result. That was the fastest way to build it, and it works. It also means
the business rules, the storage and the output are woven together in every module. Three things we want next need
them apart:

- **another storage model**, such as a cloud database with its own sync, instead of JSON files shared through git;
- **other users of atm's logic** (an MCP server, scripts, other tools) without going through the command line;
- **a dashboard without its own copy of the rules**, so a rule changes in one place.

This document defines the layers, what each one may depend on, where today's code goes, and the steps to get there.
It moves no code: the steps at the end become tasks once it is approved.

## 1. The layers

```
  ┌──────────────┐
  │  Dashboard   │  Svelte, in the browser
  └──────┬───────┘
         │ HTTP only
         ▼
  ┌──────────────┐        ┌──────────────┐        ┌──────────────────────┐
  │   REST API   │        │     CLI      │        │ other tools          │
  │ (atm serve)  │        │  `atm …`     │        │ (MCP server, scripts)│
  └──────┬───────┘        └──────┬───────┘        └──────────┬───────────┘
         └───────────────────────┼───────────────────────────┘
                                 ▼
                    ┌─────────────────────────┐
                    │         atm-lib         │  all business logic
                    └─────┬──────────────┬────┘
                          │              │
                          ▼              ▼
              ┌───────────────────┐  ┌──────────────────┐
              │    db interface   │  │  vcs interface   │
              ├───────────────────┤  ├──────────────────┤
              │ JSON-in-git store │  │ git (code only)  │
              │ Cloud DB (later)  │  │                  │
              └───────────────────┘  └──────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────────┐
  │ common: the JSON Schema of the data → the Python model and the generated TS  │
  │ types; vocabularies, status kinds, errors, pure helpers. Every layer uses it. │
  └──────────────────────────────────────────────────────────────────────────────┘
```

| Layer | Does | Never |
|---|---|---|
| **Dashboard** | Shows projects; fetches from the REST API; computes what is purely presentation (chart series, groupings, layout) | Reads files; holds business rules (what is ready, archivable, waiting on whom) |
| **REST API** | Maps HTTP requests to atm-lib calls and results to JSON; the HTTP concerns (Host check, ETags, sizes, caching) | Holds logic; touches storage |
| **CLI** | Maps arguments to atm-lib calls and results to text or `--json`; exit codes | Holds logic; touches storage |
| **atm-lib** | Every business action and query: start, done, review, claims and leases, next, sprints, releases, archive, checks, summaries | Knows how or where data is kept; prints; reads `argparse` objects |
| **db** | One interface for data access; its implementations know how and where data is kept, and how concurrent writers are kept apart | Business rules |
| **vcs** | One interface to the code's version control: commit these files, the current commit, is a commit in the history, the base branch, the remote's address | Business rules; storing tasks |
| **common** | The data model (from the schema), vocabularies and their defaults, status kinds, typed errors, pure helpers (time, IDs, names) | Depends on any other layer; does I/O |

**The dependency rule.** Dependencies run one way:

- the Dashboard depends on the REST API, over HTTP only;
- the REST API, the CLI and other tools depend on atm-lib;
- atm-lib depends on the db and vcs **interfaces**, never on an implementation; the implementation is chosen once,
  when atm starts (the CLI's and the REST API's entry points);
- the db and vcs implementations depend on their interfaces;
- every Python layer may use common; the Dashboard uses common's generated TypeScript types.

Nothing depends upward, and the CLI and REST API never call each other.

## 2. Git's two roles

atm runs `git` today for two different reasons, and the design keeps them apart.

**Git as the store for tasks** (a db implementation detail):

- tasks reach other people by commit, push and merge; atm's merge driver for `tasks.json` (`merge.py`) lets branches
  that changed different tasks merge cleanly;
- `migrate` and `init` stage atm's files (`git add`, `git mv`, `git rm --cached`);
- claims and the registry are shared by all worktrees of a clone through `git rev-parse --git-common-dir`
  (`claims.py:79`, `registry.py:60`);
- the project card's branch comes from `.git/HEAD` (`query.py:351`).

**Git as the code's version control** (the vcs interface):

- `atm commit`: commits exactly a task's files with `tasks.json`, and records the SHA on the task (`git_ops.py:172`);
- `atm commit --record`: checks a commit is in the history (`merge-base --is-ancestor`);
- `atm workflow`'s default base branch (`origin/HEAD`), `atm repository`'s remote address.

**The code host is already outside atm.** `atm pr` and `atm merged` only record what they are given; the skills call
`gh`. The dashboard already builds commit and PR links for GitHub, GitLab, Bitbucket and Gitea (`repo.ts:26`).
Supporting GitLab needs no change to these layers: it is a change in the skills.

**What the split buys:**

- **Another store for tasks.** Merge driver, commit-to-share and worktree-shared claims all move into the JSON-in-git
  store. A cloud store replaces them with its own sync and locking, and atm-lib doesn't change. This is the main gain,
  and it needs those three to leave `core.py`, `merge.py`, `claims.py` and `registry.py`.
- **Code in git, tasks elsewhere.** With a cloud store, `atm commit` still commits the code through vcs and records
  the SHA in the cloud store, but no longer commits `tasks.json`. atm-lib asks the store whether task data travels
  with code commits (`store.travels_with_code`), instead of assuming it.
- **Other version control** (Mercurial, Jujutsu, Sapling). The vcs interface is small (above). We define it and ship
  only git; another implementation is possible later, not planned.

Both git implementations (the store's and vcs's) run git the same way; they share a small git runner kept beside
them, not in common, since common does no I/O.

## 3. The db interface

atm-lib reads and writes through a **store**, opened for one project. Its first shape follows today's data, whole
documents in a transaction, so the move changes no behaviour:

```python
class Store(Protocol):
    def read(self) -> Snapshot: ...                     # tasks, meta, without a lock (status, list, show, serve)
    def transaction(self) -> ContextManager[Tx]: ...    # locked read-modify-write; commits on a clean exit
    def archive(self) -> Archive: ...                   # the archived tasks, loaded only when asked
    def claims(self) -> ClaimStore: ...                 # claims and leases, shared by the project's working copies
    travels_with_code: bool                             # True for JSON-in-git: atm commit includes the task data

class Tx(Protocol):
    data: TasksDocument                                 # the model from common
    def archive(self) -> ArchiveDocument: ...           # the archive inside the same transaction (archive/unarchive)
```

Beside the project store, a **registry** (the user's list of projects, today `~/.atm/`) is a second interface: the
REST API and `atm projects` use it.

- **JSON-in-git** is today's behaviour: the files in `.agentic-task-manager/`, `lock.py`'s lock, atomic writes, the
  merge driver, migrations of old layouts, claims in the clone's common git directory.
- **In-memory** is a second implementation for atm-lib's unit tests. It is also the proof that atm-lib depends on the
  interface only: a contract test suite runs against both.
- **Cloud** comes later. Whole-document transactions are coarse for a database shared live; once atm-lib only talks to
  the store, the interface can grow finer operations (one task, one claim) without touching the CLI or the API.

Open: whether `archived_task`'s lookup (today a module-level cache that `load_tasks` sets up, `model.py:501`) becomes
part of the snapshot or a store call. Either way the global goes.

## 4. common and the generated types

Today the model is written twice, by hand: in Python (`model.py`, `core.py`) and in TypeScript (`types.ts`,
`vocab.ts`). common makes one source.

**Source: a JSON Schema** of `tasks.json`, `tasks-archive.json`, the claims and the REST API's responses, in
`atm-cli/atm/common/schema/`. From it, at build time:

- **TypeScript types** for the Dashboard (`dashboard/src/lib/generated/`), with a dev-only generator such as
  `json-schema-to-typescript`;
- **Python types** (`TypedDict`s) and the **vocabulary defaults** (statuses and kinds, types, priorities) for
  common, generated by `atm-cli/build.py`, so `atm.py` stays standard-library only;
- **checks:** the Python validators (`check_*` in `model.py`) stay hand-written (the standard library has no schema
  validator); the test suite validates every fixture and every scenario's output against the schema, so code and
  schema can't drift.

CI fails when a generated file is stale, as it does for `atm.py` and the dashboard build. The schema is also the
contract for other users of atm-lib and for a future store.

The alternative, Python `TypedDict`s as the source with a small generator for TypeScript, has fewer files but puts
the contract in one of the two languages. We prefer the schema.

## 5. atm-lib as a library

atm-lib is usable by others, so it is an API with a version:

- functions take plain values and return results (dataclasses or dicts of the schema's types); they never take an
  `argparse.Namespace`, print or exit;
- failures are typed errors from common (`NotFound`, `Conflict`, `ClaimedByOther`, `LeaseExpired`, `Invalid`, …),
  which the CLI maps to its exit codes (today 1, 3 and 4) and the REST API to HTTP statuses;
- no module-level state; the store and the vcs are passed in (a `Project` object holds them);
- **delivery:** the plugin keeps the one-file `atm.py` (built from the same modules); others get a Python package
  built from the same sources. Its public surface is listed in one module (`atm.lib`), with semantic versioning;
  everything else is internal.

## 6. Today's code, by layer

atm-cli (7,670 lines in 21 modules). Every module with a `cmd_*` function is CLI and atm-lib at once (it parses,
loads, decides, saves and prints), and most also do storage. `cli.py` imports a `cmd_*` from 17 modules.

| Module | Lines | Goes to | Mixed today |
|---|---|---|---|
| `core.py` | 176 | common (time, JSON text, errors); git runner; CLI (`out`) | output, git and pure helpers together |
| `paths.py` | 73 | db (JSON-in-git) | `cmd_paths` |
| `lock.py` | 122 | db (JSON-in-git) | `cmd_lock_test` |
| `registry.py` | 347 | db (registry) | `cmd_projects`; git for the common dir |
| `model.py` | 799 | common (vocabularies, kinds, checks); atm-lib (`change_status`, notes, archivable) | `load_tasks`, `load_archive` read files; the `archived_task` cache |
| `components.py` | 240 | atm-lib | `cmd_components` |
| `sprint_names.py` | 99 | common | — |
| `repository.py` | 111 | common (URL parsing); vcs (remote address) | `cmd_repository` |
| `logo.py` | 96 | atm-lib (project info); db (reading the file) | `cmd_logo` |
| `claims.py` | 630 | atm-lib (claims, leases, next); db (the claims store) | `cmd_next`, `cmd_run`, `cmd_heartbeat`, `cmd_claims` |
| `query.py` | 813 | atm-lib (status, list, show, deps, summary) | five `cmd_*`; reads `.git/HEAD` |
| `tasks.py` | 191 | atm-lib (status changes, notes) | locks and writes the store itself |
| `edit.py` | 494 | atm-lib (add, edit, check) | three `cmd_*` |
| `sprints.py` | 478 | atm-lib | three `cmd_*` |
| `releases.py` | 280 | atm-lib | `cmd_releases` |
| `merge.py` | 428 | db (merge driver, merge config); atm-lib (`init`) | `cmd_init`, `cmd_merge_driver` |
| `archive.py` | 183 | atm-lib; db (the archive file) | two `cmd_*` |
| `git_ops.py` | 330 | vcs; atm-lib (commit, pr, merged, workflow) | four `cmd_*` |
| `migrate.py` | 443 | db (old layouts); REST API (the dashboard template) | `cmd_migrate` |
| `server.py` | 710 | REST API | serves the task files themselves, not atm-lib results |
| `cli.py` | 622 | CLI | — (already only parsing and dispatch) |

**The REST API today is a file server.** `atm serve` sends `tasks.json`, `tasks-archive.json` and `claims.json` as
they are on disk (`server.py:600`), plus a few computed answers (`/api/projects`, `/api/search`, worktrees, claims).
The Dashboard then applies the rules itself: which tasks are ready or waiting (`waiting.ts`, `work.ts`), status kinds
and defaults (`vocab.ts`), effective review modes and releases (`review.ts`, `releases.ts`), the archive's
threshold notice. Those are atm-lib's rules computed a second time, and they are what moves.

**The Dashboard's requests** are spread over `load.ts`, `state.svelte.ts`, `claims.ts`, `worktrees.ts` and
`overview-data.ts`: they become one client module (`dashboard/src/lib/api/`), the only one that calls `fetch`.

**The dev server plays the API a second time** (`dashboard/build/dev-data.ts`, 152 lines), already calling
`atm summary` for part of it. In the target it runs the real REST API, so there is one implementation.

## 7. Layout and how the rule is checked

```
atm-cli/atm/
  common/        model (generated types + rules), vocabularies, errors, helpers; schema/
  db/            interface.py; jsongit/ (files, lock, merge driver, migrations, claims store); memory/
  vcs/           interface.py; git/
  lib/           atm-lib: tasks, claims, query, edit, sprints, releases, archive, components, project, commit
  cli/           argparse, renderers (text, --json), exit codes
  api/           the REST API: routes, HTTP handling
```

- `atm-cli/build.py` already refuses an import from a module later in `MODULES`. It gets layers: each module's layer
  is its folder, and an import into a layer the rule doesn't allow fails the build (`api` → `lib` yes, `lib` →
  `db/jsongit` no, `lib` → `cli` no). The output is still one file, `plugin/scripts/atm.py`, and top-level names
  stay unique.
- The Dashboard gets the same as an ESLint rule (`no-restricted-imports` / `no-restricted-globals`): only
  `src/lib/api/` may call `fetch`; generated types are imported, never edited.

## 8. Constraints that stay

- atm-cli is standard-library Python 3.9 and ships as one file; the plugin needs nothing installed but Python.
- The Dashboard is one self-contained HTML file, served by `atm serve`.
- The security invariants in `docs/security-architecture.md` hold at every step. The REST API stays read-only,
  localhost-only, with its Host check; **a route that writes is a new entry point** and is out of scope here (when it
  comes, it needs its own design: CSRF protection, a token, the security document updated in the same PR).
- Users see no change: same commands, output, exit codes and files. `tests/scripts/` (530 tests through the built
  launcher) and the e2e suite pass at every step and are the safety net for the moves.

## 9. Migration plan

Each step leaves every test green and ships nothing users notice; they can be spread over several sprints. Proposed
as tasks:

1. **Layer folders and the import rule.** Create the folders, teach `build.py` layers and the rule, move modules
   unchanged where they already fit (`cli.py`, `sprint_names.py`), and list the current violations as allowed
   exceptions that later steps remove. A test fails on a new violation.
2. **common.** Move the pure parts of `core.py` and `model.py` there; typed errors; drop the `archived_task` global.
3. **The db interface and the JSON-in-git store.** Move `paths`, `lock`, loading and saving, the archive file, the
   claims store, `registry`, the merge driver and `migrate`'s old layouts behind `Store`; add the in-memory store and
   the contract tests. atm-lib modules open a store instead of files.
4. **The vcs interface and git.** Move commit, ancestry, base branch and remote address out of `git_ops.py`,
   `core.py` and `repository.py`; `travels_with_code` decides whether `atm commit` includes `tasks.json`.
5. **atm-lib, one area per task.** Split each `cmd_*` into an atm-lib function (plain values in, result out, typed
   errors) and a CLI renderer: tasks and status changes; claims and next; query; edit and check; sprints; releases;
   archive; components; init, commit, pr and merged. Each comes with Python unit tests on the in-memory store (new:
   `tests/lib/`, standard `unittest`).
6. **The REST API on atm-lib.** `server.py`'s handlers call atm-lib; versioned routes (`/api/v1/…`) return the model
   plus derived values (readiness, waiting on, effective review and release, archive hints). The raw file routes stay
   until the Dashboard no longer needs them. `docs/multi-project-api.md` is updated.
7. **The schema and the generators.** The JSON Schema in common; the TypeScript and Python generators in the build;
   fixtures and outputs validated against it; CI's staleness check.
8. **The Dashboard's client layer.** All requests in `src/lib/api/`, typed by the generated types; the ESLint rule.
9. **The Dashboard without rules.** Replace each duplicated rule with the API's derived values, one area per task
   (readiness and waiting, kinds and defaults, review and release, archive hints); then drop the raw file routes.
10. **One API implementation in development.** The dev server runs the real REST API instead of `dev-data.ts`.
11. **atm-lib for others.** The public module and its documentation, a package built from the same sources, the
    versioning policy.
12. **Documentation.** `docs/v4/atm-cli.md`, `CLAUDE.md`'s layout, `docs/security-architecture.md`'s components.

Steps 1–2 come first; 3 and 4 can run in parallel; 5 needs 3 and 4; 6 needs 5; 7 can start after 2; 8 after 7;
9 after 6 and 8; 10 after 6; 11 after 5; 12 at the end.

## 10. Open questions

1. **The store's grain:** whole documents in a transaction first (proposed), finer operations when a cloud store
   comes; or design the finer operations now?
2. **API versioning:** `/api/v1/` beside today's routes (proposed), or change the routes in place since the
   Dashboard is served with the API and always matches it?
3. **The package for others:** published (PyPI) or installed from the repository; its name.
4. **Presentation vs rules in the Dashboard:** forecasts, velocity and burndown are computations on history. Proposed:
   they stay in the Dashboard as presentation, and only rules that decide what atm *does* (ready, waiting, archivable,
   effective modes) move behind the API.
