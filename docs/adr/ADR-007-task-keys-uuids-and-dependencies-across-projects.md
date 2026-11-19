# ADR-007: Tasks stored by UUID, shown by a project key (ATM-527), with dependencies across projects

- **Status:** accepted, 2026-11-15 (T-534); §7's lookup and freshness replaced by
  [ADR-008](ADR-008-other-projects-tasks-referenced-anywhere-loaded-when-needed.md) (2026-11-16, from
  [cross-project-freshness-design.md](../cross-project-freshness-design.md)); §3 (provisional keys) and §4's rules on
  which number moves replaced by [ADR-009](ADR-009-task-keys-plain-from-creation.md) (2026-11-19: keys plain from
  creation, a clash moving one task to the next free number)
- **Decided by:** Hakan Serce (T-534: "stored as UUID, displayed as human-readable", the number permanently tied to the
  UUID, the prefix a project's short name of 3 to 6 letters and digits, a migration that changes it any time, a
  clash shown by UUID with a warning; provisional keys until merged, with a marker)
- **Context:** T-534; [ADR-003](ADR-003-workspaces.md) (workspaces); the merge driver
  (`crates/atm-db/src/jsongit/merge.rs`); [security-architecture.md](../security-architecture.md)

## Problem

A task's ID today is one string, `T-527`, both what people read and what the files point at:

- **There is no prefix setting.** The next ID copies the last task's prefix and width (`edit.rs`'s `id_format`, `T-`
  and 3 digits by default), so every project's tasks are `T-…` and no ID says which project it belongs to.
- **Nothing can point at another project's task.** A dependency is resolved by exact string in the project's own
  tasks and archive (`rules.rs`'s `unfinished_dependencies`); `LFB-005` can't wait for `ATM-978`.
- **Two branches adding tasks collide.** Both take the next number; the merge driver renumbers the incoming side's
  task (`next_free_id`) and follows the rename in that side's new tasks' `dependencies` only: not in `related`, not in
  ours', and not in anything written outside the file (commit messages, notes, chat), so a mention can end up naming
  the wrong task.
- **Renaming is impossible.** An ID is a primary key, so changing a project's prefix would mean rewriting every
  reference.

## Decision

### 1. Two identities: a UUID stored, a key shown

- **`id` is the task's UUID** (version 7 for a new task, so it sorts by creation; version 8 for one the first
  migration converted, §6), given once and never changed. Every reference in
  the files is a UUID: `dependencies`, `related`, claims, the archive, notes that point at a task. Keeping the field
  name `id` keeps every lookup by `id` as it is; only its value changes.
- **`number`** is the task's number in its project (an integer, no padding), given from `meta.next_number`.
- **The key is `<meta.key>-<number>`**, worked out, never stored: `ATM-527`. It is what people and agents read and
  type, in the CLI, the dashboard, the app, commit messages and the skills.
- **`--json` carries both** (`"id"` the UUID, `"key"` the key, `"key_final"` below), so scripts can hold the UUID.

### 2. The project's short name

- **`meta.key`**, beside `meta.project` (the name): 3 to 6 ASCII letters and digits, kept as written (`ATM`,
  `TskMan`), matched without regard to case. `atm db init` asks for it on a new project and suggests the name's
  initials (Agentic Task Manager: `ATM`).
- **`meta.key_history`**: the project's earlier prefixes, oldest first (`["T", "ATM"]`), so an old key still finds
  its task (§5).

### 3. A number is permanent once its task is on the base branch; until then the key is provisional

- **`key_final`** on each task: `false` when it is added, `true` once its task is on the project's base branch
  (`meta.workflow.base`, else the remote's default branch). atm checks that from git, never from what an agent says:
  the task, with that number, is in `origin/<base>`'s `tasks.json`. `atm vcs pushed` (a PR merged, or a direct
  push) and `atm db migrate` make the check and set the flag; nothing sets it back.
- **A final number never changes.** That is the guarantee: a key shown without the marker belongs to that UUID for
  good.
- **A provisional key is marked wherever keys show**: lighter, with a "draft" mark, and a tooltip "Provisional until
  merged into main: the number may change if another branch took it"; the CLI marks it with a `~` (`~ATM-538`).
  Agents and commit messages use it freely: §4's alias keeps it working.

### 4. Merges match by UUID and move only provisional numbers

- The merge driver matches tasks by `id` (the UUID). Two branches that each add a task add two tasks, never one
  merged into the other.
- **Two tasks with the same number:** a provisional one takes the next free number (above both sides' numbers and
  `meta.next_number`); of two provisional ones, the incoming side's moves (as today). Two final ones can't happen (only
  the base branch makes numbers final); if they do, the merge stops with a conflict rather than guess.
- **A task whose number moved keeps its old key as an alias** (`aliases: ["ATM-538"]`), so a commit or note written
  on the branch still finds it, and atm says it moved ("ATM-538 is now ATM-541").
- **No reference is rewritten**: they are UUIDs. `meta.next_number` merges as the larger of the two.

### 5. Every place that takes a task accepts a key, an alias or a UUID

`atm task …`, `atm vcs commit`, dependencies given to `atm task add`/`edit`, the dashboard's `?task=` and
`atm://…?task=` addresses: a current key (`ATM-527`), a key with an earlier prefix (`T-527`, from `meta.key_history`
and the same number), a task's own alias (`ATM-538`), or the UUID. Addresses use the key, so they stay readable; an
alias resolves to the task it moved to.

### 6. Changing the prefix is a migration, the first time and later

- **`atm project key set <KEY>`** sets `meta.key` and pushes the old one onto `meta.key_history`. Numbers don't
  change, nor does any reference; only how keys read. `T-527` becomes `ATM-527`, later `TskMan-527`, and all three
  find it.
- **The first migration** (`atm db migrate` on a file without UUIDs) gives every task, archived ones too, a UUID and
  its number (the trailing digits of its ID, when the ID has the file's prefix and the number is free; an ID that
  doesn't fit, `T-P1-03`, gets the next number atm never gave, above `meta.next_id`, and keeps its old ID as an alias),
  rewrites the references to UUIDs (`dependencies`, `related`, `duplicate_of`, the sprints' reports), keys the claims
  by UUID, sets `meta.key` (one the file names, else the name's initials when they make 3 to 6 letters, else the old
  prefix, to change with `atm project key set`), puts the old prefix in `meta.key_history`, and marks every task
  final (they are all on the base branch or on branches made from it).
- **The same in every clone** (T-540): each clone and worktree migrates its own copy, so a converted task's UUID is
  worked out, not random: a version-8 UUID from the project's `meta.id` and the task's old ID. Two branches that both
  migrate give each task the same UUID and number, and merge task by task.
- **By `atm project key set <KEY>`** (T-543), the one migration feature, as Hakan put it: on a project of IDs it is
  the first migration, with the key chosen; on one with keys, a change of prefix. `atm db migrate` leaves IDs alone,
  so no project changes its tasks' IDs until someone sets its key. A new project (`atm db init`) starts with keys.
- **The file format changes** (`meta.format`: 2), and an atm that knows formats refuses a newer one ("upgrade atm to
  work on this project"). atm 0.37 and older don't check it, and would read a format-2 file as one of IDs (UUIDs pass
  their ID rule) and give new tasks `T-` IDs: the skills require atm 0.38 or later (T-547) and the release notes say to
  upgrade every clone. The merge driver migrates an old side first, as it turned phases into sprints, so a branch made
  before the migration still merges.

### 7. Dependencies across projects

> [!NOTE]
> Where a key is looked up (every registered project), and how another project stays up to date (loaded when needed,
> never watched), are [ADR-008](ADR-008-other-projects-tasks-referenced-anywhere-loaded-when-needed.md)'s.

- **A reference to another project's task** is `<project id>:<UUID>` (the project's `meta.id`, 8 hex digits, which
  never changes), so atm knows where to look and can say what is missing. It is written by key
  (`atm task edit LFB-5 --add-dep ATM-978`): atm finds the project whose `meta.key` is `ATM` among the projects of the
  workspaces it is listed in, then the task by key or alias there.
- **Resolved through the registry** (`workspaces/<w>.json`), read only: its status shown where dependencies show, an
  unfinished one an unmet dependency for `atm loop next` and the dashboard; a project not registered or missing, or a
  task not found, an unmet dependency named by what is known ("project 3f2a9c1e (not in your workspaces): task …"),
  reported by `atm doctor`.
- **Reading another project's task folder is a new entry point** (`docs/security-architecture.md`): only projects the
  user registered, through the same store and its checks (the folder's symlink rules), for the task's status, name
  and key; nothing in them is run or followed further.

### 8. Two projects with the same short name

Keys are unique within a project. Across a workspace two projects may share a short name (two `ATM`s); atm doesn't
refuse it (`atm project key set` warns), but where a key would be ambiguous (the overview, a dependency on another
project, search across projects) atm shows the task's UUID (its first 8 characters) with a warning: "Two projects in
this workspace use the key ATM: rename one with `atm project key set`". Inside a project's own views its keys stay.
Typing an ambiguous key elsewhere is refused with the same advice; the UUID always works.

### 9. Where keys show

The CLI's output and `atm docs`; the dashboard (cards, rows, details, the graph, addresses, related tasks, search) and
the app; the commit template's `{id}` becomes the key (with `{uuid}` for those who want it); `atm vcs commit
--record`'s check that the message names the task accepts the key or an alias; the skills' examples
(`skills/atm-*/SKILL.md`), `docs/agent/` and `create-tasks`' placeholders (`new-1` stays).

## Alternatives considered

- **References by key** (the key stored in `dependencies`): readable in the file, but every renumber and every prefix
  change rewrites references, the merge driver has to follow renames everywhere (today it doesn't), and a reference
  across projects breaks when the other project changes its prefix.
- **Number ranges per agent or branch**, so numbers never collide: no renumbering, but gaps and numbers that say
  nothing about order, and a range to hand out to every clone.
- **Keys given only at merge**: permanent from the start, but a new task would have no short key until merged, which
  is when agents and people talk about it most.
- **A global registry of keys** (a server, like Jira's): atm is files in git, with no server to ask.

## Risks

- **A one-way file migration.** Mitigated by the version check, the merge driver migrating old sides, and the old
  IDs kept as resolvable keys; a scenario migrates this repository's own file (300+ tasks, the archive) and checks
  nothing is lost.
- **`origin/<base>` stale or missing** (no remote, not fetched): keys simply stay provisional until a check can see
  the base; atm never guesses final.
- **Many projects to scan** for a dependency across projects: resolved per command, from the registry's paths, with
  the store's own caching; only projects named by a reference are read.
- **The `~` and the draft mark are new to read**: the help, `atm docs tasks` and the tooltip say what they mean.

## Consequences

- Implementation tasks (in order): the data model and schemas (`id` as UUID, `number`, `key_final`, `aliases`,
  `meta.key`, `meta.key_history`, `meta.next_number`, the file's version); key parsing and resolution (§5) in
  atm-common and atm-lib; `atm db migrate`'s migration and `atm project key set`; the merge driver by UUID (§4);
  finality (`atm vcs pushed`, migrate; §3); references across projects (§7) and `atm doctor`; the CLI's output and
  `--json`; the dashboard and the app (keys, the marker, addresses, the warning); `docs/agent/`, the skills and the
  security document.
- `edit.rs`'s `id_format`/`split_id` and the merge driver's `next_free_id` go; the hard-coded `T-` with them.
- `docs/security-architecture.md` gains §7's entry point.
