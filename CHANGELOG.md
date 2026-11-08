# Changelog

All notable changes to atm and its task dashboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).
The dashboard comes with atm: upgrading atm updates it everywhere (until 0.32.0, with the Claude Code plugin).

## [Unreleased]

### Changed
- **atm's state moves to the platform's app data folder** (T-490, [ADR-003](docs/adr/ADR-003-workspaces.md) §2):
  `~/Library/Application Support/com.solo-swe.atm/` on macOS, `~/.local/share/com.solo-swe.atm/` (`$XDG_DATA_HOME`)
  on Linux, `%APPDATA%\com.solo-swe.atm\` on Windows, the folder the app's Tauri APIs name too; `ATM_STATE_DIR` puts
  it elsewhere, and `ATM_HOME` keeps working as its older name. The first run without either moves
  `~/.agentic-task-manager/` there (one rename; `MOVED.txt` is left saying where), and the registry, `projects.json`,
  becomes `workspaces/default.json`, renamed in place when found. The `dirs` crate (approved, already in the tree)
  finds the folder; a test checks the app's identifier is atm-common's `APP_ID`. The tests, the e2e fixture and the
  dev server use `ATM_STATE_DIR`, and the suite drops an inherited one.
- **Projects open through atm-lib, on store and version-control factories** (T-491, ADR-003 §1 and §4): a project's
  `.agentic-task-manager/settings.json` (`store`, `vcs`; `json-git` and `git` without one) is read before any store
  is chosen, and a store or vcs atm doesn't know is refused. `atm_db::open` and `atm_vcs::open` build them;
  `Project::open` and `open_tolerant` use them, and `project.store_maintenance()` has what only the JSON-in-git store
  has (its paths, `db init`'s merge driver, migrations, the lock test hook, the merge driver itself). atm-lib's
  `Workspaces` and `Workspace` (over atm-db's `workspaces/<id>.json` and `recent.json`) list a workspace's projects
  with their summaries, worktrees and logos, search them, and add and remove them; atm-api's project list and search
  call them, the summary cache moved with them. Nothing users see changes.
- **The atm-boot crate is removed** (T-492, ADR-003 §4): the CLI, the REST API and the app depend on atm-lib and
  atm-common only. The `atm db` commands, `atm project config` and `atm _lock-test` go through
  `project.store_maintenance()`; registering is `project.join_default_workspace()` (the hook's,
  `join_default_workspace_if_needed()`); the actor and `default_base` are atm-lib's (`actor`, `code`); the pid and
  signal helpers are the CLI's (`process.rs`); `hook_project` is gone. `tests/scripts/layers.test.mjs` checks each
  crate's `Cargo.toml` against the layer rule. `docs/layered-architecture.md`, the Rust guide, porting-to-rust.md and
  AGENTS.md describe the layers without boot. Nothing users see changes.

### Added
- **The dashboard's IPC transport** (T-457): in the atm app the dashboard reads through the app's commands, not HTTP.
  `browserFetch` picks it at start (`isTauri()`); `lib/api/ipc.ts`'s `ipcFetch` maps the dashboard's URLs (v1 as
  `default`'s, v2) to `list_workspaces`, `list_projects`, `search`, `project_tasks`, `claims`, `worktrees`, `document`,
  `file` and `logo`, errors to their HTTP statuses; logos come as `data:` URLs (`lib/api/images.ts`). One window,
  switching workspaces in place. New dependency: `@tauri-apps/api` 2.12.4 (approved). Tests: `tests/unit/ipc.test.ts`
  (on `@tauri-apps/api/mocks`) and `tests/e2e/app.spec.ts` (the page as in Tauri's window, the fixture answering the
  commands, no HTTP request made).
- **`atm workspace`** (T-493, ADR-003 §3): `list` (each with its project count; the last opened marked), `create`,
  `delete` and `rename` (`default` can't be deleted or renamed, and no workspace can take its name), `projects [<w>]`,
  `add-project [<w>] [<folder>]`, `remove-project [<w>] <project>` and `prune [<w>]`, each defaulting to `default`.
  A project can be in several workspaces. **`atm project list|add|remove|prune` are removed**: use
  `atm workspace projects|add-project|remove-project|prune`. The `atm-serve` skill names the new commands, and its
  Claude Code pre-approvals narrow to the two reads (`atm workspace list`, `atm workspace projects`); `atm docs
  commands` lists them. The dev server adds its projects with `atm workspace add-project`.
- **Workspaces in the dashboard** (T-457, ADR-003 §5): addresses `/w/<w>/…` (`/w/<w>/projects/<id>/<tab>`,
  `/w/<w>/board`, …; without `/w/<w>`, `default`'s, so every link so far still opens), and `/` opens the workspace
  opened last. A workspace switcher sits next to the project selector when there is more than one. The dashboard reads
  `default` from the v1 routes and another workspace from its v2 ones (`lib/api/workspace.ts`, the URL builders in
  `lib/api/http.ts`); `/api/projects` now records `default` as the last opened, as `…/default/projects` does. atm
  serve's pages (`pages.rs`), the e2e server and `PAGE_PATH` accept the `/w/<w>` prefix.
- **Workspaces in the REST API, and the app's commands over them** (T-456, ADR-003 §5): `GET /api/v2/workspaces`
  (the workspaces, how many projects each lists, the last opened) and, per workspace, `…/<w>/projects` (which records
  it as the last opened in `recent.json`, only when that changes), `…/<w>/search` and
  `…/<w>/projects/<p>/tasks|documents|files|claims|worktrees|logo`. The v1 routes and `/p/<p>/…` are the `default`
  workspace's, unchanged. The app's commands mirror them (`list_workspaces`, `list_projects`, `search`,
  `project_tasks`, `claims`, `worktrees`, `document`, `file`, `logo`), through the same atm-lib reads (`Workspace`,
  `Member`), and a test checks each command answers what its route does. Live changes: `subscribe_workspace` and
  `subscribe_project` send on a channel when the folders the store reports change (`Store::watch_folders`; notify
  8.2.1 and notify-debouncer-full 0.7.0, approved, CC0-1.0 allowed in `deny.toml`), `unsubscribe` ends one. The main
  window's capability lists exactly these commands; the commands' JSON answers are typed `Json` for TypeScript.
- **The atm app's skeleton** (T-455, `app/src-tauri/`, the Tauri guide): a Tauri 2.12 app in the workspace, one
  instance (opening it again focuses its window), the user's registry of projects in its state, two commands
  (`version`, `list_projects`) mapping to atm, their TypeScript types generated with tauri-specta
  (`.generated/app/bindings.ts`), errors as one tagged enum, and a capability that lets only the main window's own
  pages call them. The window shows the built dashboard under the dashboard server's policy, with `connect-src` for
  IPC only; the dashboard talks to the app from T-457. `cargo build`/`test` build the app too: CI's Linux job installs
  WebKitGTK, and the e2e binary and the release build only the CLI (`-p atm-cli`). New dependencies (approved):
  `tauri`, `tauri-build`, `tauri-plugin-single-instance`, `tauri-specta`, `specta`, `specta-typescript`, and
  `@tauri-apps/cli` (the icons, from `branding/atm-logo.png`). `deny.toml` allows BSD-2-Clause and MPL-2.0 and
  ignores five "unmaintained" advisories of the app's tree (`paste`, `proc-macro-error`, `zbus_names`, `zvariant`,
  `zvariant_derive`; no vulnerability), each with its reason (approved).

### Fixed
- **A sprint whose name holds a comma can be chosen** (T-494): the Sprint filter in the address was a comma-separated
  list (`?sprint=a,b`), so "Library, gallery and blog" read back as two unknown sprints and the filter fell back to
  All tasks. It is now one `sprint=` per value (`?sprint=Foundations&sprint=backlog`), as HTML forms send a list;
  older links with commas no longer select several sprints. The scope menu also split its choice's key on commas;
  each choice is one value now (`ScopeBar.svelte`).
- **No more 404 for `/_app/version.json` in the browser's console** (T-495): SvelteKit checks it for a new deployment
  when the tab regains focus; atm serve now answers it with the page's own version (`{"version": …}`), so it never
  sees one (atm's reload offer stays whoami's).
- **The local screenshots run again** (T-485): `pnpm run test:e2e:visual` and `test:e2e:visual:update` ran the
  macOS `target/debug/atm` inside the Linux Playwright container since T-478, and every test failed. They now first
  run `pnpm run build:atm-linux`, which builds the CLI in the official `rust:1.99.0-bookworm` image (pinned by digest,
  approved) into `target/linux/`, and the fixture takes it from `ATM_E2E_BIN`.
- **A release whose tasks merge in several PRs is released** (T-486): `atm vcs pushed --pr N` releases a planned
  release once every one of its tasks is merged, in that PR or an earlier one, with `released_at` its last merge;
  before, only the tasks of PR N counted, so a sprint that reached `main` in two PRs (0.32.0: #39, then #40) stayed
  planned. Its message says which tasks are "not merged yet". `atm doctor` warns about a planned release whose tasks
  are all merged and names the `atm vcs pushed` that releases it. `/atm-push` says so too.

## [0.33.0] - 2026-11-08

The Rust atm, without the plugin: the first release that is only the `atm` binary (`brew install solo-swe/tap/atm`, or
a release's archives), with the dashboard and the skills embedded. 0.32.0 was the plugin's last release; plugin users
move as its entry and the README's "Moving from the plugin" say (T-484, the release).

### Removed
- **The Claude Code plugin and the Python atm** (T-454): `plugin/` (the `/atm:*` skills' generated copies, the
  launcher and `atm.py`, its hooks, `plugin/CLAUDE.md`), `.claude-plugin/marketplace.json`, `atm-cli/` (its modules,
  `build.py`, `uv.lock`) and the dashboard's `build/python-copy.ts` and `pnpm run build:cli`. atm is the Rust binary
  only, installed with `brew install solo-swe/tap/atm` or from a release's archives; its skills come with it
  (`atm agents install`). The pre-commit hook is gone too (nothing generated is committed any more), with the
  `prepare` script's `core.hooksPath`: run `git config --unset core.hooksPath` in an existing clone.

### Changed
- **The skills are `/atm-…` everywhere** (T-454): atm's hints and messages (`Run /atm-create-tasks first`, `plan one
  with /atm-plan-sprint`, `/atm-close-sprint`, `/atm-push`, `/atm-serve`), the dashboard's hints and Help, and the
  skills' own links; the skills no longer say that exit code 127 means Python is missing. `atm version`'s help says
  "atm's version".
- **The checks follow the code** (T-454): `security-invariants.test.ts` scans the Rust sources (I5: no shell);
  `security-headers.test.ts` reads atm-common's `page_csp`; `skills.test.ts` reads `skills/` and
  `skills/claude-code.json`; `check-versions.mjs` checks only `CHANGELOG.md` and `WHATS-NEW.md` against `Cargo.toml`;
  the ruleset (`.github/rulesets/main.json`) requires the three Rust jobs instead of the Python ones; the agent
  sandbox runs a Linux build of atm (`ATM_SANDBOX_BIN`) with its skills installed by `atm agents install`.
- **The README, `AGENTS.md`, `SECURITY.md` and the security architecture** describe atm as the binary: installing it,
  its skills, and moving from the plugin.
- **What Rust and the dashboard build from is out of `plugin/` and `atm-cli/`** (T-477), so the switch (T-454) can
  delete them: the JSON Schemas and `vocabularies.json` are in `crates/atm-common/schema/` (read by atm-common,
  `atm-cli/build.py` and the dashboard); the dashboard's TypeScript is generated by `dashboard/build/generate.ts`
  (`pnpm run generate`, no Python), not `build.py`; this changelog and `WHATS-NEW.md` are at the repository's root;
  and atm's version is `Cargo.toml`'s `[workspace.package]` (with the description, author, licence and repository the
  About dialog shows), which `plugin.json` and `marketplace.json` must match until the switch.
- **The dashboard and CompMD's runtime are no longer committed** (T-477): `pnpm run build` in `dashboard/` writes
  them into `dashboard/dist/` (`tasks-dashboard.html`, `assets/compmd/<version>/`), which the Rust atm embeds
  (`cargo build` explains what to run when they're missing), and copies them into `plugin/templates/` and
  `plugin/assets/` (gitignored) for the Python atm. CI's Rust and Python jobs and `release.yml` build the dashboard
  first; the staleness checks of the committed copies (CI's, `check-compmd-assets.mjs`, the pre-commit hook's
  rebuild of the page) are gone.
- **`pnpm run dev`'s REST API is the Rust atm** (T-478): `atm _serve` from `target/debug/` (`cargo build` first),
  restarted when the binary is rebuilt; `dev-api.test.mjs` runs it from a copy of the binary.
- **The suite runs only against the binary** (T-478, lands with T-454): no `ATM_IMPL`, `--rust` or
  `rust-expected-failures.json` (the last expected failure, the merge driver's command, now expects the binary's
  path); `expectRust` became `expect`; the Python-only tests went (`build`, `vendor`, `lib` with `tests/lib/`,
  `test_architecture`, `bin`, and the two tests of the plugin folder in `serve.test.mjs`); `run.mjs` always runs each
  file in its own process with a watchdog, and prints a failing file's TAP even with `--quiet`. The e2e fixture runs
  the binary (`pnpm run test:e2e` builds it; CI's e2e job takes one built on Ubuntu 22.04). CI's Python jobs are gone,
  and the benchmark runs against the binary. The suite on Windows is T-480's (40 failures on CI's first run there).

### Fixed
- **atm on Windows** (T-480): paths in atm's output and files use `/` and the real, long form of each folder (never an
  8.3 short name), so a file under the project is never "outside the repository" (atm-common's `pathtext`: `Shown`,
  `real`, `canonical`); `HOME` is the home folder when set, as on Unix; text files are LF in every checkout (a root
  `.gitattributes`); and `atm service` and `atm claim hold` start their background process without the caller's
  pipes (`windows-sys`, the one `unsafe` in atm, approved), so the shell that ran them doesn't wait for it. The lock
  retries a folder Windows still has pending delete. On CI's Windows runner the suite went from 40 failures to a
  handful; it stays off there until T-482 (the `claim hold` watcher, serve, the dev server) and T-481 (stress).

### Added
- **The public skills repo, `solo-swe/skills`** (T-453, [ADR-002](docs/adr/ADR-002-atm-public-skills-repo.md)):
  publishing a release runs `.github/workflows/skills.yml`, which puts the release's `skills/atm-*` (the portable core)
  and a README section into `solo-swe/skills`, the repo our apps share, and tags it `atm-v<version>`, so
  `npx skills add solo-swe/skills` finds them. `packaging/skills-export.sh` changes only atm's part; the workflow
  needs `SKILLS_DEPLOY_KEY` and, without it, says so and does nothing.

## [0.32.0] - 2026-11-06

The sprint *Rust and distribution*: the groundwork for atm as an app that installs its own skills
(`docs/better-way-to-distribute-atm.md`) and for the Rust port (`docs/porting-to-rust.md`).

**The plugin's last release** (T-479). 0.33.0 is the Rust `atm` CLI on its own (`brew install solo-swe/tap/atm`, or
the release archives for macOS, Linux and Windows), with the dashboard and the skills embedded; the Claude Code plugin,
its marketplace entry and the Python atm are retired. To move: install the CLI, run `atm agents install` (the skills
as `atm-do-task`, `atm-loop-tasks`, … in `~/.claude/skills/`, no longer `/atm:…`), stop the plugin's server
(`atm service stop`), then `/plugin uninstall atm@solo-swe-plugins` and `/plugin marketplace remove solo-swe-plugins`.
Task files, commands and options are unchanged. The README's "Moving to the atm CLI" has the steps.

### Changed
- **Claims last without the plugin's hooks** (T-439): every atm command an agent runs renews its live claims once
  their last renewal is ten minutes old (as `claim heartbeat --auto` does; a command with `--agent` renews nobody's),
  and the default lease is two hours instead of 30 minutes (`meta.lease_minutes` still sets a project's own). A task
  back from review stays its agent's for that long too.

### Added
- **`atm docs [<topic>]`** (T-440): reference for agents that matches the installed atm: `rules`, `commands`,
  `states`, `claims`, `tasks`. Written in `docs/agent/*.md`, built into `atm.py` (`atm-cli/atm/cli/agent_docs.py`,
  generated by `build.py`); `atm docs` lists the topics, `--json` gives them as data, an unknown topic exits 2.
  The skills will point to it instead of repeating it (T-441).
- **`atm agents install [--harness claude] [--project] [--copy]`, `atm agents status [--project]` and
  `atm agents uninstall [--harness …] [--project]`** (T-442): atm's skills, this version's, where Claude Code reads
  them. Written to `<ATM_HOME>/skills/claude/` (Claude Code's copy, with its `allowed-tools`) and linked from
  `~/.claude/skills/atm-*`; `--copy` copies instead, and `--project` copies into the project's `.claude/skills/` to
  commit. Every path written is printed; a folder atm didn't write is left alone (exit 1); `<ATM_HOME>/agents.json`
  records what was written, so `uninstall` removes exactly that. `status` says, per skill, linked, copied, missing,
  stale or not atm's. Built into `atm.py` (`common/skills.py`, generated from `skills/`); harnesses are a table in
  `lib/agents.py`, so others (T-413) are an entry each. Opt-in only: nothing installs on its own.

### Development
- **Release machinery for the atm CLI** (T-452, `packaging/README.md`): a `v*` tag runs
  `.github/workflows/release.yml`, which builds atm for macOS (arm64, x86_64; signed with the Developer ID and
  notarized), Linux (x86_64, arm64) and Windows and drafts a GitHub release with the archives and `SHA256SUMS`;
  publishing the draft runs `.github/workflows/homebrew.yml`, which writes `Formula/atm.rb` to `solo-swe/homebrew-tap`
  (`brew install solo-swe/tap/atm`). `packaging/release.sh` checks the version and tags; `sign-macos.sh` and
  `homebrew-formula.sh` are the workflows' steps. Only the job that drafts the release can write (I18).
- **Generated files live in `.generated/`, never committed**: `atm-cli/build.py` writes the data model's types, the
  vocabularies, `atm docs`' topics and the skills as Python modules into `.generated/atm-cli/atm/` (the build reads
  them from there, beside `atm-cli/atm/`), and the dashboard's types and vocabularies into `.generated/dashboard/`
  (imported as `$generated/…`). `build.py --generated` writes only that; `pnpm install`, the dashboard's scripts
  (`pnpm run generate`) and `tests/scripts/run.mjs` run it first. What the plugin ships stays committed.
- **The command tree is one module per top-level command**: `atm-cli/atm/cli/commands/<group>.py` (task, loop, claim,
  vcs, db, release, component, project, sprint, agents; `maintenance.py` for doctor and service, `meta.py` for
  version, help and docs), each adding itself to the root in `cli/app.py`, with the small rendering helpers only it
  used folded in; `cli/main.py` keeps shell completion and the entry. No command, option or output changes.
- **The Rust port's skeleton** (T-443, `docs/porting-to-rust.md`): a Cargo workspace (`Cargo.toml`,
  `rust-toolchain.toml` at 1.99.0, `deny.toml`) with a crate per layer under `crates/`, and `atm version` from the
  binary. CI's new `rust` job builds, lints and tests it on Linux, macOS and Windows, checks its dependencies with
  cargo-deny, and runs the CLI suite against the binary (`node tests/scripts/run.mjs --rust`; `pnpm run
  test:scripts:rust`) with the tests it doesn't pass yet listed in `tests/scripts/rust-expected-failures.json`.
- **The Rust atm reads a project** (T-444): the model and the vocabularies' checks (`atm-common`), reading
  `tasks.json` and the archive strictly with the task folder's symlink checks (`atm-db`, behind a store interface,
  with an in-memory store for tests), `task details` as text and `--json` (`atm-lib`, `atm-cli`), and the CLI's
  layout: `main.rs` with the global options (`-j/--json`, `-n/--dry-run`, a leading `--dir`), the errors and exit
  codes, and one module per command group in `commands/`. New dependencies: `serde_json` (keys in stored order) and
  `thiserror`. Nine scenarios left the expected-failures list.
- **`atm agents` and `atm docs` in the Rust atm** (T-450): `install|status|uninstall` for Claude Code as in Python
  (the harness table, the record in `<ATM_HOME>/agents.json`, links or copies, folders atm didn't write left alone),
  and `atm docs [topic]`. Build scripts embed `skills/` (`atm-common`) and `docs/agent/` (`atm-cli`) in the binary,
  checking each topic's first line as `build.py` does. Ten more tests left the expected-failures list.
- **`atm task list` in the Rust atm** (T-462): every filter (`--status`, `--sprint`, `--backlog`, `--release`,
  `--component`, `--type`, `--priority`, `--label`, `--agent`, `--ready`, `--blocked`, `--in-pr`, `--with-user`,
  `--archived`) and `--derived`, with what atm derives for a task (kind, readiness, `waiting_for`, effective priority,
  review and release, stage), text and `--json` as in Python. `--tolerant` comes with `atm serve` (T-449). Fourteen
  more tests left the expected-failures list.
- **The Rust atm's write path** (T-445): the task folder's lock as the Python atm's lock folder, the same protocol
  (`owner.json`, broken when its owner is gone or after a minute, released on SIGTERM), so both can work on one
  project at once; the claims store (in the git common folder, shared by worktrees) and its dashboard snapshot; and
  the change transaction, which writes only what changed, atomically. `atm _lock-test`, and `lock.test.mjs` passes.
  New dependencies: `sysinfo` (whether a lock's owner still runs, the host name) and `signal-hook` (Unix).
  `docs/porting-to-rust.md` §3 now says the lock is the lock folder, not `File::lock`.
- **`atm task deps` in the Rust atm** (T-463): what a task waits for and what waits for it, archived tasks included
  and marked, text and `--json` as in Python. Seven more tests left the expected-failures list.
- **`atm project status` in the Rust atm** (T-464): the overview with its scope options, text and `--json` as in
  Python: the active sprint, counts by status, sprint, type and label, what is in progress, in review, blocked (here
  or in another worktree) or waiting, the next task by the loop's rule with why there is none, the largest tasks,
  the archive and its hint, open tasks in closed sprints. The loop's selection and order (`atm-lib` `loop_rules.rs`)
  come with it, for `loop next` (T-467). Sixteen more tests left the expected-failures list.
- **`atm task state` in the Rust atm** (T-465): the task states as an enum of commands with their table of sources
  and destinations (checked against `docs/task-states.md` for every kind and command), the guards (dependencies,
  review modes and decisions, resolutions, `--status` of the right kind), the notes, timestamps and intervals each
  transition writes in Python's key order, claims taken on start and dropped on finish, expired leases (an agent's own
  explained, exit 4), the records shared by worktrees (completed, waiting, resolved), the archive hint once a day, and
  the actor (`--agent`, `ATM_AGENT`, the Claude Code session, git's user name). Seventy-nine more tests left the
  expected-failures list.
- **`atm task add` and `atm task note` in the Rust atm** (T-447): one task from options or many from `--json-file` (a
  list or `{"tasks": [...]}`), placeholder IDs replaced by the next IDs in the project's pattern (never behind the
  highest, `meta.next_id` advanced) and followed in dependencies and `related`; the whole file refused, nothing
  written, for an unknown type, priority or sprint, a closed sprint, a bad label, review or `with`, a missing
  dependency (an archived one is fine) or a cycle; `created_by`, timestamps, the sprint's history, `--components
  auto` and the components a task's files suggest (`atm-common` `components.rs`, globs matched as Python's
  `fnmatchcase`). A note on a task in any status, at most 500 characters. Twenty-three more tests left the
  expected-failures list.
- **`atm task review approve|request-changes|reject` in the Rust atm** (T-466): a review's decisions on the state
  machine of T-465, each leaving the task in review with its `review_state`; `--reason` required as Click words it.
  Five more tests left the expected-failures list.
- **`atm loop next` in the Rust atm** (T-467): the selections (`--sprint`, `--tasks`, `--all`, `--backlog`, `--type`,
  `--with-user`) on T-464's loop rules, expired claims released and other worktrees' decisions applied first, the
  reason and exit code 3 when nothing is ready, and `--claim` starting and claiming the task in the same change.
  Forty-two more tests left the expected-failures list, among them the 16-agent stress test.
- **Claims in the Rust atm** (T-446): `atm claim list|heartbeat|hold`, `heartbeat --auto` as the plugin's hook (the
  session and folder from stdin, silent, never failing), and every command an agent runs renewing its own claims ten
  minutes on (not one run with `--agent`). `claim hold`'s watcher is the binary itself, started detached as the
  hidden `atm _claim-watch`, following the shell by pid and start time (`sysinfo`); invariant I24's check
  (`security-invariants.test.ts`) covers the Rust atm: it starts only `git` and itself. Fifteen more tests left the
  expected-failures list.
- **`atm task edit` in the Rust atm** (T-470): every option of the Python command (name, description or its file,
  sprint or backlog with the sprint's history, type, priority, release, review, `with`, `--components auto`, and the
  add/remove pairs for files, dependencies, labels, components, related tasks, requirements and use cases), checked
  before anything is written (cycles, missing or self dependencies, empty requirements), what changed in words, and
  `--json`. An unknown option is reported as Click words it (`No such option: --status`). Twenty-two more tests left
  the expected-failures list.
- **The `sprint` group in the Rust atm** (T-471): `atm sprint list|add|edit|start|close|delete|move|backlog` as in
  Python: generated names (`people`, picked with `ATM_SEED` in tests, or `time` in the local time zone, read with the
  new `jiff` dependency), goals, releases and due dates, one active sprint, closing with its report and the
  unfinished tasks carried to a sprint, a new one or the backlog (`--carry-to`, `--name`, `--reviewed`), deleting one
  made by mistake (`--move-to`, `--force`), and moving tasks with their `sprint_history`. A missing argument is
  reported as Click words it (`Missing argument 'NAME'.`). Forty-seven more tests left the expected-failures list.
- **The `release` group in the Rust atm** (T-472, split with T-473 and T-474): `atm release list|add|edit|archive|notes`
  as in Python: releases with their tasks by kind and stage (a task's release its own, else its sprint's), `--pr`
  giving a new release every task of those pull requests in both files, `--released-at`, `--name` and `--target`
  (empty removes them), archiving, and the Markdown notes draft. Thirteen more tests left the expected-failures list.
- **The `component` group in the Rust atm** (T-473): `atm component list|add|edit` as in Python: globs (`--path`,
  `--add-path`), `--label` and `--review` (empty removes them on edit), `--from-label` moving a label onto the
  component on every task of both files, `--assign` giving it to the tasks its globs match, and their checks. An
  unknown option is reported with the command's usage line, as Click does. Sixteen more tests left the
  expected-failures list.
- **The rest of `atm project` in the Rust atm** (T-474): `project config` (where the task files are, a legacy one
  too), `project workflow [set]` (its defaults, trailers, the remote's default base), `project logo [set|unset]`
  (meta.logo or one found in the usual places, refused outside the project, too large or not an image), and the
  dashboard's list of projects, `project list|add|remove|prune`, in `$ATM_HOME/projects.json` (`atm-db`
  `registry.rs`: a linked worktree registers its main one, an unreadable file is moved aside, project IDs from the
  path's SHA-1 as in Python); `claim heartbeat --auto` lists the project once a day. Forty-eight more tests left the
  expected-failures list.
- **`atm-vcs` and the `vcs` group in the Rust atm** (T-448, split with T-475 and T-476): git as the code's version
  control (`atm-vcs`: the files a commit stages, literal pathspecs, whole-tree pathspecs refused, deletions, `--dir`'s
  changes under a folder), `atm vcs commit` (the workflow's template and trailer rule, `--record` for an existing
  commit, refusing a file in another agent's claimed task), `vcs pr`, `vcs pushed` (merges recorded, releases
  completed by the PR released) and `vcs repository` (web addresses from remotes, PR links moved; `atm-common`
  `repository.rs`). Invariant I24's check lists git in `atm-vcs` too. Thirty-one more tests left the
  expected-failures list.
- **The merge driver in the Rust atm** (T-475): `atm db merge-driver` (`atm-db` `jsongit/merge.rs`) merges tasks.json
  task by task as the Python atm does: the side further along wins a task's progress, notes, blocks, reviews and the
  sprint history grow, `related` merges as a set, theirs' new tasks with a taken ID are renumbered with their
  dependents, sprints, releases and components merge item by item (the further state wins), and a real conflict keeps
  ours and says where. Seeded property tests check that an unchanged side changes nothing and that every task keeps
  an ID of its own. The real-git merge tests follow with `db init` (T-476). Seven more tests left the
  expected-failures list.
- **The `db` group and `doctor` in the Rust atm** (T-476): `atm db migrate` (`atm-db` `jsongit/migrate.rs`: a legacy
  `docs/tasks.json` moved with `git mv` or renamed, `.gitignore` entries, finish times filled in, phases turned into
  sprints, `interactive` labels into `with: user`, old per-project dashboards and servers cleaned up),
  `db init [--meta-file]` (meta merged or the task file created, `meta.id`, `meta.repository` from origin, the
  registry, the merge driver in git config and `.gitattributes`; `jsongit/setup.rs`, registering the Rust binary
  itself), `db archive [--sprint] [--before] [--keep-days]` and `db unarchive` (`atm-lib` `archive.rs`: what stays
  and why, duplicates dropped, the archive's `.gitattributes` line on the first move) and `atm doctor [--files]`
  (`atm-lib` `check.rs`). The store gained `exists`, `create` and `raw`. Eighty-nine more tests left the
  expected-failures list; the one that checks the merge driver points at Python's launcher stays until the switch.
- **`atm service` and its server in the Rust atm** (T-449): `atm service start|stop|status [--open] [--all] [--port]
  [--dir] [--refresh]` (`atm-cli` `commands/maintenance.rs`, with `doctor`) starts the binary itself, detached, as
  `atm _serve <home> <port>`: one server per `ATM_HOME`, its record in `server.json`, the server lock so two starts
  start one, a stale record removed, `stop` ending only a server whose whoami confirms it (SIGTERM, then SIGKILL) and
  this folder's unrecorded ones, `--refresh` and every other command restarting an older dashboard on the same port
  (a newer one, or another install of the same version, is left alone). The server (`atm-api`, on `axum` and `tokio`,
  the new dependencies) listens on `127.0.0.1` and `[::1]`, moves past a busy port, answers only GET and HEAD for the
  Host `localhost:<port>` or `127.0.0.1:<port>`, sends `nosniff`, CORP same-origin and `no-store` on every response,
  and serves the dashboard and CompMD's runtime embedded in the binary (`atm-common` `dashboard.rs`; `atm-api`
  `build.rs`), the page at each of its paths with its CSP (the inline scripts' SHA-256, `atm-common` `sha256.rs`), and
  `/__atm/whoami`. Invariant I24's check lists the server and the browser opener; the read API follows (T-468).
  Eighteen more tests left the expected-failures list.
- **The read API of the Rust atm's server** (T-468): `/api/projects` (each registered project's summary, `atm-lib`
  `summary.rs`: counts by kind, the active and recent sprints, who works on what, next, needs-you, the git line, the
  board and its active-sprint view, the stats; a project that can't be read listed as missing), `/api/v1/projects/<id>/tasks`
  (the tasks with what atm derives, filtered by the query, the tolerant read of a malformed task file with its
  problems, T-377), `/p/<id>/claims.json`, `worktrees.json` (`atm-db` `jsongit/worktrees.rs`, `atm-lib`
  `worktrees.rs`) and `logo`, each with an `ETag` and 304s; the hidden `atm project summary`, and
  `atm task list --derived --tolerant`. `atm service stop` counts a process gone only after three missed checks in a
  row, re-sending SIGKILL while it is seen, and waits for it to be reaped. Twenty-eight more tests left the
  expected-failures list; the document viewer and search follow (T-469).
- **The document viewer's files and search in the Rust atm's server** (T-469): `/api/v1/projects/<id>/documents` and
  `files` (`atm-lib` `documents.rs`: the path checks, no hidden, ignored or key file, size caps, no symlinks, images
  by their bytes, code fenced in its language; `atm-vcs`'s reads of the working copy, a git submodule's files by its
  own rules) and `/api/search` (every word, in any field or the project's name, capped). `atm-common`'s errors gain
  `Refused` and `TooLarge` (403 and 413). `pnpm run test:e2e:rust` runs the dashboard's e2e suite with the Rust atm
  giving the fixture its task views and summaries; it passes. The last three `serve.test.mjs` tests left the
  expected-failures list.
- **Help, usage errors and shell completion in the Rust atm** (T-451, decided with Hakan): clap's help layout and
  usage lines, and its wording of usage errors (with its tips), behind atm's `atm:` prefix and, with `--json`, as
  `{"error", "code"}` (the message's first paragraph on one line); the Click rewording the Rust atm carried is gone.
  The help lists the commands everyday-first as before, `atm help` is a command of its own, and bare `atm` (or a
  group without its command) prints its help and exits 0. Shell completion is `clap_complete`'s dynamic completion
  (the new dependency), with atm's values: task IDs with their names, sprints, releases, components, statuses, types,
  priorities, `task state`'s targets, docs topics and harnesses; `atm completion <shell>` prints the line that turns
  it on. Help texts lost the backticks a lint had put into them. `atm vcs pushed` and `atm component edit
  --from-label` no longer write `"meta": null` into a task file without a meta. Scenarios hold the Rust text as
  `expectRust` until the switch. All but one test (the merge driver's path, at the switch) left the
  expected-failures list.
- **The skills' one source is `skills/atm-*/SKILL.md`** (T-441), portable to any agent harness: frontmatter `name`
  and `description` only, a version check first (`atm version`, at least 0.32.0), links to each other as
  `/atm-<name>`, reference detail from `atm docs` instead of repeated (the rules, leases, how task text renders).
  The plugin's `plugin/skills/` are generated from them by `atm-cli/build.py`, with Claude Code's `allowed-tools`
  (`skills/claude-code.json`), the `/atm:` links and the plugin's own `bin/atm` fallback; `build.py --check` (CI)
  fails when they are stale. CompMD's guide for agents moved from `plugin/docs/` to `docs/agent/compmd-llms.txt`
  and is `atm docs compmd`.

## [0.31.0] - 2026-11-06

The sprint *CLI ergonomics* (release 0.31.0).

### Changed
- **atm's commands are a tree of groups** (T-412–T-425, `docs/improving-atm-cli-ergonomics.md`), built with Typer
  on Click (`atm-cli/atm/cli/main.py`): `task` (`list`, `details`, `deps`, `add`, `edit`, `note`, `state`,
  `review`), `loop` (`next`), `claim` (`list`, `heartbeat`, `hold`), `vcs` (`commit`, `pr`, `pushed`, `repository`),
  `release` and `component` (`list`, `add`, `edit`, …), `project` (`status`, `config`, `workflow`, `logo`, `list`,
  `add`, `remove`, `prune`), `sprint` (`list`, `add`, `edit`, `start`, `close`, `delete`, `move`, `backlog`); `db`
  (`migrate`, `init`, `archive`, `unarchive`), `doctor` and `service` for maintenance; `help` and `version`.
  `db merge-driver`, `project summary` and `_lock-test` work but aren't listed.
- **`atm task state <id> <state>`** (`in_progress`, `in_review`, `done`, `todo`, `blocked`, `skipped`) is every
  status change: `todo` runs the one for where the task is (in progress: a failure with `--reason`, else a release
  with an optional `--note`; in review: a rejection; blocked: the answers, `--answer`; done or skipped: a reopen,
  `--reason`), here or, for a task claimed, blocked or in review in another worktree, there (`todo_command` in
  `lib/status.py`). It refuses options that aren't for the change (`--commit: not for in_progress (start)`) and asks
  for the ones it needs. The agent comes from `--agent`, `ATM_AGENT` or the session, as before.
- **A review's decisions are `atm task review approve <id> [--note]`, `request-changes <id> --reason` and `reject
  <id> --reason`** (T-425, T-438): the decision first, so a permission rule tells them apart. No skill pre-approves
  them (I23). Each leaves the task in review with its `review_state` (`approved`, `changes_requested`, `rejected`)
  and opens one way out, which atm guards for agents and people alike: approved, `done` (or `in_progress` for what
  is left); changes requested, `in_progress`; rejected, `todo` (its work dropped; atm-lib `drop_task`) or
  `skipped`. Without the decision, atm refuses (`can't finish T-1: its review isn't approved …`). The task's
  `waiting` record carries the decision, so an exit taken in another worktree meets the same guard. A rejected task
  isn't handed out by `atm loop next`. The dashboard labels `rejected`. `reject_task` is the decision now (it used to
  send the task back to to do).
- **`skipped` from any open state** (T-438): to do, blocked, in progress (its claim dropped) or in review (once
  rejected). **`done` takes no `--resolution`**: a done task is done; one that ends without its work done is skipped,
  with its resolution. atm-lib's `done_task` loses `resolution` (`tests/lib/public-surface.json`).
- **`--json` (`-j`) and `--dry-run` (`-n`) go anywhere on the line** (before a `--`); `atm --dir PATH <command>`
  as before. Errors are `atm: <message>`, or `{"error", "code"}` with `--json`; a usage error (Click's: a missing
  argument, an unknown option or command) has the command's usage line and exit code 2. Shell completion for zsh,
  bash and fish (`atm --install-completion`), with task IDs, sprints, releases, components and the vocabularies.
- **Messages and hints name the new commands** (`atm db archive` would move …, `atm sprint move <id> --to backlog`,
  `Run atm task state T-1 in_progress, then atm task state T-1 done again.`, `run atm doctor`, …).
- **The merge driver is `atm db merge-driver %O %A %B`**: `atm db migrate` and `atm db init` register it again.
- **The plugin's hooks** run `atm claim heartbeat --auto` and `atm service --refresh`.
- **The skills, `plugin/CLAUDE.md` and the README** use the new names; each skill pre-approves its commands one by
  one (`Bash(atm task state:*)`, …), never a review decision; `/atm:close-sprint` pre-approves no
  status change, so a skip asks for permission. An approval or a request for changes is the user's.
- **Typer and Click ship with the plugin** (T-425), copied as they are into `plugin/scripts/vendor/` (click 8.1.8,
  typer 0.15.4, as `atm-cli/uv.lock` pins them; without Typer's rich support, so help is plain Click output on every
  machine); `atm.py` puts the folder first on its import path. Nothing is installed; CI's pip step is gone.
- **The task state machine** is `transitions` (T-417, T-418), vendored and built into `atm.py`, with the same
  refusals, notes and exit codes as before.
- **The dashboard is a SvelteKit app** (T-428, the Svelte guide): SvelteKit 3.1.1 with the static adapter, its
  options in `vite.config.ts`'s `sveltekit({ … })` call (no `svelte.config.js`); rendered in the browser only, from
  the adapter's fallback page with every script and style inlined (`output.bundleStrategy: 'inline'`), so it is still
  one self-contained file, which `build/finish-page.ts` stamps (version, favicon, build ID) and writes to
  `plugin/templates/tasks-dashboard.html` (`pnpm run build:page`). About 60 kB more (SvelteKit's runtime; 23 kB gzipped): the size guardrail is
  650 KiB. The dev page's Content-Security-Policy is now worked out from each page as sent (SvelteKit renders it per
  request, with its own inline start-up script): its inline scripts by hash, as `atm serve` allows the built page's
  (`dev-api.test.mjs` checks it). `vite-plugin-singlefile` is gone.
- **The dashboard's views have addresses** (T-429, SvelteKit routes): `/projects/<id>/<tab>` a project on a tab
  (`/projects/<id>` on its saved one), `/projects` the overview's cards, `/board`, `/progress` and `/agents` its other
  tabs, `/` the landing page (a single project then takes its own address). What is open over a page is in the query:
  `?task=T-1` a task's details (new: linkable; opening them is a history entry, so Back closes them), `?doc=` a
  document, `?sprint=` the Sprint filter, `?about`, `?help`. `atm serve` answers those paths with the page
  (`PAGE_PATH`, `docs/multi-project-api.md` §2.7) and nothing else under them. Old `#project=…&view=…` links still
  open and move to the new address. A link in task text to a dashboard page opens in place. The page's API requests
  are from the server's root (`/api/…`, `/p/<id>/…`), and so is a project's logo. `lib/routes.ts` and `lib/address.ts`
  (instead of `lib/hash.ts`), `lib/navigation.ts` (SvelteKit's `goto`), `lib/sprint-param.ts` (was `sprint-hash.ts`).
- **The dashboard's dialogs, menus and popovers are Bits UI's** (`bits-ui` 2.20.6, T-431): `Dialog.svelte` (About,
  Help, What's new) on its dialog (focus trapped while open, the page under it doesn't scroll); the "need you" list
  and the scope bar's sprint menu on its dropdown menu (roving focus, typeahead; the sprints as radio items); the
  settings, the Filters popup, the project selector and the ⓘ tips on its popover. Each is placed where it was (its
  static content, no floating positioning), with the same ids and classes; the menus aren't modal. About 150 kB more:
  the size guardrail is 850 KiB, only to catch a runaway build (no size target until the Rust/Tauri move sets new
  ones). `runed`'s SvelteKit 2 peer is allowed for 3 (`pnpm-workspace.yaml`).
- **The dashboard's design tokens are one file**, `src/tokens.css` (T-433): the colours (moved from `app.css`; the
  themes set them, as before), a spacing scale (`--space-1` … `--space-10`: 2, 4, 6, 8, 10, 12, 16, 20, 24, 32 px),
  radii (`--radius-sm|md|lg|xl`: 4, 6, 8, 12 px, `--radius-pill`, `--radius-round`), text sizes (`--text-2xs` …
  `--text-4xl`) and the page's font. Every padding, margin, gap and radius uses them: a value off the scale moved to
  the nearest step (105 of 571; mostly 1 to 2 px), radii of 16 px and over are pills; text sizes were replaced where
  one matched, the others kept, and `em` values and layout sizes stay as they are. `tests/unit/tokens.test.ts` keeps
  it so.
- **What can be clicked on the dashboard is a control** (T-434): a card's task name and its dependency links, and
  the details panel's dependency rows, are buttons (reached with Tab, pressed with Enter), looking as before.
  `tests/unit/accessible-controls.test.ts` fails on a click handler on a `div` or `span` (but a card that is a
  button by ARIA, and three containers, each with its reason) and on an icon-only button without `aria-label`.
- **The dashboard's components are grouped by area** under `src/lib/` (`shell/`, `ui/`, `kanban/`, `tasks/`,
  `graph/`, `details/`, `dialogs/`, `documents/`, `progress/`, `agents/`, `overview/`; T-430). SvelteKit's app
  version is the plugin's, so the same sources build the same page.
- **The icons come from Lucide's package** (`lucide` 1.69.0, T-432) instead of embedded SVG copies: imported by
  name, drawn by `Icon.svelte` as elements, never as markup, so `{@html}` is left only in `Logo.svelte` (I15). Its
  Svelte package was 14 kB larger (a component per icon); this is about 3 kB more than the embedded copies. Lucide
  now calls `history` `rotate-ccw-clock`.
- **The dashboard's development uses pnpm** (T-427): `pnpm-lock.yaml`, overrides in `pnpm-workspace.yaml`, CI and
  the hooks on pnpm 12.14.0.

### Added
- **`atm claim hold <id> [--max MINUTES] [--every SECONDS]`** (T-437): keeps a claim alive while the shell that ran it
  lives (`atm claim hold T-1 && npm test`), and runs nothing. It renews the claim (refused when it isn't yours), leaves
  a detached watcher and returns; the watcher renews three times per lease (at least once a minute) and stops when
  the shell ends (checked by its pid and start time), a renewal fails (the claim was released, finished or taken
  over), or `--max` is up (240 minutes by default, at most 720). `--json`: `{task, claim, watcher, watching,
  until}`. The skills pre-approve it and use it before long commands.

### Removed
- **The old flat commands** (`atm start`, `done`, `review`, `block`, `show`, `list`, `sprints`, `commit`, `serve`,
  `check`, …): "No such command". No aliases.
- **`atm loop run <id> -- <command>`** (formerly `atm run`, T-437): atm runs no command it is given (I24,
  `docs/security-architecture.md`); `atm claim hold` replaces it.
- **`atm2`** and its launcher: the tree is `atm`.

### Docs
- **ADR-001**: filtering and search stay in the dashboard (T-435). The coding guides (`docs/guides/`) are linked from
  `AGENTS.md`.

## [0.30.0] - 2026-11-04

The sprint *Viewer fixes* (release 0.30.0).

### Added
- **`atm commit <id> --dir FOLDER`** (T-405, repeatable, beside `--file`): every change git lists under the folder
  (modified, new, deleted, renamed), for a generated folder the task rebuilt, such as CompMD's runtime after an
  upgrade (T-398 named 230 files one by one). Refused for the repository's root or above, a path outside it, the task
  folder, a file, a folder with nothing changed, and, as a whole, when a changed file under it is one another agent's
  claimed task lists in its `files`. atm-lib: `folder_changes(project, task_id, folders, cwd)`
  (`tests/lib/public-surface.json`); the vcs: `changes_under`; `host/git.py`'s `run_git` takes `strip=False`.
  `/atm:do-task`, `/atm:loop-tasks` and `plugin/CLAUDE.md` say when to use it.

### Changed
- **CompMD 0.5.0** (T-406): `@compmd/processor`, `render`, `sandbox` and `vite` at 0.5.0, the runtime in
  `plugin/assets/compmd/0.5.0/` (0.4.0's removed). Its frames speak frame protocol version 4, so page and frames ship
  together; Shiki and Temml ship once, beside the frames (`highlight.js`, `math.js`), not also beside `compmd.js`.
  `plugin/docs/compmd-llms.txt` is its `llms.txt` at `v0.5.0`, unchanged since `v0.4.0` but for atm's header.
- **A Markdown document's gutter numbers every line** (T-406), 1, 2, 3…, as an editor's does, instead of each block's
  first line (T-400): each line beside its own text, where CompMD drew it (the view's `lines()`, re-read on its
  `layout` event), a paragraph's later lines too; lines that share a visual line share a row (`3–4`); lines that draw
  nothing (blank lines, frontmatter, fences, a diagram's source after its first line) spread over the gaps between,
  fainter. `:L<n>-L<m>` marks exactly those lines, in the gutter and on their text's boxes, not their whole blocks.
  `gutterRows` in `lib/compmd.ts` replaces `blockSources` and `placeBlocks`; `CompMd.svelte`'s `onlines` replaces
  `onblocks`.

### Fixed
- **The document viewer's contents stay readable with many headings**: the list's entries were squeezed to fit its
  height (a flex column whose entries' `overflow: hidden` let them shrink); now each keeps its height and the list
  scrolls.
- **A card on the Omniboard opens its task over the Omniboard** (T-411), instead of switching to the project's
  page: `openOnBoard` in `lib/state.svelte.ts` loads the card's project's data underneath (tasks, claims, worktrees,
  as its page would; not its saved view) and opens the details, kept current by the refresh until they close or the
  page changes (`peeking`); the document viewer opens for that project too. The project chip and "+ N more" still open
  the project; a card whose task its project's data doesn't have opens the project, as before. The Omniboard's ⓘ
  says so.
- **`atm help` lists every command's options again** (T-409): its overview had drifted from the commands' own
  `--help`: `edit`'s requirement, use-case, component, related, release, review and `--with` options, `add`'s, `status`
  and `list`'s `--release`/`--component` (and `list --archived`), `check --files`, `commit --record` and
  `sprints close --reviewed` / `sprints delete` are in it now; `add --requirement`'s help says an entry may be a link.
- **Files inside a git submodule open in the document viewer** (T-408): the viewer refused them as if git ignored
  them, since `git check-ignore` in the project's repository refuses a path inside a submodule and the check fails
  closed. The vcs layer now asks the submodule's own repository (`ignored` and `tracked`, down to the innermost
  submodule; any git error still refuses), and the documents route gives `submodule` (`{path, commit, repository}`,
  the vcs's new `submodule_of`), so **Open on GitHub** links to the submodule's repository at its commit (or doesn't
  show when its remote isn't a web address).

## [0.29.0] - 2026-11-03

The sprint *Document Viewer* (release 0.29.0): open the documents and files a task refers to inside the dashboard
(`docs/compmd-viewer-design.md`).

### Added
- **A project's files for the document viewer** (T-392): `atm serve` answers
  `api/v1/projects/<id>/documents?path=` (what the file is, and the CompMD document atm wraps it in: Markdown as it
  is, code in a fence of its language, an image as a Markdown image) and `…/files?path=` (an image's bytes). Read-only
  and checked by atm-lib (`lib/documents.py`): a relative path without hidden parts, no key files, no file git
  ignores, no symlink on the way, 1 MB at most (5 MB for an image). The version control gains `file_stamp`,
  `read_file`, `ignored`, `tracked` and `head`.
- **The document viewer's window** (T-393): `#doc=<path>[:L<n>[-L<m>]]` in the URL opens a file of the project in a
  window over the details panel, rendered by CompMD from the document atm wraps it in (`DocumentViewer.svelte`,
  `lib/documents.ts`). It moves by its title bar and resizes from a corner grip (the arrow keys too: Shift resizes),
  stays inside the window and keeps where it was (`documentViewer` preference); on a window narrower than 720 px it
  fills it. The document is a page on a desk (border, shadow, margins), which a theme may restyle (`--doc-page-*`):
  Hand-drawn writes it on its ruled notebook page, Sticky Notes tapes a sheet to its board. Scrolled, the title bar
  compacts, as the dashboard's and the dialogs' do (`compactAt` in `lib/scroll.ts`, shared by the three). Back (its
  button, Escape, the browser's) returns to the document before; × closes it, back past every document opened in
  it. The title bar has
  the document's first heading and path, **Copy path** (with `#L<n>`) and **Open on GitHub** (or GitLab, Bitbucket,
  Gitea: the file at the current commit, for a file git tracks). The file is asked for again on the dashboard's
  refresh, a 304 while unchanged; a missing file says "Not in the project", one atm doesn't show "Can't be shown here".
- **Lines and images in the viewer** (T-394): a code or text file has its line numbers in a gutter (its lines don't
  wrap; the block scrolls sideways), and `#doc=…:L40` or `:L40-L45` scrolls to the lines and marks them. A changed file
  re-renders where the reader is. A relative image in a document loads from the files route (resolved from the
  document's folder); any other image still shows its alt text. Markdown documents get their line numbers from
  CompMD's renderer once it gives them (T-400, blocked on that release).
- **Relative links and the Files list open the viewer** (T-395): a relative link in task text (`docs/x.md`,
  `docs/x.md#routes`, `src/app.ts#L40-L45`) opens the file in the document viewer, resolved from the project's root;
  one inside a document is resolved from its folder; one leaving the project stays text. `#L…` opens at the lines,
  another fragment at the heading with that slug (Copy path keeps it). Every file in the details panel's Files list
  is a link (a folder stays text). Help and What's new come with the plugin: their relative links stay text.
- **Line numbers in Markdown documents** (T-400): the viewer's gutter numbers each top-level block of a Markdown
  document with its first source line, counted from the file's first (a frontmatter's lines too), beside where the
  block was drawn; `#doc=…:L<n>-L<m>` scrolls to and marks the blocks those lines fall in. The mapping is CompMD
  0.4.0's: the document's request asks for `origins`, and the view's `rectsAt` places each block, measured at its first
  and last offset (`blockSources`, `placeBlocks` in `lib/compmd.ts`; `CompMd`'s `onblocks`). Every line numbered, as an
  editor does, waits for CompMD 0.5.0's text-precise positions (T-406).
- **Text-fragment links** (T-391): a link ending in `#:~:text=…` ([text fragments](https://web.dev/articles/text-fragments))
  opens the document at the first match, marked (the CSS Custom Highlight API), until the next click; prefix, suffix
  and ranges work, and text it doesn't find opens the document at the top with a note. A `requirements` or `use_cases`
  entry written as a link (`[SR-2.1](docs/x.md#:~:text=SR%2D2.1)`) is a link in the details panel; cards show its
  text. `/atm:create-tasks`, `/atm:do-task`, `/atm:loop-tasks` and `plugin/CLAUDE.md` tell agents to write references
  that way: relative links for files, text fragments for requirements and places in a document, with `-`, `,` and `&`
  encoded. Help says references open the document.
- **Contents in the viewer** (T-402): a Markdown document with two or more headings has a faint contents button at
  its top left, in view as it scrolls; hovered or focused, the button grows into the list of headings (h1 to h3, by
  level; at once with reduced motion), so the pointer is inside it all along, the section being read marked; a pick
  scrolls there and the list hides, as it does when the pointer leaves or on Escape (which closes the list before
  the viewer). Read from the headings CompMD rendered.
- **`atm edit` changes requirements and use cases** (T-401): `--add-requirement`/`--remove-requirement` and
  `--add-use-case`/`--remove-use-case` (each repeatable) take an ID or a Markdown link to where it is defined
  (`[SR-2.1](docs/x.md#:~:text=SR%2D2.1)`); an entry already there isn't added twice, and an empty one is refused
  (exit 2, nothing written). atm-lib: `edit_existing_task` gains `add_requirements`, `remove_requirements`,
  `add_use_cases` and `remove_use_cases` (`tests/lib/public-surface.json`). `/atm:create-tasks`, `/atm:do-task` and
  `plugin/CLAUDE.md` list the flags.
- **Requirements and use cases one a line** (T-403): the details panel lists them as it lists Files (`.detail-refs`,
  sharing `.detail-files`' styles), one entry a line; an entry written as a link looks like a file link, a bare ID is
  text. Cards keep their badges.
- **Document styles** (T-404): **Document style** in the ⋯ menu, under Theme (`#doc-style-select`, the `docStyle`
  preference, `DOC_STYLES` in `lib/appearance.ts`): **Auto** (the theme's page, as before), **GitHub** (a README:
  sans-serif, 980px, ruled `h1`/`h2`, striped tables), **Docs** (an 18px serif column 44rem wide) or **Magazine**
  (Didot/Bodoni headings, a double rule under the title, a drop cap, pull quotes, one red accent), each in light and
  dark. They are host styles, the dashboard's own (`themes/doc-styles.css`, after the ideas of CompMD's editor's
  `frame.css`), set on the viewer as `data-doc-style`; the themes' pages (Hand-drawn, Sticky Notes) apply to Auto
  only. A code or text file keeps its gutter and only takes the style's paper and colours. The single file grows by
  11 kB, to 571 kB.
- **The single file's guardrail goes from 550 to 600 KiB** (`tests/e2e/guardrails.spec.ts`, decided with Hakan): the
  document viewer took it to 559 kB, and CompMD's coming upgrade needs room.
- **Help has a Documents section** (T-396): what opens the document viewer, where it opens (a heading, lines, a
  piece of text), what it shows and how its window works.
- **A link to the dashboard itself opens in place** (T-394): a web link to the page's own address with a `#…` (a
  document's or a view's URL) follows its hash in the same tab, a history entry for Back; every other web link still
  opens in a new tab, without access to the page.

### Changed
- **CompMD 0.4.0** (T-398): `@compmd/processor`, `@compmd/render`, `@compmd/sandbox` and `@compmd/vite` 0.4.0 (the
  Mermaid pack stays 0.1.1); the runtime is `plugin/assets/compmd/0.4.0/`. **Math typesets** (Temml, as MathML) in
  task text, Help, What's new and documents, which 0.28.0 showed as TeX; **code blocks are highlighted** by CompMD
  (Shiki, GitHub's light and dark, following the dashboard's mode), in task text and in the document viewer, its
  gutter unchanged. No atm code changed for either, and the page's CSP is the same. `plugin/docs/compmd-llms.txt` is
  CompMD's guide at `v0.4.0` (its body unchanged). The runtime folder grows from 11 to 18 MB: Shiki's grammars ship
  in the page's runtime and again in the frames' (CompMD's packaging), each loaded only for a block of its language.
  CompMD 0.4.0 also maps a rendered block to its source lines (`origins`), which T-400 needs.

## [0.28.0] - 2026-11-03

The sprint *Rich Markdown* (T-284 to T-397): task text renders with CompMD (`docs/compmd-rendering-design.md`). Also
designed in it: opening referenced documents in a viewer (T-336, `docs/compmd-viewer-design.md`; built by the backlog
tasks T-391 to T-396).

### Added
- **Task text renders with CompMD** (T-284): the details panel renders a task's description, notes, the review note
  and a blocked task's questions with CompMD (Markdown with components) instead of the dashboard's own Markdown subset:
  GFM tables, task lists, strikethrough, footnotes, nested lists, alerts, and CompMD's layout and control components.
  The dashboard puts one frontmatter in front of every text when it renders it (never stored), giving the text its
  task as `md.data.task`: its fields and atm's derived values. Until CompMD has loaded the text's place is empty; if
  it can't load, the text shows as written. Readers never see CompMD's diagnostics.
- **Mermaid diagrams** (T-285): a ` ```mermaid ` block draws a diagram, through the `@comp-markdown/mermaid` pack
  (0.1.1), ambient, run by `@compmd/sandbox` in a strict frame (an opaque origin with no network). Diagrams follow
  light and dark mode; one that Mermaid can't read shows its code.
- **CompMD's guide for agents ships with the plugin** (T-286): `plugin/docs/compmd-llms.txt`, CompMD's `llms.txt` as
  of `v0.3.0`. `/atm:create-tasks` lists what renders in atm and points to it, as do `/atm:do-task`,
  `/atm:loop-tasks` and `plugin/CLAUDE.md`; agents read the local copy and never fetch it. Help describes what task
  text renders.

### Changed
- **`atm serve` serves the plugin's assets at `/assets/`** (T-284, T-285): the CompMD runtime
  (`plugin/assets/compmd/<version>/`: `compmd.js` and the processor's WebAssembly), the sandbox's frame documents and
  the Mermaid pack's module, built by `@compmd/vite` from `dashboard/compmd.config.json`. GET and HEAD only; `.js`,
  `.wasm` and `.html`; plain path segments; regular files reached through no symlink; readable cross-origin (the
  frames have an opaque origin), a frame document sandboxed (`Content-Security-Policy: sandbox allow-scripts`).
- **The page's Content-Security-Policy** adds `'wasm-unsafe-eval'` and the asset folder (with the page's own origin)
  to `script-src`, and `frame-src` for the same folder; never `'self'`.
- **Task text is made safe before it renders:** an image that isn't `data:` shows its alt text and is never
  requested; a link other than `https:`, `http:`, `mailto:` and `#` becomes text; a component the text defines in a
  fence is never given to the sandbox.
- **One Markdown renderer:** Help (`docs/help.md`) and What's new (`plugin/WHATS-NEW.md`) render with CompMD too,
  loaded the first time one of them opens; the dashboard's own Markdown parser (`src/lib/markdown.ts`,
  `Markdown.svelte`) is gone. Tooltips and card excerpts show text without its marks (`src/lib/plain-text.ts`). The
  single file is 9 kB smaller. About names what renders Markdown (*Rendering*: CompMD, with its version, licence and
  site, read from the installed package at build time), and Help links CompMD's and Mermaid's sites where it
  describes task text, and draws its diagram example.
- **Help and What's new are one kind of dialog** (`Dialog.svelte`'s `scrollItem`): the title stays, with a hairline
  under it, and the content scrolls below it without a scrollbar, the ▲/▼ hints counting Help's sections or the
  releases; once it has scrolled, the title compacts, as the dashboard's title bar does. What's new now moves focus in
  and back like the other dialogs.
- **What's new shows the newest 10 releases** (`RECENT_RELEASES` in `dashboard/build/changelog.ts`, the only ones
  embedded), ending with a link to `plugin/WHATS-NEW.md` on GitHub, which keeps every release.
- **README:** the Tenets section (T-389), and CompMD named, with its site, where it describes the dashboard.
- **`npm run size`** (T-342) lists what the single file is made of, largest first; Help and What's new are embedded as
  text and parsed when the page loads.
- **Development:** `npm run build` also builds the CompMD runtime (`vite.compmd.config.ts`); `npm run check` rebuilds
  it into a temporary folder and fails when the committed files differ. The dev server forwards `/assets/`.
- **Development dependencies:** `minimatch` (used by ESLint and typescript-eslint) is pinned to 11.0.1 through
  `overrides` in `dashboard/package.json`, for GHSA-g3hv-75wg-m249 (a denial of service through crafted patterns).
  `deepmerge` 4.3.1 (used by `@sveltejs/vite-plugin-svelte` on its own configuration) has GHSA-gcqx-w48m-w2h4 and no
  fixed release yet; neither ships with the plugin.

### Fixed
- **An ⓘ explanation near a popup's right edge was cut off** (Activity's *Where the time went*): the popover now moves
  left to fit inside what clips it (`InfoTip.svelte`).
- **`atm serve` lost track of running servers** (T-390): `stop` removed the server's record after five seconds even
  when the server was still running, and two `atm serve` at once (two sessions' hooks) could each start one, the
  second record replacing the first; the lost servers kept running, unseen by `status` and `stop`. Now `stop` ends a
  server that outlives the wait (SIGKILL) and drops the record only once it is gone; starting holds a lock
  (`ATM_HOME/server.lock`), so two starts make one server; and `status` and `stop` find a server of this atm folder
  that the record doesn't name, on the 20 ports from `--port` (`whoami` and the record now carry `home`, a hash of
  the folder's path): `status` reports it (`unrecorded` in `--json`), `stop` stops it.

### Known
- Math shows as its TeX source: `@compmd/sandbox`'s host mode doesn't typeset it yet (a CompMD fix to come).

## [0.27.0] - 2026-11-03

The sprint *Layered Architecture* (T-360 to T-388): atm-cli rebuilt in layers (`docs/layered-architecture.md`).
Commands, their output, exit codes and files are unchanged unless listed below; the atm-cli suite checked that at
every step.

### Added
- **A malformed task file no longer blanks the dashboard** (T-377): the dashboard reads the project leniently and
  shows what atm finds wrong in a footer notice, with the details in a list. Entries that aren't tasks are left out,
  tasks with bad IDs stay, a dependency list that isn't a list shows as the IDs it can read, and unusable vocabulary
  lists fall back to the defaults. A task with a problem is never *ready* (`derived.problems`, and `problems` on the
  response). Commands still refuse such a file. `atm list --derived [--tolerant]` prints the same views as JSON.
- **`GET api/v1/projects/<id>/tasks`** (T-364, T-365, T-372): a project's tasks as views `{task, derived}`, where
  `derived` holds atm's own kind, readiness, `waiting_for`, effective review mode and release, stage and problems.
  Also returns `meta` and `bytes`. Filters are query parameters; `?archived=true` returns the archive. Answers 304
  with an ETag, and 400, 404 or 413 on errors. The dashboard loads tasks and the archive only from this route.
- **atm-lib, a Python surface for tools** (T-376, T-383, T-384): `atm.lib` (its `__all__`) and `atm.boot`
  (`open_project`, `open_memory_project`, `open_registry`), imported from the repository's `atm-cli/`, not from the
  shipped `atm.py`. There is one function per status change (`start_task`, `review_task`, `done_task`,
  `reject_task`, `approve_task`, `request_changes`, `block_task`, `unblock_task`, `reopen_task`, `skip_task`,
  `fail_task`, `release_task`). Sprint, release and task edits take keyword arguments (`add_sprint`, `edit_sprint`,
  `close_sprint`, `delete_sprint`, `add_release`, `edit_release`, `edit_existing_task`). `commit_task` takes
  repository-relative paths. Also `expire_claims`, `get_workflow` and `task_views`. The names and signatures are
  frozen in `tests/lib/public-surface.json`.
- **Schemas for atm's data and answers** (T-362, T-367, T-382): in `atm-cli/atm/common/schema/`, generated into
  Python TypedDicts and the dashboard's TypeScript types. They include nullable fields, maps, the project summary
  and the `atm status` report.

### Changed
- **atm-cli is built in layers** (T-361 to T-376, T-378 to T-381, T-385): common, host, db, vcs, atm-lib, boot, cli
  and api, each in its own folder. The build refuses any import the layer rule forbids. Every command and the REST
  API go through atm-lib, and only `host/` runs git.
- **The CLI and the server are two processes** (T-379): `atm serve` starts the server through the plugin's internal
  launcher `plugin/scripts/atm-serve` (the same `atm.py`, run with `--server`), not `atm _serve-child`. Nothing
  changes for users, skills or permissions.
- **The dashboard takes its rules from atm** (T-366, T-371, T-372, T-387). It used to work these out itself:
  - a task's readiness and what it waits for;
  - whether it can be done together with you now;
  - its review mode and its release;
  - the archive notice.

  Worktree copies (`worktrees.json`) now give their tasks as `{task, derived}` views, computed in their own
  worktree. Every request the dashboard makes goes through `src/lib/api/http.ts` (T-365).
- **One meaning of "the branch"** (T-385): with a detached HEAD, the short commit ID is used wherever a branch is
  shown: claims, the project summary's `git.branch`, `working` and the worktree list. A repository with no commit
  yet reports its branch (e.g. `main`) instead of none.
- **Status changes are conditional writes** (T-368): when two agents start the same task at once, one gets it and
  the other gets *claimed*. `tasks.json` is written only when its data changed (T-369), so a read-only `atm next`
  doesn't make open dashboards reload.
- **The server checks the task folder's symlinks on every request** (T-374), not once at start: a file linked after
  the server started is refused too, and a task folder with a linked file is refused whole.
- **Development:** `npm run dev` runs the real REST API from this checkout on a temporary `ATM_HOME` (T-375, T-386).
  It restarts when `atm.py` is rebuilt, lists a folder without a task file as missing, and serves the page under the
  server's Content-Security-Policy plus what Vite needs.

### Removed
- **The raw routes `p/<id>/tasks.json` and `p/<id>/tasks-archive.json`** (T-374): they answer 404. A 0.26.0 page left
  open across the update keeps its board and offers the reload (checked in T-388).

### Fixed
- **`atm serve` could judge a project by another project's archive** (T-378): the archive lookup was global. Each
  project now uses its own.
- **The dashboard showed a task as waiting on an archived dependency** (T-366): an archived task counts as done, as
  it does in atm.
- **A task file with a string where a list belongs broke the page** (T-377).

## [0.26.0] - 2026-11-02

### Added
- **`atm archive` says what stays, and why** (T-352, found in T-277): after what moves (or would, with `--dry-run`),
  a line on the finished tasks kept in `tasks.json`, grouped by reason: in an open PR, finished in the last
  `keep_days` days, finished on or after `--before`, not in the `--sprint` chosen (`kept: [{id, reason}]` in
  `--json`).
- **`atm unarchive` says when the next archive moves a task back, and what a wrong ID is** (T-357, found in T-277):
  a task brought back that is still finished and old gets a line saying the next `atm archive` moves it back unless
  reopened (`moves_back` in `--json`); an ID it can't bring back is named as *already in tasks.json* or *no such
  task*, instead of one list for both.
- **`atm deps` lists archived dependents** (T-358, found in T-277): *is waited on by* also names the archived tasks
  that depended on the task, marked `(done, archived)` as the *waits for* side already marks them (also in `--json`).
- **`atm --dir PATH <command>`** (T-351, or `--dir=PATH`): runs any command on the project at PATH, as if started
  there (relative paths in the command count from PATH), without changing directory first; a missing folder is an
  error. `atm serve --dir` keeps its own meaning.
- **Graph auto-fit** (T-341): *Auto* (`#graph-auto-fit`, on by default, saved as `graphAutoFit`) refits the graph
  when the filters, data, Hide/Dim or panel size change; panning or zooming turns it off.
- **Fit All or Matches** (T-341): `#graph-fit-all` (F) and `#graph-fit-matches` (Shift+F, without the greyed-out
  tasks; disabled when the same). The last one pressed is what auto-fit fits (`graphFitTarget`). `fitBox`, `nodesBox`.
- **`atm commit <id> --record <rev>`** (T-343): records an existing commit on the task without committing, for a
  task whose commit a rebase or cherry-pick replaced; the revision must be a commit on the current branch. A hint
  when its message doesn't mention the task. `--file` is now checked by atm (needed unless `--record`).
- **Animated fits** (T-341): fits, zoom buttons and filter changes glide (280 ms, `interpolateView`); not with
  reduced motion.
- **`atm sprints delete <name> [--move-to S|backlog] [--force]`** (T-339): deletes a sprint created by mistake. An
  empty one goes at once, closed too; an open one's unfinished tasks move (backlog by default). Refused for a closed
  sprint that had tasks, while a task in it is in progress, and without `--force` for an open one with history.
  `/atm:plan-sprint` mentions it, for when the user asks.
- **Open tasks left in closed sprints** (T-330): `atm status` warns about them (`open_in_closed_sprints` in
  `--json`), `atm check` warns about each, and `atm reopen` of a task in a closed sprint suggests `atm move <id> --to
  backlog` or `--to active` (`hint` in `--json`), moving nothing. `/atm:plan-sprint` shows them first as candidates.
- **An Agents tab on the Projects Overview** (T-333, `#overview&view=agents`, remembered as `overviewTab`): every shown
  project's agents on one timeline, a lane per agent or person across projects naming the projects it worked in, a
  bar opening its task in its project. It reads each project's `tasks.json` and `claims.json` while shown
  (`lib/overview-agents.ts`; conditional on refresh), with its own zoom and Live. `AgentsTab` takes a timeline, view
  state and click handler. A project's own `#view=agents` no longer changes the overview's remembered tab.
  The overview's *Active only* is now *Hide inactive projects (N)*, the same setting, shown only when a project would
  be hidden; the separate "N inactive projects hidden" note is gone.
- **Activity on the Projects Overview** (T-328): the title bar's Activity chart and popup for every shown project
  together (statuses read by kind, task IDs as "Project: T-1"), its own period, and a task opening in its project.
  Stacked and coloured by project (the Progress tab's colours): the pill's bars, the completions, who was working
  and the time-per-task dots (`projectTasks`). The insights popups now scroll inside the window rather than run under
  the status bar. The overview now loads every project's
  `tasks.json` and `claims.json` while shown (`lib/overview-data.ts`, which `lib/overview-agents.ts` became), rather
  than the summary growing; `ActivityWidget` takes the tasks, period state, a click handler and the breakdown.
- **A Progress tab on the Projects Overview** (T-329, `#overview&view=progress`): a row per shown project with its
  done of total, its active sprint (done of total, due date, goal on hover) and open releases, opening the project's
  own Progress tab; then the burndown of every project's tasks together (last 14 days or all time, with the forecast)
  and the velocity: tasks done per day (last 14), week (last 10, the default) or month (last 6, 30 days each), stacked
  in a colour per project with a legend (the projects' sprints don't line up). `projectProgress`, `velocityByPeriod`
  in `lib/overview-data.ts`. The shipped file's size guardrail goes from 525 to 550 KiB (T-342 trims).

### Changed
- **Archive tests** (T-359, after T-277): scenarios for archiving a second time (the archive grows, `meta.archived`
  counts all), archiving with nothing to move, bringing back several tasks at once and `list --archived --json`;
  component tests for the subtitle's archived toggle and the List's *N archived tasks hidden* (new
  `TaskGrid.component.test.ts`).
- **The Omniboard shows the active sprints by default** (T-340): each project's active sprint's tasks, with their own
  counts and *+ N more*, from the new `board_active_sprint` in `api/projects` summaries (`null` without an active
  sprint; `atm summary` too). *All tasks* (`#board-scope`, remembered as `boardScope`) shows every task, as before; a
  note names the projects without an active sprint. An older server's summary falls back to the whole board.
- **Graph toolbar** (T-341, `.graph-toolbar`): grouped, with segmented switches and a compact zoom; under 920 px the
  hint hides and the toolbar wraps. e2e runs with `reducedMotion: 'reduce'`; `openDashboard(…, { graphAutoFit:
  false })` opens a project without auto-fit. The shipped file's size guardrail goes from 500 to 525 KiB (T-342 looks
  for what makes the file grow).
- **The tabs sit right under the title bar** (T-334), 12 px below the subtitle instead of 38 px (`.topbar`'s bottom
  padding and margin), on the overview and on a project's page alike; `.landing` lost its 24 px top padding.
- **One look for the tabs** (T-346): a project's view tabs look like the overview's, on a line with the selected tab
  open onto the page; the views no longer sit in a framed, filled panel (`.view-panels`), and their content lines up
  with the page's edges.
- **Richer Task List cards** (T-338, `TaskCard`, `lib/cardfacts.ts`): the description's first two lines and the
  latest outcome or handoff under the chips, and a column on the right with who created it, worked on it and (when
  someone else) finished it, when it was done, started or created, and its work time. One column under 760 px.
- **A new project card on the overview** (T-335, `ProjectCard`): some depth (a soft shadow, a light top edge, a lift on
  hover); a title band with the name on one line, a hairline under it and the 14-day sparkline at its right; what needs
  you as a badge on the card's top-right corner; the active sprint's ring marked; quiet labels (*Sprint*, *Now*,
  *Next*) and the last finished task led by its time; the stats line as a caption under the sprint line. The task at
  work (or next up) is a mini task card on a stack of tilted cards, one per other task in progress or ready (at most
  three, from the summary's board counts), with *+ N more*. A card with nothing to show centres its note.

### Fixed
- **A dependency on an archived task** (T-356, found in T-277): `atm add` (`--dep`, or `dependencies` in
  `--json-file`) and `atm edit --add-dep` accept an archived task, as `atm check` does (it counts as done); before,
  they said it didn't exist. An ID in neither file is still refused.
- **`atm show` marks an archived task** (T-353, found in T-277): `· archived` in its header, `archived: true` in
  `--json`; before, only its archived dependencies said so.
- **The Projects Overview counts each project's archive** (T-355, found in T-277): its Activity, Progress and Agents
  load each project's `tasks-archive.json` with its `tasks.json` (conditionally; a missing one is empty) and count the
  history as the project's own pages do (`withArchive`: a task in both files is the live one).
- **The archive one click away** (T-354, found in T-277): the subtitle's *N archived* is a toggle for Include archived
  (*showing N archived* while on, `aria-pressed`), and the List ends with *N archived tasks hidden · show*; before,
  only the checkbox at the foot of the Filters popup showed them.
- **The Activity popup's periods and hover** (T-350): a period still running (Today, This week, This month) spans
  all of it on the charts' time axis, what's to come empty (`rangeAxis`): on a Monday, This week was one bar across
  the whole chart. The hover's tooltip sits beside the pointer, on the side with room, inside the popup (it was
  offset by half a bar's width, so a wide bar pushed it out), lists at most five tasks, and keeps showing when the
  pointer moves straight from one chart to another. It is placed against the window, so near the popup's bottom it
  no longer grows the popup's scroll area (the popup flickered). The same in a project's and the overview's popup.
- **Scrolling honours “reduce motion”** (T-345): the detail panel's paging and sections, dependency links, Markdown
  anchors and scroll hints jump at once when the system asks to reduce motion (`scrollBehavior`), rather than glide.
  For development: screenshots have Linux baselines only, made in Docker (`npm run test:e2e:visual[:update]`), of
  parts rather than whole pages, with a small pixel tolerance; `npm run test:e2e` leaves them out; traces only on
  CI's retry; three tests that read the page before it was drawn now wait for it.
- **Projects Overview has an icon in the project selector** (T-348): a grid (`layout-grid`), where each project shows
  its logo.
- **The Omniboard's Active sprints shows the sprints' tasks, no more, no less** (T-349): its To Do column was the
  loop's list, missing the tasks done with you and those waiting on dependencies (and, in *All tasks*, every other
  sprint's), and Done held only the last 24 hours. Now To Do has every to-do task (`todo_order`: the ready ones in the
  loop's order first, then with you, then waiting; a card says *With you* or *Waits on T-…*, from `with_user` and
  `waiting_for`), and the active sprints' Done every finished task of them. *All tasks* keeps the last 24 hours'
  Done. The column is *To Do*, not *To Do · ready*.
- **“Needs you” counts only what you can take up now** (T-347): a to-do task to do with you (`with: user`) counts only
  when it is in the active sprint (any, without one) and its dependencies are done; before, every such task did, also
  in planned sprints and the backlog. Reviews and questions for you count as before. The same in `atm summary`
  (`needs_you.together`, `needs_you_tasks`) and a project's title bar (`needsYouReason`'s scope).
- **The “needs you” list on a project card stays inside the window** (T-332): lined up with the chip's right edge, it
  ran off the left of the window on cards in the first column; `NeedsYouMenu` now shifts it back in when it opens.

## [0.25.0] - 2026-11-01

### Fixed
- **The sprint table lists the active sprint first** (T-323): the open sprints follow the loop's order (the active
  one, the planned ones, the backlog), as the table says, not the order they were added in; a sprint started after
  others were planned was listed after them. The Progress panel's sprint rings follow the same order.

### Added
- **`atm components`** (T-324): lists the project's components (`meta.components`) with their label, globs, review
  mode and how many tasks have them, archived ones too. **`atm components add <id> [--label T] [--path GLOB]...
  [--review MODE] [--from-label L] [--assign]`** defines one in a single locked write: `--from-label` turns a label
  into the component on every task, in `tasks.json` and `tasks-archive.json`, and drops it from `meta.labels` (its
  text becomes the component's label); `--assign` also gives it to every task whose files its globs match. For
  projects that marked their parts with labels before components existed. It refuses a component already there, a
  bad ID or glob, and a label no task has (a typo), writing nothing. `plugin/CLAUDE.md` lists it among the *Define*
  commands.
- **`atm components edit <id> [--label T] [--add-path GLOB]... [--review MODE] [--from-label L] [--assign]`**
  (T-326): changes an existing component the same way, so a second label can move onto it (`--label` and `--review`
  empty remove them); it refuses an unknown component, naming them.

### Changed
- **The scope bar replaces the sprint header** (T-323, SR-7.2): the Kanban, List and Graph open on the active sprint,
  and the bar above them (`#scope-bar`) says which tasks they show, kept in step with the Sprint filter. Its menu
  (`#scope-trigger`) picks one: the active sprint, a planned one, the backlog, all tasks, or one of the last five
  closed; several, picked in the filters, show as *Filtered: N selected*. It carries the one progress figure for its
  tasks, and *showing N, filtered* (with ✕ to clear the other filters and the search) when they narrow the board
  further. Hints name the skill to run next: close the sprint (`/atm:close-sprint`) once its work is done, with its
  release ready to ship and the backlog to pull from; settle what's in review or blocked first; start a planned
  sprint, or one at all when none is active (`/atm:plan-sprint`). A project without sprints has no bar. The Sprint
  filter now scopes only the Kanban, List and Graph: the Progress tab shows every sprint (it focuses one itself, by
  its rows), and the Agents tab greys out by the other filters alone.
- **The active sprint, whichever it is**: the Sprint filter's value for the active sprint is `:active`, so a saved
  view follows the project to its next active sprint; while none is active it selects nothing. New views and views
  saved before this version without a Sprint filter open on it (the saved view's `version` is 2).
- **Markers for sprint states**: ▶ the active sprint, ✓ a closed one, in the scope bar and its menu, the Sprint
  filter and the Kanban's sprint lanes.
- **The List groups by sprint: the planning view** (T-323): where it shows more than one sprint (All tasks, or
  several picked in the filters), a section per sprint in the order of the work: the active sprint, the planned ones
  in order, the backlog (with how to plan it, `/atm:plan-sprint`), then the closed ones folded into one row
  (`#list-closed-toggle`; open while a search or filter narrows the list, and in a project not migrated yet, whose
  finished phases are read as closed sprints). Each section's header has its tasks, how
  many are done and the sprint's goal. *Group* (`#list-group`) also groups by label or by component, as
  `/atm:plan-sprint` groups the backlog, or not at all; it is part of the saved view (`listGroupBy`,
  `listShowClosed`). Group and Order sit at the scope bar's end, like the Kanban's options.
- **The Progress tab opens the burndown on the active sprint** (T-323): its tasks, since it started, its due date
  marked; *Show all sprints* and the range buttons still widen it, and with no active sprint it shows them all. The
  focus is `:active` in the saved view (`progressSprint`), so it follows the next sprint; views saved before this
  version without a focus take it too.
- **Velocity** (T-323, `#velocity`): under the sprint table, a bar per closed sprint, the last ten in order, with the
  tasks it got done (from its report, else its tasks done), the average as a dashed line, and what it carried over or
  had added mid-sprint on hover.
- **Released releases fold into one row** (T-323, `#release-released-toggle`): the open releases first, then
  "✓ 19 released · last 0.24.0", newest first when opened, each with its release date. A release recorded without
  tasks (recovered from git history) shows its name, date and PR only, no empty counts or notes draft.
- **List cards name their sprint**, instead of the phase badge left from before sprints, when the list shows several
  sprints and doesn't group by them (`.task-card-sprint`, as on the Kanban).
- **Search and filters move into the title bar** (T-323): the filter row under the title is gone. A compact search
  box (`#task-search`; `/` focuses it, and it widens while in use) and a **Filters** button (`#filters-toggle`) sit
  with the charts; the button's badge (`#filters-badge`) counts the groups that narrow the board. It opens a popup
  (`#filter-panel`): how many tasks match (`#match-count`) and what is selected (`.filter-chip`), one line per group
  with its most used values first and the rest behind "+N" (Labels with a find box once there are more than ten), and
  *Clear filters* at its top (`#clear-filters`: the other filters and the search; the sprint stays, it is the scope
  bar's), and Include archived at its foot. *Reset view* (`#reset-view`), which forgets the whole saved view (filters,
  tab, each tab's options and zoom), moved to the settings menu as *Reset this project's view*. Values are quiet text
  chips with a muted count, a selected one tinted; there
  is no "All" chip any more: nothing selected is all, and a group with a selection has *clear* by its name. The search
  is a magnifier until used. Escape or a click outside closes it. The scope bar's note names what narrows
  the board: "showing 12, filtered: Type Feature · Labels ux ✕".
- **A ⋮ settings menu** (T-323, `#settings-btn`, `#settings-menu`) at the title bar's right end holds auto-refresh
  (on/off, the interval, when it last loaded), mode, theme and, on a project's page, *Reset this project's view*. The
  reload button stays beside it, showing the auto-refresh state; the separate refresh and appearance menus are gone.
- **The Kanban's toolbar row is gone**: its completion ring of the tasks shown repeated the scope bar's progress, and
  its options (*Group by sprint*, only when the board shows more than one sprint, and *Show skipped*) moved to the
  scope bar's end. A project without sprints keeps *Show skipped* above the board.
- **The sprint is in the URL** (T-323): `#sprint=active`, `#sprint=Foundations,backlog` or `#sprint=all` opens the
  board on those sprints (names in any case; unknown ones, like a renamed sprint in an old link, are dropped). Picking
  a sprint in the scope bar or the filters writes it, so the address shares what is shown and Back returns to the
  previous sprint; a link with it wins over the saved Sprint filter, and opening another project drops it.
- **Components by their labels** (T-323): cards (now List cards too), the detail panel, the Component filter and the
  List's groups show a component's label ("REST API") rather than its ID, in the project's order. The Progress tab
  has a **By component** table (`#component-table`) for the focused sprint, or all: done, progress and what is left
  per component, tasks in none last; a row filters by its component. The Project tab lists the components with their
  task counts and globs.
- **The Project tab shows sprints, not phases**: its Vocabulary has *Sprints* (the active one ▶, the planned ones,
  the backlog, the closed ones folded) and its Stats *N sprints · M closed*.
- **Planned and closed sprints** (T-323): a planned sprint's hint says it starts once the active one is closed
  (`/atm:close-sprint`, then `/atm:plan-sprint`); a closed sprint's board folds its empty columns into slim strips so
  Done gets the width, its cards in a grid, and its release reads "released" once shipped.
- **Open tasks left in closed sprints** (T-323): a task reopened after its sprint closed (or merged back open) is
  work no loop takes. The scope bar warns on All tasks and Backlog ("N tasks open in closed sprints", a link to those
  sprints) and on the closed sprint itself; the List keeps such a sprint unfolded; their cards' sprint badge is marked
  ✓ with a dashed outline. atm's side is T-330.
- **The title bar**: the pills on the right line up with the title, and a hairline separates the title row from the
  line under it.

## [0.24.0] - 2026-10-31

**Before the first run in a shared project, update every agent's plugin:** this version turns the project's phases
into sprints, and atm 0.23 or older reads a migrated `tasks.json` as tasks without phases (`atm next` would order by
priority alone, `--phase` would select nothing; nothing is lost or rewritten).

### Changed
- **The skills end tasks by their review mode** (T-315, SR-5.4, SR-4.4): `/atm:do-task` and `/atm:loop-tasks` run
  `atm done` and follow the mode instead of reading "a review is asked for" from descriptions: `none` and `batch`
  are done (a `batch` note is what the end-of-sprint review checks); `blocker` and `async` go to review, `blocker`
  first (the user reviews it now, or an unattended loop stops), `async` while the loop goes on. Reviewing a task
  can approve it or ask for changes (`atm approve`, `atm request-changes`). `/atm:close-sprint` starts with the
  end-of-sprint review of the tasks waiting for it (fixes live, follow-ups `related` in the next sprint), then
  closes with `--reviewed`.
- **`/atm:create-tasks` proposes sprints** (T-303, SR-5.5): for new tasks, the active sprint, a planned one, a new
  one (added with `atm sprints add`, planned, never started) or the backlog, instead of phases and phase names; tasks
  go in with `"sprint"` (or none, the backlog). It pre-approves `atm sprints` and `atm backlog` instead of
  `atm phases`. `atm add` no longer needs a phase.
- **Sprints in the detail panel, the header and the overview** (T-298, SR-7.6, SR-7.7, SR-4.5): the detail panel's
  phase badge is the task's sprint (or *Backlog*, `#detail-sprint`), with its effective release
  (`#detail-release`), and a *Sprint history* when it was in more than one, flagged once carried twice. The header
  counts sprints and the backlog ("12 tasks across 3 sprints · 2 in the backlog"). `atm serve`'s project summary adds
  `active_sprint` (with progress, due date and goal) and `recent_sprints` (up to four: the open ones, then the latest
  closed), and its `next` follows the loop's default selection (the active sprint); a project card with sprints
  shows rings for them and the active sprint's line ("Ada Lovelace — 1 / 3 · due in 3 days", or "No active
  sprint"). Projects without sprints lost their phase rings with T-320 (Removed, below).
- **Kanban: Group by sprint** (T-296, SR-7.3) replaces *Group by phase* (`#kanban-group-by-sprint`): one lane per
  sprint with tasks, in the project's order, the backlog last (`buildLanes` in `lib/kanban.ts`, lanes
  `data-sprint`). A card names its sprint (`.kanban-card-sprint`, coloured by the sprint's place in the list) only
  when the ungrouped board shows more than one; the "Phase N" badge is gone. The saved view's `kanban.groupByPhase`
  is `groupBySprint` (an old view's value is kept).
- **The loop and do-task work by sprint** (T-302, SR-5.1, SR-5.2): `/atm:loop-tasks` takes a selection, passed to
  every `atm next`: `--sprint S` (repeatable; replaces `--phase`), `--tasks T-1,T-5`, `--all`, `--backlog`; none
  means the active sprint, or every task in a project without sprints. It stops when its selection has nothing
  ready and says why; for a finished sprint, what it got done and what's left, and that `/atm:close-sprint` closes
  it. `--continue` goes on to the next planned sprint (`--sprint` of it); the loop never closes or starts a sprint.
  It pre-approves `atm sprints` to read them. `/atm:do-task` without an ID takes the next task of the active sprint.
  The Kanban's **Next up** follows the same selection (since T-321 it is `atm next`'s own answer, below).
- **Phases become sprints** (T-292, `docs/sprints-and-releases-design.md` SR-8.1): `atm migrate`, which every skill
  runs first, converts a project with phases once, in `tasks.json` and `tasks-archive.json`. Each phase with tasks or
  a name becomes a sprint named after it ("Phase n" when unnamed, " (phase n)" added to a taken name), in phase order;
  one whose tasks are all finished is `closed` (start, end and a report from its tasks), the others `planned`, none
  active (it says to plan one with `/atm:plan-sprint`). A task's `phase` becomes `sprint` with a `sprint_history`
  entry from its `created_at`; `phase` and `meta.phases` are removed. The files are staged, not committed, and
  `atm migrate --json` lists the new sprints as `sprints_from_phases`. A task with a phase merged into a migrated
  project joins its phase's sprint at the next `atm migrate`. `atm serve` migrates the projects it serves too.
- **Progress tab by sprint** (T-297, SR-7.4): the phase table is a sprint table (`SprintTable.svelte`, `#sprint-table`,
  rows `data-sprint`): one row per sprint with tasks, in the project's order, then the backlog; a sprint's state and
  due date next to its name; a closed sprint shows when it closed and its report (carried to, added mid-sprint), and
  two or more fold into one row. Estimates add up in the loop's order: the active sprint, the planned ones, the
  backlog (`forecastSprints`, `nextInLoop` in `lib/forecast.ts`; `forecastPhases`, `phaseRanges` and `activePhases`
  are gone). The burndown focuses on a sprint; its default range is since that sprint (or the one the loop works on)
  started; a closed sprint's chart ends where it closed, and the due date of an open one is marked. The saved view
  keeps the focused sprint (`progressSprint`; a saved `progressPhase` is dropped, a `phase` range reads as
  `sprint`). The Progress panel's *By phase* is *By sprint*: a ring per open sprint and the backlog, one chip for
  the closed sprints.
- **The Kanban's Next up comes from atm** (T-321): the project's Kanban marks the first task of its summary's `next`
  in `api/projects` instead of applying the loop's rule itself, so it is always the task `atm next` hands out, also
  when the browser can't see why another isn't (a claim, a task done or waiting in another worktree). The summary's
  `next` now follows `atm next` exactly, tasks back from review first (`loop_order` in `claims.py`, shared by both).
  The dashboard's `nextUp()` and `next-up-parity.test.ts` are gone; the To Do column orders ready tasks by priority,
  then ID, for reading only.
- **Releases recovered from past PRs** (T-310, SR-8.2): `atm releases add` takes `--pr` more than once (a release
  shipped in several PRs gets all their tasks, the release records the last PR) and `--released-at TIME`, which adds
  it as released at that time. This repository's releases 0.5.0–0.23.0 were recovered that way, each released at its
  PR's merge; 0.5.0–0.12.0 have no tasks, as their PRs weren't recorded on tasks yet.

### Added
- **Review states, components and work together on the dashboard** (T-316, SR-6.6, SR-7.6): cards show a task back
  from review (*Changes requested*, *Approved*), *with you* for `with: user`, and its components; the detail panel
  also its effective review mode (`lib/review.ts`) and whether it waits for the sprint review. *Needs you* counts
  `with: user` tasks to do ("to do with you") and no longer a task back from review (an agent's work again), in
  the dashboard (`needsYouReason`) and in `atm serve`'s summary (`needs_you.together`). The sprint header says how
  many done tasks wait for the end-of-sprint review.
- **`with: user`** (T-314, SR-3.5, SR-5.3, SR-6.5, SR-6.6): a task done together with the user (`atm add` / `edit
  --with user`, removed with `--with ''`). `atm next` skips it unless `--with-user`, and says when only such work is
  left; `atm start <id>` takes it; `atm list --with-user` lists them. `/atm:loop-tasks --with-user` takes them;
  without it they are listed at the end as waiting for you. `atm migrate` gives unfinished tasks labelled
  `interactive` `with: user` once (recorded in `meta.migrations`; the label stays; `--json`: `with_user_from_label`).
  `/atm:create-tasks` sets it for work the user wants to do together.
- **`atm approve` and `atm request-changes`** (T-313, SR-3.9, SR-6.1, SR-6.2, SR-6.4): a task in review carries
  `review_state` (`waiting` after `atm review`, `approved`, `changes_requested`) and `reviewed_at`; both stay in the
  review kind (dependents keep waiting) and go when the task leaves it, the round recorded in `reviews` with its
  `outcome`. `request-changes --reason` adds a rejection note. `atm next --claim` takes such tasks back before new
  ones, giving them to the agent that worked on them for one lease length after the review, then to any agent;
  `atm start` takes one back too, but not one still waiting for its review. `atm reject` stays (back to to-do).
  `/atm:do-task` and `/atm:loop-tasks` record an approval with `atm approve` instead of a rejection. The merge
  driver merges the new fields with the progress fields.
- **Release view** (T-309, SR-7.5, `ReleaseView.svelte`, `lib/releases.ts`): on the Progress tab, each release not
  archived with its state, PR, target, the sprints that deliver to it, its tasks by stage (done, in PR, merged), and
  the notes draft (`atm releases notes`' grouping) to copy. The dashboard file's size limit (`guardrails.spec.ts`)
  goes from 450 to 500 KiB for the sprint and release views.
- **Review modes** (T-312, SR-3.8, SR-4.4): a task's effective mode (its own `review`, else its first component's,
  else `meta.review`) decides how `atm done` ends it. An agent's `atm done` on an active `blocker` or `async` task is
  refused (`atm review` instead); a `batch` task is done with its note as a `review` note and `sprint_review:
  pending`; `none` is done as before. A person (an actor not starting with `claude:`) may always `atm done`.
  `atm sprints close` refuses while a task in the sprint waits for the sprint review; `--reviewed` marks them
  reviewed. `atm status` and `atm sprints` list them; `atm reopen` clears `sprint_review`. `/atm:create-tasks`
  proposes a mode where one is needed.
- **Components** (T-311, SR-2.7): `meta.components` (`[{id, label, paths, review}]`, `paths` globs over the
  project's files, checked with `meta`). In a project with components a task's components must name them. `atm add`
  and `atm edit --add-file` print the components a task's files suggest (with the `atm edit … --add-component`
  that sets them); `--components auto` (and `"components": "auto"` in `--json-file`) sets them. `atm check` warns
  about a task whose files match no component. `/atm:create-tasks` proposes them.
- **`atm merged` releases a release** (T-308, SR-3.7, SR-5.6): when the PR it records completes a planned release
  (all its tasks in `tasks.json`, by their own release or their sprint's, skipped ones aside, merged in that PR), the
  release becomes `released` with its `pr` and `released_at`; otherwise atm says which tasks keep it planned
  (`--json`: `released`, `pending`). `/atm:push` relays it, and on the base branch of a release-branch project
  suggests `dev/<release>` from the active sprint's release, never renaming a branch.
- **`atm releases notes <R>`** (T-307, §3.3): a Markdown draft of the release's changes from its tasks (by effective
  release, `tasks.json` and the archive): the done tasks grouped by type in the project's order, each with its ID,
  name and last outcome note, then what isn't done yet; `--json` gives the same as data. For the CHANGELOG and
  WHATS-NEW entries a person writes.
- **Releases** (T-306, SR-2.2, §3.3): `meta.releases` (`[{id, name, state, target, pr, released_at}]`, states
  `planned`, `released`, `archived`, checked with `meta`) and `atm releases [--all]` (each release's tasks by kind and
  stage, by their own release or their sprint's), `atm releases add <R> [--name] [--target DATE] [--pr N]` (`--pr`
  gives it every task of that PR, in `tasks.json` and the archive), `atm releases edit`, `atm releases archive`. A
  sprint's and a task's `--release`, and the `--release` filters of `list` and `status`, must name a release
  (`no release '2.0'; the releases: …`); `atm check` reports one that doesn't exist. `atm check` no longer asks
  for a `phase`: migrated tasks have none (a `phase` that is there must still be a number).
- **`/atm:close-sprint`** (T-301, `skills/close-sprint/SKILL.md`, §4.3): closes a sprint with the user: reviews the
  done tasks with their outcome notes (follow-ups through `/atm:create-tasks`); decides each unfinished task (carried
  over by default, back to the backlog with `atm move`, or skipped with `atm skip`); chooses where carried tasks go
  (the next planned sprint, one of several, or a new one); closes it with `atm sprints close … --carry-to …` and
  offers `/atm:plan-sprint`. The user decides each step; it pre-approves only `atm migrate`, `status`, `sprints`,
  `list`, `show`, `deps`, `move` and `skip`.
- **`/atm:plan-sprint`** (T-300, `skills/plan-sprint/SKILL.md`, SR-4.1, SR-4.2): plans a sprint with the user: shows
  the active sprint and what's left, the planned sprints, the backlog by label and component, and the last closed
  sprint's carried tasks; agrees on the sprint (`atm sprints add`/`edit`); goes through candidates (`atm move`,
  dependencies flagged with `atm deps`), comparing its size with the last closed sprints; starts it only on the
  user's word (`atm sprints start`). It never starts, moves or dates anything without the user agreeing in the
  session, adds tasks only through `/atm:create-tasks`, and pre-approves only `atm migrate`, `status`, `sprints`,
  `backlog`, `list`, `show`, `deps` and `move`.
- **`atm sprints close <S> [--carry-to <S2> | new [--name N] | backlog]`** (T-299, SR-4.3, SR-4.5): refused while a
  task in the sprint is in progress; its unfinished tasks (to do, blocked, in review) go to `--carry-to`, needed when
  there are any, keeping their status and `sprint_history` (`new` adds a planned sprint right after it, named or
  generated); the sprint becomes `closed` with `end` and `report: {done, carried, added, to}` (`done` counts archived
  tasks too; `added`: joined after the sprint's `start`). `atm sprints` takes `--agent`.
- **Sprint header** (T-295, SR-7.2, `SprintHeader.svelte`): above the Kanban, List and Graph, the sprint the Sprint
  filter selects, or the active one when it selects none: its name (and state, when not active), goal, due date
  ("due in 3 days", "2 days overdue" in red), release, progress (done of the tasks not skipped) and, from
  `sprint_history`, how many tasks were added mid-sprint and carried in (`headerSprint`, `sprintSummary` in
  `lib/sprints.ts`).
- **Resolution and relations on the dashboard** (T-319, SR-7.6): the detail panel shows how a finished task ended
  when it wasn't done (`#detail-resolution`: won't do, obsolete, duplicate), what it duplicates and what duplicates
  it (`#detail-duplicate-of`, `#detail-duplicated-by`) and its related tasks both ways (`#detail-related`), each
  linked (a task that isn't in `tasks.json` is shown, not linked), in the Dependencies group. The Progress panel
  counts the finished tasks that weren't done apart from done, by resolution ("Not done: 2 won't do · 1 obsolete",
  `#progress-not-done`). The rules are in `lib/relations.ts` (`resolutionOf`, `relationsOf`, `notDone`).
- **Resolution and relations** (T-318, SR-2.8): `atm done --resolution done|wont_do|duplicate|obsolete` and
  `atm skip --resolution wont_do|obsolete|duplicate [--duplicate-of ID]` record how a task ended; only a resolution
  other than the default (`done`, or `wont_do` for a skipped task) is stored. `--duplicate-of` needs `duplicate` and
  a task that exists in either file; `atm reopen` clears both. `atm show` prints the resolution, the task's
  `related` tasks on both sides ("related: T-4, T-1") and "duplicated by: T-2". `atm check` checks them; the merge
  driver merges `related` as a set. None of it affects readiness.
- **`atm summary`** (T-321): the project's summary as `atm serve` lists it in `api/projects`, JSON, for the project it
  runs in. The Vite dev server serves it (its projects now have their numbers too), and the e2e fixture with
  `withAtmSummary()`.

### Removed
- **Phases leave atm-cli** (T-320, SR-8.3): `atm phases` (and `phases name`/`unname`), `--phase` on `next`, `list`,
  `archive`, `add` and `edit`, the phase column of `atm list` (now ID, status, priority, labels, name), the
  `by phase` line of `atm status` (a `by sprint` line instead, in a project with sprints) and the checks of
  `meta.phases`. `atm next` orders by priority, then ID; its line names the task's sprint ("Next: T-2 Two (Themes,
  feature)", or "backlog"), as `atm show` does. `atm serve`'s summary drops `current_phase` and `recent_phases`, its
  board cards carry `sprint` instead of `phase`, and its board's to-do column holds the loop's default selection; a
  project card without sprints shows one ring for all its tasks and "No sprints yet", and the All projects board's
  cards lose their phase badge. A `phase` left on a task is kept but ignored; `atm migrate` (run by every skill and
  by `atm serve`) turns phases into sprints first. `/atm:push` groups the PR body by sprint.

### Fixed
- `atm unblock` refused to record the answers when the questions and answers together were longer than 500
  characters; the limit now applies to each question and answer.
- `atm check` warned about every finished task's missing files without `--files` (since components, T-311): the
  option was hidden by a variable of the same name (T-318).

## [0.23.0] - 2026-10-30

### Changed
- **Kanban:** every status column stays on the board whatever the filters; a status filter empties the columns it
  leaves out instead of removing them (`buildBoard` in `lib/kanban.ts`). A skipped-kind column still shows only with
  *Show skipped* or when the status filter names it.
- **List tab** (T-281): the *Order* dropdown is drawn by the page (`.select-box` in `app.css`, the theme picker's
  style and chevron) instead of the platform's native control, whose font, padding and arrow changed between macOS
  and Chromium versions and kept failing the List screenshot tests on developer Macs.
- **Dependency graph layout:** laid out by dagre (`@dagrejs/dagre`, bundled into the dashboard) instead of columns
  by dependency depth: a task sits close to what it feeds instead of in the first column, a chain runs on one line,
  arrows cross less and follow dagre's route (`layoutGraph` in `lib/graph/layout.ts`; `topologicalLayers` and the
  layout's `layers` are gone). The layout is computed again only when the graph's shape changes (which tasks, their
  dependencies), so a reload that only changes statuses reuses it. The dashboard file's size limit
  (`guardrails.spec.ts`) goes from 400 to 450 KiB for dagre (about 48 kB).

### Added
- **Sticky notes theme** (T-283, `themes/sticky-notes.css`): the Kanban on a blackboard (dark) or a whiteboard
  (light), in a wooden or aluminium frame, with a chalk-dust or satin texture. The columns have no boxes: their
  headings and the lines under and between them are drawn in chalk or blue marker, by hand (SVG strokes that wobble,
  double up and break). The cards are paper sticky notes coloured by type (feature yellow, bugfix pink, test purple,
  infra blue, docs green, refactor and polish orange), tilted a little, with the name handwritten; inside a note the
  colour tokens are dark inks that read on every note colour in both modes (`theme-contrast.test.ts` checks the
  board and the notes). Card and board hooks other themes can set, defaulting to the old look: `--card-bg`,
  `--card-border`, `--card-radius`, `--card-shadow`, `--card-tilt`, `--card-font`, `--card-font-size`,
  `--board-surface`, `--board-frame`, `--board-radius`, `--board-shadow`, `--board-padding`, `--board-gap`,
  `--board-align`, `--board-font`, `--board-ink`, `--board-title-transform`, `--column-bg`, `--column-border`,
  `--column-radius`, `--column-divider`, `--column-header-line`, `--column-divider-image`, `--column-header-image`,
  `--column-header-overhang` (`app.css`). Cards carry `data-type` and `data-priority`; a column heading's status
  colour is `--column-color`.
- **Hand-drawn theme** (T-283, `themes/hand-drawn.css`): the Kanban sketched on a notebook page, cream paper in
  pencil (light) or a black notebook in white gel pen (dark), ruled, with a red margin line and punched binding
  holes. The columns, the lines under their headings and the cards' boxes are drawn by hand (the shared
  `--column-*-image` hooks; the boxes are a mask, `--card-sketch`, painted in the colour of the card's column), the
  page shows through the cards, and the names are handwritten in bold (`--card-font-weight`, new). *Next up* is
  circled by hand in the pen's blue instead of the straight frame.
- **Theme menu:** a separator line sets the themes that draw the board (Sticky notes, Hand-drawn) apart from the
  colour themes (`group: 'board'` in `themes/index.ts`).
- **Dependency graph hover:** hovering a task highlights it and every task it waits for, directly or not (a bolder
  border and a glow in the task's colour, the arrows between them bold in the accent colour); the rest fades
  (`dependencyChain` in `lib/graph/layout.ts`). A greyed-out task on the chain lights up too.
- **Dependency graph:** a *Dim done* button among the graph's controls greys out done tasks and the arrows touching
  them, whatever the filters, alongside *Filtered out: Hide / Dim*. Saved with the project's view (`graphDimDone`).

## [0.22.0] - 2026-10-27

### Changed
- **Task details** (T-280): the title, with the × on its line, and the badges stay at the top; the sections scroll
  below them without a scrollbar, like the Kanban columns, with *▲ N more* / *▼ N more* hints (sections cut off) that
  scroll a page. Less padding around the panel.
- **Section rail:** on the left of the sections, one entry per group the task has: *Info* (in review or blocked, the
  description), *Dependencies*, *Notes*, *Activity* (claimed by, who, timestamps, commit) and *Ref* (use cases,
  requirements, files), written bottom to top with an icon. A click brings the group to the top, adding room below
  the last section when the content is too short; scrolling by hand never adds any. The entry of the group in view
  is highlighted. The sections are in that order now (the Activity group was spread over the panel before).
- **Dependencies** in the details take one line each: a long name ends in "…" and shows in full on hover.
- **What's new** (T-282): the title, with the × on its line, stays at the top; the releases scroll below it without
  a scrollbar, with *▲ N more* / *▼ N more* hints (releases cut off). One scroller with these hints
  (`HintScroll.svelte`) now serves What's new and the Kanban columns, and one count of what is cut off
  (`cutOff` in `lib/scroll.ts`) serves those and the task details.

## [0.21.0] - 2026-10-27

**The repository moved to the `solo-swe` organisation.**

### Fixed
- **Activity popup heading:** it said *Activity per 3 hours* on *All time* and *per day* on *Week*: the bar size,
  which read as the period. It is now just *Activity*; the period label beside it names the period, and every chart
  heading names the period and its bar size (*Time per task* now names the period too). T-144 had fixed the chart
  headings only, and an e2e test pinned the wrong heading.
- **All projects board (Omniboard):** the page no longer scrolls on it: the board takes the window's height and each
  column scrolls on its own with *▲ N more* / *▼ N more* hints, like a project's Kanban (T-262). Grouped by project,
  the lanes scroll as one.

### Changed
- **Omniboard:** the All projects page's *Board* tab is called *Omniboard* (`#view=board` links and the remembered
  tab still work), with an ⓘ on what it shows and that each project's details are on its own Kanban.
- **Project tab:** the header is the logo and the name, without `meta.description` under it; side by side, the
  *Information* and *Stats* panels end at the same height.
- **Repository:** `github.com/solo-swe/agentic-task-manager` (GitHub redirects the old address). `plugin.json`'s
  `homepage` and `repository`, the dashboard's GitHub and Source links, the README and the docs point there.
- **Marketplace renamed** from `hakanserce-plugins` to `solo-swe-plugins`: the plugin installs as
  `atm@solo-swe-plugins`. An existing install doesn't follow the rename: `/plugin marketplace remove hakanserce-plugins`,
  `/plugin marketplace add solo-swe/agentic-task-manager`, `/plugin install atm@solo-swe-plugins`, `/reload-plugins`.
- **Titles:** the All projects page is titled *Projects Overview*; a project's page by its name alone, without
  *— Task Dashboard* (the title bar and the browser tab). *Task Dashboard* stays while loading and for a project
  without a name. The project selector's first option is *Projects Overview* too (typing its name finds it).
- **`atm repository set URL [--from OLD]`** also moves the tasks' recorded PR links (`pr.url`) from the old address
  (`--from`, else the one it replaces) to the new one, in `tasks.json` and the archive, without touching
  `merged_at`, timestamps or actors; `--json` adds `moved_links`.
- **The atm logo on GitHub:** the README is headed by the logo (`.github/assets/atm-logo.svg`, the dashboard's in the
  accent blue) instead of an emoji, and `.github/assets/social-preview.png` is the repository's preview card.
  `.github/assets/atm-logo.png` (512×512) is the logo as a PNG, for avatars.

## [0.20.0] - 2026-10-27

**Security hardening (phase 20) and progress and insights (phase 21).** Phase 20 implements the security review
(`docs/security-review.md`, `docs/security-architecture.md`); phase 21 the reviews of the Progress tab, the Activity
popup, project stats and the All projects Kanban.

### Security
- **Skills:** task text, notes and CI logs are data, not commands (`plugin/CLAUDE.md`, every skill); agent text is
  quoted in commands; skills pre-approve only the `atm` subcommands they need, never `atm run`.
- **atm-cli:** task IDs and the base branch are validated; `atm commit --file` takes exactly one literal file per
  flag; no reads or writes through symlinks in the task folder; robust against hostile task files (deep nesting,
  huge values, wrong types); `atm pr` accepts only http(s) URLs.
- **Server:** binds to loopback only, with Host checks, a Content-Security-Policy, `nosniff` and CORP headers, request
  limits; one broken project can't break `api/projects`.
- **Dashboard:** every link goes through one `safeUrl` check.
- **CI:** read-only token, no persisted credentials, actions pinned by SHA, Dependabot.
- **Verification:** a hostile-repository test suite; the agent sandbox (`tests/sandbox/run.mjs`) runs red-team tasks
  against the skills in a container; a verification schedule (every PR, every release, every quarter); `SECURITY.md`.
- **Loop:** `/atm:loop-tasks` confirms `--push` in the direct workflow and stops when a hook rejects a commit.

### Added
- **All projects: Projects and Board tabs** (remembered; `#overview&view=board` links to the board). The board shows
  To Do (ready), In progress, In review, Blocked and Done (last 24 h) across projects, from a new `board` summary
  field (`docs/multi-project-api.md` §3.3), sorted per column, 30 cards per column with *+ N more in <project>*, a
  project chip on each card; *Group by project* lanes; a search box over every project's tasks.
- **`GET api/search?q=`**: the task IDs per project matching every word (ID, name, description, type, labels, notes,
  project name); read-only, capped (§2.4).
- **Project tab** (last tab): Information (repository, workflow, people, agents, vocabulary) and Stats (started,
  duration, tasks, phases, commits, PRs, agent time alone and with you, parallelism, a time-per-task histogram with
  the slowest tasks).
- **Shared time use** (`timeuse.ts`): agents working, waiting on you, idle; the *Where the time went* bar on the
  Activity popup and the Progress tab.
- **atm records block and review intervals** (`blocks`, `reviews` on a task), merged by the merge driver, so waiting
  time can be measured.
- **Project summary `stats`**: started, tasks and agent minutes, archived tasks included; the overview cards show one
  line (*started 12 Aug · 236 tasks · 74 h agent time*).

### Changed
- **Tab order:** Kanban, List, Graph | Progress, Agents, Project; the Kanban is the default.
- **Activity popup:** three KPI cards (tasks done; time per task, agents alone and with you; where the time went);
  the agents chart stacked by kind with a waiting-on-you band; the time-per-task chart capped at the 90th percentile
  with outliers on the top edge and interactive tasks hollow with their own median; ⓘ notes on the heading and every
  chart. The sparkline draws the unfinished last bucket lighter and its tooltip explains the line. The buckets lose
  the unused `cumulativeDone`, `scope` and `wip`.
- **Progress tab:** agent time, waiting on you and idle replace utilisation; a filter banner (status and stage
  filters don't apply); the burndown has a title with ⓘ, a time range and what's waiting on people; the phase table
  shows finished phases in one row.
- **Header Progress popover:** finished phases as chips, types as one stacked bar, a Go to section.
- **About dialog:** statuses, phases and types only, with a *Project details* link to the Project tab.
- **Board cards** in progress carry `since` (`started_at`).
- **Detail panel:** a linked commit says *The link works once the commit is pushed*, unless its task is in a PR or
  merged (the dashboard doesn't know what was pushed).
- The dashboard's size budget is 400 KiB.

### Fixed
- **Kanban:** no page scrollbar; each column scrolls on its own, with *▲ N more* / *▼ N more* hints instead of a
  scrollbar (the wheel and trackpad still scroll).
- **Back to the overview** no longer leaves a task's detail panel open over it.

## [0.19.0] - 2026-10-26

**Insights reviewed, a security review, and dashboard polish.** Phase 19 reviewed the Progress tab, the Activity
popup, project stats and an All Projects Kanban with the user (`docs/progress-tab-review.md`,
`docs/activity-popup-review.md`, `docs/project-stats-proposal.md`, `docs/all-projects-kanban-proposal.md`); their
implementation is phase 21. The security review (`docs/security-review.md`: threat model, 19 findings) added phase 20.

### Added
- **Markdown in notes, questions and answers.** Notes, the review note and block questions in the detail panel render
  the same Markdown subset as descriptions (headings, lists, code, emphasis, safe links; HTML stays text). Card
  tooltips show the plain text.
- **Skills write Markdown.** `/atm:create-tasks` writes task descriptions in Markdown when it helps; `/atm:do-task`
  and `/atm:loop-tasks` mention `atm edit --description-file` and Markdown in notes.
- **Phase filter with many phases.** With more than 6 phases, the filter panel's Phase group lists *All*, the active
  phases (unfinished, and the one finished last) and any selected phase; *+ N finished* / *Hide finished* shows the
  rest, saved with the view (`filterFinishedPhases`).
- **Labels on Kanban cards** stay on one line: the first two, cut at about 14 characters, then a *+N* chip whose
  popover lists every label (hover or focus; a click pins it without opening the task).

### Fixed
- **Activity popup:** the Idle number fits on one line, and its colour bar became a swatch of the charts' idle
  hatching, with a tooltip.
- **About dialog:** on All projects it no longer shows the last project's section; in a project it names the project,
  with its logo.
- **Kanban columns** reach down to the bottom of the window instead of stopping at 640 px; each scrolls on its own.

## [0.18.0] - 2026-10-26

**Project cards that say what's going on.** The layout reviewed in `docs/project-cards-proposal.md`.

### Added
- **Server summary** (`/api/projects`, `docs/multi-project-api.md` §3.2):
  - `working`: the live claims, oldest first, as `{task, name, agent, branch}`;
  - `next`: up to 2 tasks `atm next` would hand out, in its order (one shared `ready_in_order`);
  - `last_done`: the task completed last, `{id, name, completed_at}`;
  - `git`: `{branch, pr}`, the main worktree's branch (from `.git/HEAD`, no git process) and the latest PR recorded on
    the tasks, `open` or `merged`. No forge call, no CI state. `.git/HEAD` is part of the summary's ETag;
  - `description`: `meta.description`, trimmed and capped at 200 characters.

  Task names in the summary are capped at 80 characters.
- **Project cards** show the tasks at work by ID and name, with agent and branch on hover, or "Next:" with what the
  loop takes next. They also show the branch and the latest PR (linked), and the last finished task with how long ago.

### Changed
- **Project card layout:**
  - Order: head (logo, name, "needs you"), recent-phase rings, working/next, git, last finished with the 14-day
    sparkline.
  - Every section keeps its height, so the cards of a row line up.
  - The project-wide done / total moved into the rings' tooltip.
  - The current-phase line shows only for servers before 0.17.
  - `meta.description` shows on hover over the name.
  - Servers before 0.18 keep the agent count and the last activity time.

## [0.17.0] - 2026-10-26

**Overview polish, project identity, and parallel agents that don't trip over each other.** Phase 17 was built by
three agents at once in separate git worktrees; what that turned up is fixed here too.

### Added
- **Project logos.** `atm logo [set PATH | unset]` (`meta.logo`: an SVG, PNG, WebP or ICO file inside the project, at
  most 512 KB); without one, a `logo.svg`, `.github/logo.png` and similar are found by themselves. The server serves
  it at `/p/<id>/logo` and lists `logo` in `/api/projects`. The dashboard shows it on project cards, in the project
  selector and next to the title, or the project's initials on a colour from its id.
- **The ATM logo** as the browser tab icon (an inline `data:` favicon), in About, and with "Agentic Task Manager
  (atm)" at the start of the status bar.
- **Phase rings on project cards.** The server summary has `recent_phases` (the lowest 4 phases with open work, filled
  up with the most recently finished ones; phases of only skipped tasks left out); each card shows up to 4 rings on
  one line, the project-wide count in their tooltip.
- **"Needs you" opens the task.** The chip in the title bar and on project cards opens the one task waiting for you,
  or a list of them (`NeedsYouMenu`); the summary has `needs_you_tasks` (`[{id, name, reason}]`). An older server
  still gets the filtered Kanban.
- **Kanban completion ring** for the tasks shown, with "67% · 24 of 36 done · 2 skipped" beside it.
- **`atm run <id> [--every SECONDS] -- <command>`** runs a long command (a test suite, a build, a wait on CI) while
  keeping the claim on `<id>` alive: renewed at the start (refused before the command runs when it isn't yours), then
  three times per lease. Output and exit code pass through. The do-task, loop-tasks and push skills use it for long
  checks; the PostToolUse hook renews only between tool calls.
- `docs/project-cards-proposal.md`: what project cards should show, reviewed; phase 18 builds it.

### Changed
- **One title bar for every page** (`TitleBar.svelte`): the overview has the same controls, at the same place, as a
  project page. The status bar sits at the bottom of the window on All projects too.
- **"Active phases only"** keeps every unfinished phase and the one that finished last (a phase of only skipped tasks
  counts as finished), instead of those finished within N days; the days field is gone (an old saved `days` is
  ignored). It is **on by default**.
- **Project cards** are taller (more padding, 220 px at least), the name gets two lines on every card (longer names
  end in "…", the full name and folder on hover), and a long phase name stays on one line, so the cards of a row line
  up.
- **Agent identity per worktree:** an agent identified by its Claude Code session is `claude:<session>@<folder>` in a
  linked git worktree (subagents share their parent's session ID); the main worktree keeps `claude:<session>`. The
  hook computes the same from its `cwd`, so it renews only that agent's claims.
- **Blocked and in-review tasks are shared across worktrees:** the claims store records `waiting` tasks, so `atm next`
  doesn't hand them out in another worktree; `unblock`, `skip`, `done` and `reject` work from any worktree and are
  applied where the task waits with its next atm command (`resolved`). `atm status` lists them, and the server leaves
  a worktree's stale copy out of `worktrees.json`.
- The update notice is a toast at the bottom right, never under the title bar; it shows only once per new build, and
  not at all when the page already is that build (a same-version server restarts only for a rebuild in its own plugin
  folder, and whoami's `build` comes from the bytes it serves).

### Fixed
- Each checkout's e2e server gets its own port (from its folder), so tests in parallel worktrees don't test each
  other's build.
- Section screenshots wait for the title bar to settle after scrolling and leave the sticky status bar out, which
  made them flaky on a busy machine.

## [0.16.0] - 2026-10-26

**All of a project's git worktrees on one board, links to commits and PRs, and clearer progress.**

### Added
- **Git worktrees on the dashboard.** `/api/projects` lists each project's worktrees (`worktrees: [{path, branch,
  main}]`, main first, read from the git directory without running git). A new route, `/p/<id>/worktrees.json`, lists
  them with, for each linked worktree, the tasks whose copy there differs from the main worktree's. The dashboard
  shows all of them on one board: each task from the worktree where it got furthest (by status kind; the main
  worktree's copy on a tie), with a branch chip on its Kanban and list card and in its details. Tasks only a branch
  has are shown too. Format: `docs/multi-project-api.md` §2.2.
- `atm repository [set URL]` records the repository's web address in `meta.repository` (git remotes are turned into
  `https` addresses, without credentials or `.git`); `atm init` fills it from `origin`. `/atm:create-tasks` asks for it
  when it's missing.
- **Commit and PR links** in the task details and on the In PR and Merged chips, built from `meta.repository` for
  GitHub, GitLab (also nested groups), Bitbucket and Gitea/Forgejo/Codeberg; a PR's recorded URL wins. A self-hosted
  server is recognised by its name (`gitlab.*`, `github.*`, ...) or by `meta.repository_type`. Other hosts show plain
  text; without an address, a click explains `atm repository set`.
- Task details show Started, Sent for review, Finished (or Skipped) and Duration (work time, "so far" while active),
  besides Created and Updated: only the times a task has.
- Progress panel: **Active phases only** on the By phase card, sharing the Progress tab's saved setting.

### Changed
- **Completion leaves out skipped tasks**: done / (total − skipped), so 7 done and 3 skipped is 100%. The rings draw
  the skipped share as a thin muted arc outside the ring, screen readers hear the count, and the phase table says
  "· N skipped". Forecasts and insights count skipped tasks as neither done nor left.
- `claims.json` is served from the clone's shared claims store, so agents at work in any worktree show on the board.
- The Progress panel has one card for By phase and one for By type.
- The theme picker in the Appearance menu is a drop-down list (the menu is 136 px tall instead of 456 px).
- The Status filter lists every status in the project's vocabulary, also those no task has yet (showing 0).
- The task description in the details panel sits in its own box, with smaller headings.
- `atm done`, `release`, `fail`, `review`, `block` and `heartbeat` on a task whose lease of the same agent expired
  now say so, when it expired and what to run next (`atm start <id>`, then the command again), with exit code 4. The
  expiry is saved before the error. An expired lease and `atm release` keep the task's commit; `fail`, `reject` and
  `reopen` still drop it.

### Fixed
- Opening another task scrolls the details panel back to the top; an auto-refresh of the same task keeps the scroll.

## [0.15.0] - 2026-10-25

**One dashboard for all your projects.** The dashboard is no longer copied into each project and served per project:
one server per user serves it, straight from the installed plugin, for every project atm runs in.

### Added
- **Project registry** in `~/.agentic-task-manager/projects.json` (`ATM_HOME` overrides the folder). `atm init`,
  `atm migrate` and `atm serve` add the project they run in, and the PostToolUse hook adds it on the first tool call
  (one small file read per call; it never waits for the registry's lock). A linked git worktree adds its main
  worktree. `atm projects` lists them; `atm projects add [PATH]`, `remove <id|PATH>` and `prune` manage them. A
  project opts out with `meta.dashboard.listed: false`. An unreadable registry is moved aside and started again.
- **`atm serve`, one server per user** (`ATM_HOME/server.json`), on port 8765 or the next free one. It holds the
  dashboard in memory and serves `/api/projects` (each project with a summary: done / (total − skipped), current
  phase, agents working, what needs you, last activity, completions over 14 days; computed by atm-cli's own logic,
  cached until a project's files change) and each registered project's `tasks.json`, `claims.json` and
  `tasks-archive.json` under `/p/<id>/` (ETag and 304), nothing else. It binds to 127.0.0.1 and refuses requests whose
  Host isn't `localhost`/`127.0.0.1` on its port (DNS rebinding). `atm serve` prints the project's URL
  (`#project=<id>`), `--all` the overview's. Routes and formats: `docs/multi-project-api.md`.
- **`atm serve --refresh`** restarts a running server whose dashboard is older than the plugin's (never a newer one),
  on the same port. A SessionStart hook runs it, and every atm command (so the PostToolUse hook too) makes the same
  cheap check, so the server follows a plugin update by itself.
- **Dashboard: project selector** next to the title: All projects, then each project with a progress bar and a dot
  when something needs you; type to filter, arrows, Enter, Escape. A badge counts what needs you in the other
  projects.
- **Dashboard: All projects overview**, a card per project: progress (skipped tasks shown apart), current phase,
  agents at work, what needs you (opens that project's Kanban filtered to review and blocked), last activity and a
  14-day sparkline. Projects that need you come first; **Active only** hides projects with nothing open and nothing
  done in 14 days. The dashboard opens on it with two or more projects, on the project with one, and explains how to
  add a project with none. `#project=<id>` and `#overview` links; an unknown project has its own page.
- **Dashboard: "A new version of the dashboard is ready"**, with Reload, when the server restarted on a newer build.
- `atm phases` lists the phases with their names and task counts by kind; `atm phases name <n> "<name>"` and
  `atm phases unname <n>` change one name at a time. `atm status` shows the names; `atm check` warns about a named
  phase without tasks. `/atm:create-tasks` names phases this way instead of rewriting `meta.phases`.
- `atm skip` records `skipped_at`; `atm migrate` fills in `completed_at` and `skipped_at` from `updated_at` for
  finished tasks without them (in the archive too), so a later edit doesn't move them in time.

### Changed
- `atm migrate` no longer copies the dashboard into the project. It deletes a project's old copy, stops its old
  per-project server (only when that server confirms it serves that folder) and removes `.server.json`/`.server.log`.
- The dashboard loads everything from the server's API (`api/projects`, `p/<id>/…`); opening the HTML file another
  way is no longer supported. The dev server and the e2e fixture play the same API.
- Saved views are kept under the project id the server gives (`meta.id`, so existing views are found again).
- Archiving dates a skipped task by `skipped_at`.

### Removed
- `/atm:update-dashboard`: updating the plugin updates the dashboard, and What's new is in the dashboard.

## [0.14.0] - 2026-10-25

### Added
- **Five more themes**, each in light and dark and checked for readable contrast: Ayu, Night Owl, Gruvbox, Noctis
  (with Noctis Lux) and Sublime Material.
- **Appearance menu** for 11 themes: it opens on the current theme, the arrow keys and Home/End move through them, and
  the list scrolls on small windows.
- **Progress tab: "Active phases only"**, which hides phases finished more than N days ago (7 by default, editable;
  remembered per project). The burndown still covers every phase.
- **Markdown in task descriptions** in the details panel (headings, lists, code, links).
- **What's new for users**: the dashboard's What's new and `/atm:update-dashboard` show `WHATS-NEW.md`, a short list
  per release; this changelog stays the detailed record.
- `atm skip <id> --reason TEXT`: a to-do or blocked task that isn't needed becomes skipped, with the reason as a note.
- `atm reopen <id> --reason TEXT`: a done or skipped task goes back to to-do (done by mistake, or not finished).

### Changed
- The Progress panel's rings (by phase, by type) are separate cards.
- `/atm:do-task` and `/atm:loop-tasks` commit a task's work while it is still in progress, and only then mark it done.

### Fixed
- `atm commit` accepts files removed with `git rm` (they were refused as unknown).
- Activity titles name the period you picked ("this week, per day"); choosing Week showed "per 6 hours".

## [0.13.0] - 2026-10-24

**Update the plugin for every agent before your first `atm archive`:** older versions see a dependency on an
archived task as missing, so its dependents never become ready.

### Added
- **atm-cli** (the `atm` command) does everything the skills need; they never touch `tasks.json` directly. New:
  `status`, `show`, `list`, `deps`, `add`, `edit`, `check`, `note`, `review`/`reject`, `block`/`unblock`, `commit`,
  `workflow`, `pr`, `merged`, `archive`/`unarchive`, all with `--json`. It is on Claude Code's PATH, so one rule,
  `Bash(atm *)`, approves it across upgrades.
- **Priorities** (`P0`–`P3`): agents take the most urgent first within a phase; cards, filters and List sort show
  them.
- **Labels** such as `spike-2` or `ux`: chips on cards, a filter, and search.
- **Your own vocabularies**: statuses (each of a kind), phases, types, priorities and labels, confirmed by
  `/atm:create-tasks` and listed in **About**.
- **In Review**: `atm review` hands work over for approval; `atm done` passes it, `atm reject` sends it back. Cards
  say how long it has waited.
- **Blocked on a decision**: agents ask instead of guessing (`atm block --on user|agent --question`); cards say
  "Waiting on you" or "on an agent", the details list the questions, `atm unblock --answer` replies. The header counts
  what needs you.
- **Notes** (handoffs, outcomes, reviews, rejections, answers, failures) and **who** created, started and completed a
  task, in its details; the Agents tab marks people, agents and approvals; a Started by filter.
- **`/atm:push`** for both workflows, direct or pull requests: pushes, opens the PR, watches CI and fixes failures
  without weakening tests or raising budgets. Your `CLAUDE.md` comes first.
- **Stages of Done**: In PR (linked) and Merged chips; in pull-request projects Done is split by stage. A Stage filter.
- **The archive**: `atm archive` moves old finished tasks to `tasks-archive.json`; atm suggests it past 1 MB or 500
  archivable tasks. The dashboard loads it only for Progress, activity details and **Include archived**.
- **Help** and **About** in the footer.
- A plugin hook renews claims while an agent works.

### Changed
- Unchanged `tasks.json` polls are a `304 Not Modified` (exact ETags, even for two changes in one second).
- The skills commit with `atm commit`: exactly the files named, plus `tasks.json`.
- `started_by` replaces `agent` (still read); `atm migrate` also updates a rebuilt dashboard of the same version.

## [0.12.0] - 2026-10-24

### Added
- Light and dark mode: the dashboard follows your system setting, or choose System, Light or Dark in the top right.
  Your choice is remembered per browser and applied before the page appears, so there's no flash of the other mode.
- Themes: Default, Nord, Solarized, Catppuccin (Latte/Mocha), Tokyo Night (Day/Night) and High contrast, each with a
  light and a dark variant. All colours meet contrast targets for readable text (see docs/theme-validation.md in the
  repository).
- Filter buttons show how many tasks each would show, given the other filters; values with no tasks are dimmed.
- A new top bar that stays in view while you scroll: a progress ring and an activity sparkline next to the title.
  Click the ring for completion overall, by phase and by type. Click the sparkline for activity: tasks completed by
  type, how many agents were working, time per task, and idle stretches when no agent worked, for a period you pick
  (hour, day, week, month, all time or custom; ‹ › step back to yesterday, last week, last month, …). Reload,
  auto-refresh, mode and theme are compact menus in the top right.
- Keyword search over task ID, name, description and type. The filters fold into one line under the title that shows
  only what's selected, with the number of matching tasks; **Filters** opens Phase, Type and Status.
- **Agents** tab: a timeline with a lane per agent and a bar per task, from start to completion, coloured by type;
  work in progress runs up to now. Zoom with the buttons, Ctrl/⌘ + scroll or the "last hour" presets, drag to move
  in time, or pick a range on the overview below the lanes. **Live** keeps it at the current time. Tasks outside the
  search and filters are greyed out.
- Agent utilisation on the Progress tab: agent work time against calendar time (24 hours a day) and against working
  time (8 hours a day, Monday to Friday), overall and per agent.
- `atm start` (and `atm next --claim`) record which agent took a task in its new `agent` field. It stays when the
  task is done and is removed if the task goes back to todo. Tasks finished earlier show as "Unattributed".
- **Task List** tab: the task cards, which used to sit below the views. The tabs have icons and sit on the view,
  which fills the rest of the window (the dependency graph uses all of it).
- The dependency graph can hide tasks outside the search and filters (the default) or show them greyed out.
- The dashboard remembers your view per project: filters and search, the selected tab, the options in each tab
  (Kanban, Progress focus, graph hide/grey out, Agents Live, the Activity period), and the zoom of the graph and the
  Agents timeline. **Reset view** forgets it.
- `/atm:create-tasks` gives new task files a `meta.id`, and `atm init` adds one to existing files, so dashboards of
  different projects on the same port keep separate views.

### Changed
- Kanban "Group by phase" is now remembered per project instead of per browser (your previous choice is kept as the
  starting point).
- The count boxes and the small completion chart above the filters are replaced by the top-bar widgets, and the
  views panel no longer collapses.

### Fixed
- Phase badges have their tinted background again (lost in 0.5.0).

## [0.11.0] - 2026-10-23

### Added
- Several agents can now work on one project at once. Run `/atm:loop-tasks` in more than one session, ideally each
  in its own git worktree: they never take the same task or overwrite each other's changes to `tasks.json`. See
  "Running agents in parallel" in the README.
- Claims: the skills take each task with `atm next --claim`, which records the agent, branch and worktree. All
  worktrees of a clone share the claims. A claim lasts 30 minutes and is renewed during long tasks; if an agent stops,
  its task becomes available again when the claim runs out.
- The dashboard shows who works on each in-progress task, on which branch and since when (cards, Kanban board and
  task details).
- `tasks.json` merges task by task: status changes from different branches combine, and tasks added on both
  branches are both kept. A git merge driver for this is registered automatically.

### Changed
- `/atm:do-task` and `/atm:loop-tasks` change task status only through the plugin's helper, which locks
  `tasks.json` and writes it atomically, so the dashboard never reads a half-written file.
- The skills commit only the files a task changed plus `tasks.json`, instead of everything in the working tree.
- A failed task records why in `last_failure`.

### Fixed
- `/atm:serve stop` no longer waits several seconds in containers (dev containers, CI images) before reporting the
  server stopped.

## [0.10.0] - 2026-10-23

### Added
- `/atm:serve` starts the dashboard in a background server (`--open` opens the browser, `stop` and `status` manage
  it). It uses port 8000 or the next free one, keeps running after the session ends, and only ever stops the server
  it started for this project.
- Everything atm needs now lives in one folder, `.agentic-task-manager/`, at the root of your project.

### Changed
- Projects that keep `tasks.json` in `docs/` or the project root are moved into `.agentic-task-manager/` the first
  time a skill runs, with `git mv` so history is kept. The move is staged for you to commit; `/atm:loop-tasks`
  commits it on its own before its first task.
- The dashboard HTML is no longer committed to your repository. It's a copy of the plugin's dashboard, listed in
  `.agentic-task-manager/.gitignore`, and the skills bring it up to date automatically when they run.
- The skills use a small helper script that needs **Python 3.9 or newer** (standard library only, nothing to
  install). On macOS it comes with the Command Line Tools, like `git`.
- Updating the plugin: run `/plugin marketplace update hakanserce-plugins`, then `/reload-plugins`. No restart.

## [0.9.0] - 2026-10-23

### Added
- **Progress** tab: a burndown chart of remaining work and total scope over time, with a forecast to the estimated
  finish. Click a phase to see only that phase.
- Per-phase table with progress, remaining work and a completion estimate, e.g. "≈ 1 h 10 m of agent work
  (50 m – 1 h 40 m)", plus the clock time if the loop keeps running. Estimates use the median time per task over
  the last 10 completed tasks and follow the loop's phase order; blocked work is flagged, not estimated.
- Estimates use `started_at`/`completed_at` when present (written by the skills since 0.7.0). Older task files
  still work: the time between completions is used instead, with idle gaps capped at 30 minutes.

## [0.8.0] - 2026-10-23

### Added
- Kanban board: the dependency graph now shares its panel with a **Kanban** tab (and a Progress tab, coming next).
  The selected tab is part of the URL, so a reload or a shared link opens the same view.
- Kanban columns for Todo, In Progress, Blocked and Done (Skipped on request). Todo cards say whether they are
  ready or which tasks they are waiting on, and **Next up** marks the task `/atm:loop-tasks` will pick next.
- Optional "Group by phase" swimlanes on the Kanban board, remembered per browser.

### Changed
- The graph keeps its zoom and position while you look at another tab.

## [0.7.0] - 2026-10-22

### Added
- Release notes in the dashboard: the version next to the title opens a "What's new" panel, and shows a "new" marker
  after an upgrade until you've opened it.

### Changed
- Auto-refresh is now on by default (every 5 seconds), and the dashboard remembers whether you switched it off and
  which interval you chose.
- The dependency graph keeps its zoom level when filters change or a reload changes the visible tasks. It only pans
  when no task would be left on screen.
- `/atm:do-task` and `/atm:loop-tasks` record when each task started (`started_at`) and was completed
  (`completed_at`). The activity timeline uses the completion time.

### Fixed
- `/atm:update-dashboard` looked for the new dashboard one directory too high inside the installed plugin, and now
  prints the actual release notes between your old and new version instead of a summary it made up.

## [0.6.0] - 2026-10-22

### Added
- Dependency graph zoom and pan: drag to pan, Ctrl/⌘ + scroll or pinch to zoom, zoom/fit/1:1 buttons, keyboard
  shortcuts when the graph has focus, and a clickable minimap when the graph doesn't fit.
- Large graphs open at a readable scale, centered on the task in progress; small graphs open at 100% instead of
  being stretched to the page width.

## [0.5.0] - 2026-10-22

### Changed
- The dashboard was rebuilt (Svelte 5) with an automated test suite. It looks and behaves the same as before.
- Filter buttons and the browser tab title now update when tasks.json is reloaded.

### Fixed
- Task names and descriptions are shown as plain text. Previously, HTML in tasks.json was rendered as markup.
- Installation instructions in the README use the correct plugin name (`atm@hakanserce-plugins`).

## [0.4.0] - 2026-04-05

### Added
- Task timestamps: `created_at` and `updated_at`.
- Activity timeline on the dashboard showing when tasks were completed.
- `/atm:update-dashboard` to copy the latest dashboard into a project.

### Changed
- `/atm:loop-tasks` commits after each task by default (`--no-commit` to opt out).

## [0.3.0] - 2026-04-04

### Changed
- The plugin lives in its own `plugin/` directory, separate from the marketplace configuration.

## [0.2.0] - 2026-04-04

### Changed
- The plugin is now called `atm`, so skills are `/atm:create-tasks`, `/atm:do-task`, `/atm:loop-tasks`.
- Repository restructured to follow Claude Code plugin conventions; installable from the `hakanserce-plugins`
  marketplace.

## [0.1.0] - 2026-04-03

### Added
- Initial release: skills to decompose requirements into `tasks.json`, execute one task, and loop through all tasks,
  plus a self-contained HTML dashboard with stats, filters, task cards and a dependency graph.
