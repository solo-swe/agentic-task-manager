# Porting atm to Rust

Status: **agreed** with Hakan on 2026-11-06 (T-415). The Python spec it ports is frozen: the command tree of
[improving-atm-cli-ergonomics.md](improving-atm-cli-ergonomics.md) (T-425), the task states and guards of
[task-states.md](task-states.md) (T-438), the layers of [layered-architecture.md](layered-architecture.md). How the
result reaches users is [better-way-to-distribute-atm.md](better-way-to-distribute-atm.md) (T-426); how it is written,
the [Rust guide](guides/rust.md).

atm becomes **one Rust binary**, `atm`: every command, `atm serve` (the dashboard for browsers), the git merge driver,
`atm agents` and `atm docs`. Nothing to install alongside it (no Python), one file per platform, the task states as
Rust enums, the layer rule checked by the compiler. The atm app (Tauri) links the same library.

## 1. What stays, and what may change

- **The command structure stays** as in the ergonomics doc: the same groups, commands, arguments and flags. Where clap
  offers a better way (a flag's form, a value parser, how a group is entered), the change is proposed to Hakan first
  and made only with his approval.
- **`--json` output is identical**, field for field: it is what agents and other tools read.
- **Text output is the same or better.** clap's help, usage lines and argument errors may replace Click's where they
  read better; atm's own messages and hints stay as they are. A scenario whose text check changes says so in the
  change that changes it.
- **Exit codes and files are identical:** 0, 1, 2 (usage), 3 (nothing to do), 4 (claimed), 127 (missing runtime, which
  goes away); `tasks.json`, `tasks-archive.json`, the claims store, `projects.json`, the server record, the merge
  driver's results.
- **`tests/scripts/` is the spec.** Its scenarios and `*.test.mjs` run against the Rust binary as they run against
  Python (§5).

## 2. The repository after the port

```
Cargo.toml              the workspace: edition, lints, [workspace.dependencies]
rust-toolchain.toml     the pinned toolchain (the minimum Rust version is this one)
deny.toml               cargo-deny: licences, advisories, bans
crates/
  atm-common/           model (task, sprint, release, …), vocabularies, errors, the JSON schema
  atm-db/               the store interface; jsongit (files, lock, merge, migrations, claims store); memory
  atm-vcs/              the version-control interface; git (running git, reading HEAD)
  atm-lib/              the domain: tasks and their states, claims, the loop, sprints, releases, components,
                        archive, settings, projects, documents, agents (installing the skills)
  atm-boot/             opening a project or the registry: picks the store and the vcs, the actor
  atm-api/              the REST routes `atm serve` serves (axum), the page and its assets, read-only
  atm-cli/              the `atm` binary: clap, renderers (text, --json), exit codes, `serve`, `docs`
app/                    the Tauri app: src-tauri/ (its commands map to atm-lib), the window on dashboard/
dashboard/              the Svelte UI, built two ways: one file for `atm serve`, a bundle for the app
skills/                 the agent skills, the one copy (portable core); the binary embeds them
docs/
  agent/                `atm docs` topics, embedded in the binary
packaging/              the Homebrew formula and cask, the release script, the public skills repo's export
tests/
  scripts/              the CLI's scenarios and tests, run against the binary
  sandbox/              the agent sandbox
atm-cli/                the Python atm, until the switch (§5), then removed
```

- **One module per top-level command in `atm-cli`.** `src/commands/<group>.rs` for each group (`task`, `loop`,
  `claim`, `vcs`, `db`, `release`, `component`, `project`, `sprint`, `agents`), and `maintenance.rs` (`doctor`,
  `service`) and `meta.rs` (`version`, `help`, `docs`) for the single commands. Each holds its clap subcommand enum
  and the handlers, with its area's text renderers beside them. `main.rs` holds only the root `Cli`, the global
  options, the dispatch and the exit codes. The Python CLI is split the same way (`atm-cli/atm/cli/commands/`).
- **One crate per layer.** A crate can use only what its `Cargo.toml` declares, so the layer rule
  ([layered-architecture.md](layered-architecture.md) §2) is the compiler's: `atm-api` and `atm-cli` depend on
  `atm-lib` and `atm-boot`, never on each other; `atm-lib` never on `atm-db`'s jsongit module (it takes the store
  interface); only `atm-db` and `atm-vcs` run git. `build.py`'s import check retires with Python.
- **Nothing built is committed.** `plugin/scripts/atm.py` and `plugin/templates/tasks-dashboard.html` were committed
  because the plugin shipped them; the binary is built in CI and released. The dashboard is built before the binary,
  which embeds it, with CompMD's runtime (`plugin/assets/compmd/` today) and the skills.
- **`plugin/` goes** at the switch: the app and the CLI are the only way to install atm (§6). Its hooks' job (renewing
  claims) moves into atm (§4).
- **The skills' one copy** is `skills/<name>/SKILL.md` (`name`, `description`, Markdown, `references/`). The binary
  embeds it, so `atm agents install` writes the skills of the version that runs; Claude Code's copy gets its
  pre-approved commands (`allowed-tools`) added as it is written. The public skills repo (`npx skills add`) is
  generated from `skills/` at each release, tagged with atm's version.

## 3. Choices inside

- **CLI:** `clap` with the derive API, subcommands as enums, shell completion from `clap_complete` with atm's own value
  completers (task IDs, sprints, releases, …).
- **`atm serve`:** `axum` on `tokio`, in the `atm` binary (`atm serve` starts it detached, as today; no second
  binary). The routes, Host check, CSP and headers are those of [multi-project-api.md](multi-project-api.md) and
  [security-architecture.md](security-architecture.md) I12–I13. The rest of atm stays synchronous (the Rust guide).
- **The task states:** an enum per kind and a transition table checked with `match`, the guards (review decisions,
  dependencies, resolutions) as functions in atm-lib; property tests against [task-states.md](task-states.md).
- **JSON:** `serde`, with the field order and formatting of today's files (2-space indent, keys in atm's order), so a
  file written by Rust diffs cleanly against one written by Python and merges with the same driver.
- **The lock:** `std::fs::File::lock` (an advisory lock on every platform) on the same lock file Python uses, so
  both can run against one project during the switch.
- **`atm claim hold`:** the watcher is the `atm` binary started detached (`atm _claim-watch …`), not a fork; it follows
  the calling shell by its process ID and start time (`sysinfo`), on every platform. Invariant I24 (atm runs no
  command it is given) and its test move with it: atm starts only git, itself (the watcher, `serve`) and the
  browser opener.
- **Errors:** `thiserror` enums in the library crates, `anyhow` at the binary's top; messages as Python's.
- **New dependencies** beyond those named here are proposed first (the Rust guide).

## 4. Claims without the plugin's hooks

The plugin's hook renewed an agent's claims after every tool call. Without the plugin, and in every harness, atm
keeps claims alive itself:

- **Every atm command an agent runs renews that agent's claims** (as `atm claim heartbeat` does), under the lock it
  already takes.
- **The default lease is longer** where nothing renews it between commands: 2 hours, unless `meta` sets it. A claim
  left by an agent that stopped is released by the next `atm loop next` after it expires, as today.
- **`atm claim hold <id>`** stays for long commands (a test suite, a build, waiting on CI): its watcher renews while
  the shell that ran it lives.

This applies before the switch too: the Python atm gets the renewal and the longer lease first, and the plugin's hook
becomes redundant (§6).

## 5. How the port runs

- **Side by side, one suite.** The Rust workspace is built next to `atm-cli/`; `tests/scripts/` runs against either
  (`ATM_IMPL=rust` picks the Rust binary), and CI runs both on macOS, Linux and Windows (Python on its current
  matrix). A scenario that Rust doesn't pass yet is listed, by file, in one expected-failures list that only shrinks.
- **Slices, not layers** ([layered-architecture.md](layered-architecture.md) §9): each step ports one area through
  every crate it needs, with its scenarios. The order:
  1. the workspace skeleton: crates, lints, cargo-deny, CI on three platforms, `atm version`, the suite's
     `ATM_IMPL` switch;
  2. reading: the store (`tasks.json`, the archive), `task list|details|deps`, `project status`, `--json`;
  3. the task states: `task state`, `task review`, the guards, notes, `loop next`;
  4. claims: the claims store, `claim list|heartbeat|hold`, the renewal of §4;
  5. defining work: `task add|edit|note`, `sprint …`, `release …`, `component …`, `project …`;
  6. git: `vcs commit|pr|pushed|repository`, the merge driver, `db migrate|init|archive|unarchive`, `doctor`;
  7. `atm serve`: axum, the routes, the embedded dashboard and assets, `service`;
  8. `atm agents` and `atm docs` (ported from their Python versions, distribution rollout steps 1–2);
  9. completion, help text, and the review of clap's changes to the text (§1).
- **The switch** happens when Rust passes every scenario on all three platforms and the release machinery exists
  (signed builds, the formula, release binaries): the Python atm, `plugin/` and `build.py` are removed in one change,
  and the Homebrew formula becomes how the CLI is installed.

## 6. What users install, and the plugin

The Claude Code plugin is not kept after the switch: the app and the CLI install atm, and `atm agents install` puts
the skills where each harness reads them ([better-way-to-distribute-atm.md](better-way-to-distribute-atm.md)). What
the plugin did that a skill can't, and where it goes:

| The plugin's part | After the switch |
|---|---|
| The CLI and the dashboard | The `atm` binary (formula, release binaries) and the app |
| Hooks: claim heartbeats | atm renews claims itself (§4) |
| Hooks: `atm service --refresh` after an update | The app and `atm serve` restart on a new version themselves |
| The `/atm:` namespace | The skills are named `atm-…` (`/atm-push`, `/atm-do-task`, …), so they don't clash with other skills |
| Pre-approved commands (`allowed-tools`) | Written into Claude Code's copy by `atm agents install` |
| Updates through `/plugin` | The app's updates and `brew upgrade`; the linked skills update with them |

## 7. atm-lib's public surface

The Python `atm.lib` and `atm.boot` (frozen in `tests/lib/public-surface.json`, T-384) were groundwork for tools that
would use atm's logic in-process (an MCP server, scripts); none exists. They go with `atm-cli/`. After the port, other
tools use `atm … --json`, which is stable and versioned with atm; the `atm-lib` crate is used by the CLI and the app
only, and is published (with a frozen API) when a real user needs it.

## 8. Risks

| Risk | Mitigation |
|---|---|
| A behaviour the suite doesn't check changes unnoticed | The suite is the spec; a bug found after the switch gets a scenario first |
| Windows: paths, line endings, file locks, process checks | CI on Windows from the first step; the lock and the watcher on cross-platform APIs |
| The merge driver differs from Python's | Property tests on merges; both implementations merge the same fixtures during the port |
| Users between the plugin and the formula | The switch waits for the formula and release binaries; the last plugin release says how to move |
| The binary can't be read like `atm.py` | The source is in the repository and each release names its commit; builds are reproducible from it |
