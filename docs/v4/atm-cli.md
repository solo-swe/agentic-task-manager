# v4 atm-cli: structure and commands

Part of the v4 design ([overview](README.md)). The fields and files the commands work on are defined in the
[data model](data-model.md).

**atm-cli** is the `atm` command the plugin ships: `plugin/scripts/atm`, a launcher for `atm.py`. Earlier documents
call it "the helper".

## 1. Why

An agent working another project's tasks with `/atm:do-task` (0.12.0) kept working around missing atm-cli commands:

- **Reading tasks:** it parsed `tasks.json` with Python, jq or partial Reads. The file is too big to read whole, and
  each workaround can set off a permission prompt.
- **Changing tasks:** it edited `tasks.json` by hand, outside atm-cli's lock, racing other agents.
- **State that lived only in chat:** a task waiting on the user's decisions, and handoff notes.

Part of the cause is in the skills: `/atm:do-task` says "Read and parse the file", and `/atm:create-tasks` writes the
JSON by hand. Meanwhile `atm.py` is 1,299 lines in one file, and about fifteen commands are about to be added.

## 2. Structure

Two constraints stay: **one file in the plugin** (users install by cloning, nothing is built on their side) and
**readable** (people read what a plugin runs; Claude reads atm-cli when a skill's call fails).

**Modules for development, concatenated into one readable file.** The source lives in `atm-cli/atm/` at the repo root
(dev only, not shipped); `atm-cli/build.py` (standard library) writes `plugin/scripts/atm.py`:
- a "generated from atm-cli/atm/ by atm-cli/build.py — don't edit" header;
- standard-library imports, deduplicated and hoisted;
- each module in dependency order under a banner (`# ==== lock.py ====`), its intra-package imports removed;
- an `ast` check that no top-level name is defined in two modules.

```
atm-cli/
  build.py
  atm/
    core.py       # constants, errors, output (text and --json), git(), time helpers, atomic writes
    paths.py      # project root, task folder, legacy locations
    model.py      # loading and validating tasks.json and the archive, vocabularies, kinds, next_id
    lock.py       # the lock directory
    tasks.py      # status changes, notes
    query.py      # show, list, status, deps
    edit.py       # add, edit, check
    archive.py    # archive, unarchive, the size hint
    claims.py     # claims store, leases, next
    git_ops.py    # commit, pr, merged
    merge.py      # the merge driver, init
    migrate.py    # migrate, the dashboard copy and build ID
    server.py     # serve
    cli.py        # argparse commands, main()
```

**Options considered:** Python's `zipapp` (one file, but a zip archive nobody can read); shipping the package folder
(no build, but about 15 files in the plugin).

**argparse, not click.** click isn't in the standard library: users would have to `pip install` it (a second
requirement, refused by PEP 668 system Pythons), or we'd vendor about 10,000 lines (8× atm-cli; click 8.2+ also
needs Python 3.10). argparse gives subcommands, per-command help, typed and required arguments and choices, declared
together in `cli.py`.

| ID | P | Requirement |
|---|---|---|
| CLI-2.1 | P1 | `npm run build:cli` (part of `build` and `verify`) regenerates `plugin/scripts/atm.py`; the pre-commit hook rebuilds and stages it when `atm-cli/` changes; CI fails if it is stale. The build is deterministic. |
| CLI-2.2 | P1 | `tests/scripts/` keeps testing the built file through the launcher. The move is done one module at a time, each step passing the existing tests unchanged. |
| CLI-2.3 | P1 | Python 3.9+, standard library only, tested with 3.9 and a current version. |

## 3. Conventions for every command

| ID | P | Requirement |
|---|---|---|
| CLI-3.1 | P1 | Every command takes `--json`. Errors with `--json` are `{"error": "…", "code": N}` on stderr, with the same exit code as in text mode. |
| CLI-3.2 | P1 | Existing commands keep their output and exit codes; new lines are only added after the existing ones. argparse words some usage errors differently, and atm-cli tests checking them are updated. |
| CLI-3.3 | P1 | Text output is short and stable, meant for agents to read; anything that parses uses `--json`. |
| CLI-3.4 | P1 | Every change runs under the lock and writes atomically. Status and timestamps change only through the status commands. |
| CLI-3.5 | P1 | Every change records its actor: `updated_by` always, and `created_by`, `started_by`, `completed_by` as in DM-4.4. The actor is `--agent`, else `ATM_AGENT`, else `claude:<session>`, else the git user name (DM-4.5). In a linked git worktree the session form adds the worktree's folder, `claude:<session>@<folder>`: subagents share their parent's session ID, so agents spawned in separate worktrees would otherwise be one actor and could finish or release each other's tasks (T-202). The main worktree keeps the plain form. |

## 4. Commands

Existing commands are marked *(exists)*.

### 4.1 Reading

| Command | Prints |
|---|---|
| `atm show <id>... ` | tasks in full, falling back to the archive |
| `atm list [--status S] [--type T] [--priority P] [--label L]... [--agent A] [--ready] [--blocked] [--archived]` | one line per task: ID, status, priority, labels, name |
| `atm status` | counts by status, sprint (with sprints), type and label; in progress and who claims it; the next available task; blocked tasks and what they wait for; in review; the files' sizes, the archive count and the largest tasks; a warning for open tasks left in closed sprints, which the loop's default selection doesn't take (`open_in_closed_sprints` in `--json`, only when there are any; T-330) |
| `atm deps <id>` | what the task waits for (with each one's status) and what waits for it |
| `atm summary` | the project's summary as `atm serve` lists it in `api/projects` ([API](../multi-project-api.md) §3.2), JSON either way; the dashboard's dev server plays `atm serve` with it, and the e2e fixture with `withAtmSummary()`, so the Kanban's Next up is atm's own answer everywhere (T-321) |
| `atm vocab` | the effective vocabularies (P2) |
| `atm next [--claim]` *(exists)* | ordered by priority rank, then ID (natural sort); phases no longer count (T-320) |
| `atm next [--sprint S]... \| --tasks ID,ID \| --all \| --backlog` | a **selection** ([sprints design](../sprints-and-releases-design.md) §3.2): by default the active sprint in a project with sprints (none active: nothing, and a reason that says to plan one or use `--all`), every task in a project without; `--tasks` in the order given, `--all` the active sprint, then planned ones in order, then the backlog. The reason for nothing ready says whether the selection is finished, waiting (and on what) or empty (SR-3.4, SR-3.6) |

### 4.2 Status changes

| Command | From → to | Also |
|---|---|---|
| `atm start <id>` *(exists)* | todo → active | now prints the task, as `next --claim` does |
| `atm done <id> [--note T] [--commit SHA] [--resolution R]` *(exists)* | active, review → done | prints the tasks it unblocked; `--resolution` (`done`, `wont_do`, `duplicate`, `obsolete`) is stored only when it isn't `done` (SR-2.8, T-318) |
| `atm skip <id> --reason T [--resolution wont_do\|obsolete\|duplicate] [--duplicate-of ID]` | todo, blocked → skipped | the reason is a note; the resolution is stored only when it isn't `wont_do`; `--duplicate-of` needs `duplicate` and a task in either file. `reopen` clears both. `atm show` prints the resolution, `related` on both sides and "duplicated by"; neither affects readiness, and the merge driver merges `related` as a set |
| `atm fail <id> [--reason T]` *(exists)* | active → todo | the reason becomes a `failure` note |
| `atm release <id> [--note T]` *(exists)* | active → todo | the note is a `handoff` |
| `atm review <id> [--note T] [--status S]` | active → review | records `review_requested_at`, releases the claim |
| `atm reject <id> --reason T` | review → todo | a `rejection` note; clears `review_requested_at` |
| `atm block <id> --on user\|agent (--question T)... [--reason T]` | active, todo → blocked | records `waiting_on`, `questions`, `blocked_at`; releases the claim. `--on agent` hands the decision to another agent (e.g. a reviewer agent), which answers with `unblock` |
| `atm unblock <id> [--answer T]...` | blocked → todo | the questions and answers become an `answer` note |
| `atm reopen <id> --reason T` | done, skipped → todo | for a task marked done by mistake, or not finished after all: clears the attempt (as `fail` does) and keeps the reason as a note (added with T-153); in a closed sprint, a hint suggests `atm move <id> --to backlog` or `--to active` (`hint` in `--json`), moving nothing (T-330) |
| `atm note <id> T` | — | adds a `note` |
| `atm heartbeat <id>` *(exists)* | — | extends the claim |

**An expired lease** (T-184): every command first expires claims whose lease ran out; their tasks go back to todo with
a `failure` note, and keep their `commit` (the work is in git). Going back to todo drops the commit only for `fail`,
`reject` and `reopen`, which undo the work. When `done`, `release`, `fail`, `review`, `block` or `heartbeat` finds
that the expired claim was the agent's **own**, it writes the expiry, then stops with exit code 4 and says what
happened and what to do: *"T-171: your lease expired at 23:00 UTC (no heartbeat for 30 min), so it went back to todo.
Your commit is kept. Run `atm start T-171`, then `atm done T-171` again."* Another agent's expired claim gives the
usual refusal.

### 4.3 Editing tasks

| Command | Does |
|---|---|
| `atm add --name … --type T [--priority P] [--label L]... [--description T \| --description-file F] [--dep ID]... [--file F]...` | adds one task with the next ID; prints it |
| `atm add --json-file F` | adds many (what `/atm:create-tasks` uses); IDs in the file are placeholders (`new-1`, …) that dependencies can refer to, and are replaced by real IDs |
| `atm edit <id> [--name] [--description \| --description-file] [--add-file/--remove-file] [--add-dep/--remove-dep] [--type] [--priority] [--add-label/--remove-label]` | changes a task; refuses a dependency that is missing or would make a cycle |
| `atm check [--files]` | the schema, `meta` (DM-2.2), missing and circular dependencies, IDs in both files; unknown types and priorities as warnings, and open tasks in closed sprints (T-330); with `--files`, files listed by finished tasks that don't exist as warnings (off by default: files get renamed and deleted later, and to-do tasks list files they will create); exit code 1 on errors |
| `atm sprints [--all]`, `atm sprints add ["<name>"] [--goal T] [--release R] [--due DATE] [--after S]`, `atm sprints edit <S> [--goal] [--release] [--due]`, `atm sprints start <S>`, `atm sprints close <S> [--carry-to <S2> \| new [--name N] \| backlog]` | the sprints in order with their task counts by kind and the backlog's size (closed ones with `--all`); add a planned sprint (named by `meta.sprint_names` when no name is given: a historical person, or the creation time), change one (an empty value removes a field), make one active, one at a time, or close one ([sprints design](../sprints-and-releases-design.md) §3.1, SR-2.9). Closing refuses while a task in the sprint is in progress; its unfinished tasks go to `--carry-to` (required when there are any; `new` adds a planned sprint after it), keeping their status and `sprint_history`; the sprint gets `end` and `report: {done, carried, added, to}`, `added` being the tasks whose history entry for it begins after its `start` (SR-4.3, SR-4.5) |
| `atm sprints delete <S> [--move-to <S2> \| backlog] [--force]` | deletes a sprint created by mistake (T-339): an empty one (no task in it or in its history, in either file) at once, closed ones too; an open one's unfinished tasks go to `--move-to` (the backlog by default). Refused while a task in it is in progress, for a closed sprint that had tasks, and, without `--force`, for an open one that tasks finished in or moved on from (with `--force` every task still in it moves). An active sprint deleted leaves none active |
| `atm releases [--all]`, `atm releases add <R> [--name N] [--target DATE] [--pr N]... [--released-at TIME]`, `atm releases edit <R> [--name] [--target]`, `atm releases archive <R>`, `atm releases notes <R>` | the releases in order with their tasks (by their effective release, both files) by kind and by stage (done, in PR, merged), archived ones with `--all`; add a planned one (`--pr N`, repeatable, gives it every task of those pull requests, in both files, and records the last one; `--released-at TIME` adds it as released at that time: releases recovered from PRs, SR-8.2, T-310), change one (an empty value removes a field), archive one, or print a Markdown draft of its changes for the CHANGELOG and WHATS-NEW entries a person writes (its done tasks grouped by type in the project's order, each with its ID, name and last outcome note, then the tasks not done yet; `--json` gives the same as data) ([sprints design](../sprints-and-releases-design.md) §3.3, SR-2.2). A sprint's or task's `--release`, and the `--release` filters, must name one of them |
| `atm components`, `atm components add <id> [--label T] [--path GLOB]... [--review MODE] [--from-label L] [--assign]`, `atm components edit <id> [--label T] [--add-path GLOB]... [--review MODE] [--from-label L] [--assign]` | the components (`meta.components`) in order with their label, globs, review mode and how many tasks have them, archived ones too; add one, in one locked write (T-324): `--from-label L` moves the label onto it on every task in both files (added to `components`, removed from `labels`) and drops `L`'s entry from `meta.labels`, whose text becomes the component's label unless `--label` gives one; `--assign` also gives it to every task whose `files` its globs match. Components may overlap. It refuses a component already there, a bad ID, an empty glob, `--assign` without `--path`, and a label no task has and `meta.labels` doesn't define (a typo), writing nothing. For projects that marked their parts with labels before components existed ([sprints design](../sprints-and-releases-design.md) §2, SR-2.7). `edit` (T-326) changes an existing one the same way: `--label` and `--review` set them (empty removes them), `--add-path` adds globs, `--from-label` moves another label onto it and `--assign` gives it to the tasks its globs (the new ones too) match; it refuses an unknown component, naming them |
| `atm move <id>... --to <S>\|next\|active\|backlog` | moves to-do, blocked and in-review tasks into a sprint, the next planned one after theirs, the active one, or the backlog, keeping their `sprint_history`; claimed and finished tasks are refused, and then nothing moves (SR-3.1) |
| `atm backlog [--all]` | the unfinished tasks with no sprint, as `atm list` prints them (finished ones too with `--all`) |
| `atm list`, `atm status`: `--sprint S`... `--backlog` `--release R` `--component C`; `atm archive --sprint S`... | only those tasks (a task's release is its own, else its sprint's); `atm status` starts with the active sprint: progress, due, release, goal (SR-3.2) |
| `atm add`: `--sprint S --release R --component C... --review MODE --related ID...`; `atm edit`: `--sprint S \| --backlog`, `--release R`, `--review MODE`, `--add-/--remove-component`, `--add-/--remove-related` | the sprint fields; an empty `--release` or `--review` removes it (SR-3.3) |

Phases are gone (T-320): `atm phases`, the `--phase` options and the phase in `list`, `status` and `next` were
removed once every reader used sprints; `atm migrate` turns a project's phases into sprints (DM-6.3), and a `phase`
left on a task is kept but ignored.

### 4.4 Archive

| Command | Does |
|---|---|
| `atm archive [--sprint S]... [--before DATE] [--keep-days N] [--dry-run]` | moves finished tasks to the archive (DM-5); prints what it moved and the new sizes |
| `atm unarchive <id>...` | brings tasks back, for example to reopen one |

### 4.5 Git and workflow

| Command | Does |
|---|---|
| `atm commit <id> --file F [--file F]... [--message T]` | stages exactly the files the agent names (`--file` is required; atm-cli never works them out, since another agent's work can be in the same tree) plus `tasks.json`, and commits them; refuses `-A`, `.` and paths outside the repository; the message comes from `meta.workflow.commit_template` (default `feat({id}): {name}`) and its trailer rule; records `commit` on the task (the SHA exists only after the commit, so that change to `tasks.json` goes in with the next commit) |
| `atm commit <id> --record <rev>` | records an existing commit on the task without committing (T-343), e.g. the one a rebase or cherry-pick made of its commit: the revision must be a commit on the current branch (an ancestor of HEAD) and not start with `-`; stored as the full SHA; a hint when its message doesn't mention the task's ID |
| `atm workflow [set <direct\|pull-request>] [--base B] [--verify true\|false] [--max-rounds N]` | prints or changes `meta.workflow` |
| `atm pr <id>... --number N --url U` | records the PR on tasks |
| `atm merged --pr N [--at TIME]` | records the merge on the PR's tasks; a planned release whose tasks in `tasks.json` (its own or their sprint's, skipped ones aside) are now all merged in that PR becomes `released`, with its `pr` and `released_at`; one with tasks outside it stays planned, and atm names them (SR-3.7) |

### 4.6 Setup

| Command | Change |
|---|---|
| `atm migrate` *(exists)* | compares the dashboard's **build ID** (a second meta tag, `atm-dashboard-build`, a hash the dashboard build computes once) instead of only the version, so a new build with the same version is installed. It reads only the head of both files, as today, and hashes nothing at run time. Adds `meta.next_id` when missing. |
| `atm serve --dir <path>` *(exists, new option)* | acts on another project's server without a `cd` |
| `atm init --meta-file F` *(exists, new option)* | merges a JSON object into `meta` (project, description, phases, vocabularies), creating `tasks.json` if there is none, after checking the vocabularies; keeps `meta.id`. `/atm:create-tasks` uses it so it never writes `tasks.json` itself (added with T-128) |
| `atm paths`, `version`, `claims`, `merge-driver` *(exist)* | unchanged; the merge driver also handles the archive |

## 5. Hints

| ID | P | Requirement |
|---|---|---|
| CLI-5.1 | P1 | When the archive threshold is reached (DM-5.3), commands add one line, e.g. ``tasks.json is 1.4 MB with 620 finished tasks; `atm archive` would move 580 of them to tasks-archive.json.``, and `"suggest": {"archive": {…}}` with `--json`. |
| CLI-5.2 | P1 | `atm status` and `atm migrate` always show the hint; `atm done` at most once a day per clone (remembered in the claims store), so a loop doesn't repeat it. |

## 6. Plumbing

| ID | P | Requirement |
|---|---|---|
| CLI-6.1 | P1 | **A stable command name.** Skills call `"${CLAUDE_SKILL_DIR}/../../scripts/atm"`, a versioned cache path that changes with every upgrade, so permission rules don't carry over. First check whether Claude Code puts a plugin's `bin/` on the Bash tool's PATH. If it does, ship `bin/atm` and the skills call plain `atm` (one rule, `Bash(atm *)`); if not, the README documents one rule for atm-cli's path. **Finding (T-124):** it does: "Files in `bin/` at the plugin root are on the `PATH` of the Bash tool's shell while the plugin is enabled" ([plugin components](https://code.claude.com/docs/en/plugins/components.md)), after the user's own `PATH` entries. So `plugin/bin/atm` (which runs `scripts/atm`, still the merge driver's target) is the command; the skills call `atm` and pre-approve it with `allowed-tools: Bash(atm *)`, and fall back to `${CLAUDE_SKILL_DIR}/../../bin/atm` if `atm` isn't found. A plugin can't ship allow rules (only `agent` and `subagentStatusLine` take effect from a plugin's `settings.json`), so the README gives users the one rule. |
| CLI-6.2 | P2 | **Automatic heartbeats.** A plugin `PostToolUse` hook renews the agent's claims instead of the agent remembering to. It reads only the claims store and writes only when the lease is more than ten minutes old. **Done (T-125):** `plugin/hooks/hooks.json` runs `atm heartbeat --auto` after every tool call. The agent is the hook's `session_id` (as `claude:<first 8>`, with `@<folder>` in a linked worktree, as CLI-3.5), the project its `cwd`; a claim records `renewed_at`, and one is renewed when that (or `claimed_at`) is ten minutes old. It never prints and always exits 0. |
| CLI-6.3 | P1 | **Leases during long commands.** The hook renews only after a tool call, and an agent waiting on a long command (a full test suite, a build, CI) makes none, so a wait longer than the lease handed the task to someone else (seen with three parallel agents, each running a 10-minute verify). **Done (T-203):** `atm run <id> [--every SECONDS] -- <command> [args...]` renews the claim at the start (refusing before the command runs when there is no live claim, or another agent holds it), then three times per lease (at least once a minute) while the command runs, from the main thread between waits (the lock uses signals). The command's output passes through and atm exits with its exit code (a signal N as 128+N). A renewal that fails is reported once on stderr and never stops the command. The skills run long checks this way. Claude Code has no hook for a running background command, so the hook can't cover it. |
| CLI-6.4 | P1 | **Waiting tasks across worktrees.** The claims store (`<git common dir>/atm/claims.json`) recorded tasks *completed* in a worktree, so `next` wouldn't hand them out elsewhere before the branches merge, but not tasks blocked or in review there: a second agent redid a blocked task, and a block had to be answered in each worktree (seen with three parallel agents). **Done (T-204):** the store has `waiting` (`{task, status, waiting_on, questions, worktree, branch, at}`, written by `block` and `review`, removed by any other status change of the task) and `resolved` (`{task, command, answers, reason, by, at, worktree}`). `next` skips waiting tasks in every worktree ("blocked or in review in another worktree: …"). `unblock`, `skip` (a blocked task), `done` and `reject` (a task in review) work in any worktree: one where the task isn't waiting first takes the waiting state from the store, applies the command, and records it in `resolved` for the worktree the task waits in, whose next status command or `next` applies it to its own copy. `atm status` lists them, marked "[in another worktree, <branch>]". The server leaves a linked worktree's stale copy out of `worktrees.json`. |

## 7. Benchmark

| ID | P | Requirement |
|---|---|---|
| CLI-7.1 | P1 | `tests/scripts/bench.mjs` (committed) generates 1k- and 10k-task files and times `atm` writes, the merge driver and the dashboard's reload. It replaces the estimates in the [data model](data-model.md#5-size-and-the-archive) and runs in CI as a check with generous limits. |
