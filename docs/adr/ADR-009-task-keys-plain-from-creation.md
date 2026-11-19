# ADR-009: Task keys are plain from creation; a number clash at a merge moves one task to the next free number

- **Status:** proposed, 2026-11-19 (ATM-634); replaces [ADR-007](ADR-007-task-keys-uuids-and-dependencies-across-projects.md)
  §3 (provisional keys) and §4's rules on which number moves
- **Decided by:** Hakan Serce (2026-11-19: "I don't want any draft number ever being created any more"; "T- era but
  better. Next free number is good"; no counter per machine)
- **Context:** ADR-007; ATM-623 (keys left provisional after a direct push), a CompMD task whose review was approved
  under `~CMD-250` but not counted for `CMD-250` (ATM-630), the `~` read by zsh as a home folder; the merge driver
  (`crates/atm-db/src/jsongit/merge.rs`)

## Problem

ADR-007 gave every task a UUID and a key (`ATM-527`), and marked a key **provisional** (`~ATM-527`, a "draft" mark in
the dashboard, `key_final: false`) until its task was on the base branch, because a merge may still move its number.

- **The marker never prevented a renumber.** A clash moves a number with or without it; the marker only labelled
  *every* new task as one that might move, though very few ever do.
- **Its bookkeeping is where the bugs are.** Keys are made final from git (`finalize_keys`, run by `atm vcs pushed`
  and `atm db migrate`): a direct push didn't run it (ATM-623); a review stored `~CMD-250` and no longer matched its
  task once the key was final; an agent typing `~ATM-630` in zsh gets "no such user or named directory"; the draft mark
  shows on most of the board most of the time.

Before ADR-007 (the `T-` era) there was no marker: the merge driver renumbered the incoming task silently and followed
the rename only in that side's new tasks' dependencies, so mentions elsewhere could name the wrong task. ADR-007 fixed
that with UUID references and aliases. This ADR keeps those and drops the marker.

## Decision

### 1. Kept from ADR-007

The UUID is the stored identity and every reference in the files is one (§1); the project key, its history and
`atm project key set` (§2, §6); merges match tasks by UUID (§4); every place that takes a task accepts a key, an
alias, an earlier prefix or a UUID (§5); dependencies across projects (§7, ADR-008); two projects with one short name
(§8).

### 2. A key is plain from the moment its task is made

- `atm task add` gives the next number (`meta.next_number`) and the key reads `ATM-641` at once, everywhere: the CLI,
  `--json`, the dashboard, the app, commit messages, review files.
- **Gone:** `key_final`, `finalize_keys` and its callers' key work (`atm vcs pushed` still records merges and
  releases), the CLI's `~`, the dashboard's draft mark and tooltip.
- **Old text still works:** a leading `~` on a key given to atm (an old note, commit or review file) is ignored.

### 3. A number clash at a merge: one task takes the next free number

Two different tasks (different UUIDs) with the same number, one from each side:

- **Which one moves.** The task already on the base branch keeps its number: atm checks it from git when the merge
  runs (that UUID with that number in the base branch's `tasks.json`, `origin/<base>`, else the local `<base>`),
  storing nothing. If neither is there, or the base can't be read, the **newer** task moves (UUIDv7 sorts by creation;
  a migrated UUIDv8 counts as older than any v7, and two v8 compare by UUID). So every clone, and a merge either way
  (a branch into main, or main into a branch), moves the same task, and a key that reached the base branch never
  changes.
- **Where it goes.** The next free number: above both sides' numbers, the archive's, and `meta.next_number`, which
  then merges as the larger of the two, past it.
- **Its old key stays an alias** (`aliases`), so a commit message, a note or a review written with it still finds
  the task. References in the files are UUIDs: nothing is rewritten.
- **A review written with the old key still means the task that moved** (ATM-630). Its key is ambiguous: the task
  that kept the number holds it now. So atm reads `tasks.json` at the review's own commit (its first round's, else
  the one that added its `review.md`) to see which task held that number then. Nothing is rewritten here either.
- **atm says what moved, once**: "ATM-538 is now ATM-641 (both branches made ATM-538)", from a record the merge
  leaves for the next atm command.
- **A clash never stops a merge.** (ADR-007 stopped on two final numbers; with the base-branch check, two tasks on the
  base branch with one number can't arise from a merge.)

### 4. Migration

`atm db migrate` drops `key_final` from `tasks.json` and `tasks-archive.json`, once, and says so. Today's provisional
keys become plain as they are; a clash still pending between branches is settled by §3 when they merge.

## Alternatives considered

- **A minor number on a clash** (`ATM-630.2`), Hakan's first idea: keeps the moved task visibly tied to the number it
  was made with, but adds a second key shape everywhere keys are parsed, sorted and shown. The alias gives the same
  link back (`ATM-641` "was ATM-630").
- **A counter per machine** (in atm's state folder, shared by every worktree): would make clashes between agents on one
  machine impossible, but is more state to keep, and clashes are rare enough that the alias covers them. Not needed.
- **Reserving numbers on the remote** (creating a ref such as `refs/atm/keys/ATM-631`, which git refuses to create
  twice): unique across machines, but a push on every new task, push rights, and nothing offline.
- **Hash keys** (`ATM-k3f9`), as [beads](https://beads.gascity.com/core-concepts/hash-ids) moved to after its agents
  kept minting the same sequential IDs on branches: never clash, but lose order and are hard to say.
- **Duplicate numbers told apart at lookup** (git-bug's stance: the ID is a hash, any unambiguous prefix works): no
  rename ever, but a bare key written before a clash stays ambiguous, and every lookup needs a "which one?" path. §3
  is this idea with the disambiguation decided once, at the merge, and stored.
- **Number ranges per agent or branch** (rejected in ADR-007 too): gaps, and numbers that say who made them.
- **Keeping the marker, fixing its bugs one by one**: the marker carries no information a clash needs; the alias does.

## Risks

- **The base branch can't be read at merge time** (no remote, not fetched): the newer task moves, which is right
  unless the newer one already reached the base on another clone; then a pushed key moves on that clone's next merge.
  The alias keeps it findable, and atm says it moved.
- **A commit message or document names a moved task by its old key**: it resolves through the alias (§5); text that
  isn't given to atm (a plain document read by a person) still says the old number.
- **Agents used to `~` keys**: atm accepts them; the docs and skills stop mentioning them.

## Consequences

- Tasks: atm drops provisional keys and migrates the files (ATM-635); the merge driver's clash rule (ATM-636); the
  dashboard and the app show keys plain, a moved task its old key (ATM-637); reviews find their task by an alias or an
  old key (ATM-630); the state machine for tasks and reviews has a "number moved at a merge" event, no "key made final"
  (ATM-633).
- `docs/agent/tasks.md`, `docs/agent/commands.md` and the skills lose the `~` and "provisional"; `atm docs tasks`
  explains a moved number and its alias.
