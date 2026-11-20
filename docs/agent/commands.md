# Commands: atm's commands, by what you are doing

Every command takes `--json` and `--dry-run` (`-n`), anywhere on the line; `atm help` lists them all and
`atm <command> --help` describes one. Exit codes: 0 ok, 1 an error (the message says what: no such task, a refusal),
2 a usage error (a value atm doesn't know, a note too long), 3 nothing to do (`loop next`) or a wait whose time was up
(`review wait`, `review watch --once`), 4 a conflict: the task is claimed by another agent, or your own lease ran out.
Output to a pipe (as agents get it) is plain text, never cut; `--color auto|always|never` only styles it for a terminal.

`<id>` is a task's key (`ATM-527`, any case) in a project with task keys, or its old ID (`T-527`), an alias or its UUID;
in a project without keys, its ID (`atm docs tasks`). Keys are plain from the moment a task is made: use them anywhere
(commit messages too). If two branches gave the same number, the merge moves one task to the next free number (the one
not on the base branch; when both or neither are, the newer), the next `atm db migrate` says which ("ATM-538 is now
ATM-541"), and its old key keeps finding it once no task holds that number. (A `~` before a key, as older atm printed
it, is still taken and ignored.)

## Reading

- `atm project status`: the overview (the active sprint, what is in progress, in review or blocked, what's next);
  `--all` adds the counts by sprint, type and label
- `atm task details <id>…`: tasks in full, with their notes
- `atm task deps <id>`: what a task waits for, and what waits for it
- `atm task list [--status S] [--sprint NAME | --backlog] [--type T] [--priority P] [--label L] [--ready]
[--blocked] [--in-pr] [--archived]`: one line per task (key, status, priority, name, labels), then a line counting
  them
- `atm claim list`: who works on what; `atm project workflow`: how the project pushes
- `atm docs <topic>`: this reference (`atm docs` lists the topics)

## Working on a task

- `atm loop next --claim`: the next task, started and claimed in one step (`atm docs claims`)
- `atm task state <id> <state>`: every change of status (`atm docs states`)
- `atm task note <id> '<text>'`: a note on a task
- `atm claim hold <id> && <command>`: keep your claim alive while a long command runs
- `atm vcs commit <id> --file F …`: commit the task's files (`atm docs rules`)
- `atm review create|show|comments|reply|round|wait|watch …`: a code review of your task's change, and listening to
  it (`atm docs reviews`); `atm review poke` is a person's nudge, which you answer, never make; `atm review draft …`
  is a person's drafts, never an agent's

## Pushing

- `atm vcs pr <id>… --number N --url U`: record the pull request that contains the tasks
- `atm vcs pushed --pr N [--at TIME]`: record that it merged (a direct push has nothing to record)

## Defining work

- `atm task add` (`--json-file F` for many) and `atm task edit <id>`: a task's definition (never its status)
- `atm sprint list|add|edit|rename|start|close|delete|move|backlog`: sprints (planned, started and closed with the
  user); `atm sprint rename <old> <new>` renames one in any state, and its tasks (archived ones too) follow
- `atm release …`, `atm component …`: releases, and the project's parts by file globs; `atm release rename <old>
<new>` changes a release's ID, and its tasks (archived ones too) and sprints follow; `atm component rename <old>
<new>` a component's, and the tasks listing it follow
- `atm label rename <old> <new>`: a label on every task that has it, archived ones too
- `atm project vocab rename status|type|priority <old> [<new>] [--label L]`: a vocabulary entry's ID or label; the
  tasks using it (archived ones too) and the default priority follow
- `atm release edit <id> --released [YYYY-MM-DD]`: a published release marked released by hand, when a task with no
  commit (a check done with the user) keeps `atm vcs pushed` from doing it; refused while a task of it is open, unless
  `--force`; `--planned` takes it back. Only when the user says the release is out
- `atm db init --meta-file F`: the project and its vocabularies (a new project gets task keys: give `key`, 3 to 6
  letters and digits, in the file); `atm project workflow set`: how it pushes
- `atm project key show|set <KEY>`: the project's key, its tasks' prefix. `set` on a project of IDs (`T-527`) gives
  every task a key, keeping its number (`ATM-527`; `T-527` still finds it); on one with keys it changes the prefix.
  Only with the user's say: it rewrites every task's ID
- A dependency on another project's task (`--add-dep LFB-5`): found by its key among the user's registered projects,
  and waited for until it is done there

## Maintenance

- `atm db migrate`: move an old task file into `.agentic-task-manager/` and set the folder up
- `atm db archive` (when atm suggests it) and `atm db unarchive <id>…`
- `atm doctor`: check the task file
- `atm service [start|stop|status]`: the dashboard's server
- `atm workspace list` and `atm workspace projects [<w>]`: the workspaces, named lists of the projects the dashboard
  shows (`default` always there; atm adds a project to it when it runs there). Making, deleting or renaming one and
  adding or removing projects (`atm workspace create|delete|rename|add-project|remove-project|prune`) are the user's
  to decide. No other command takes a workspace: every one works on the project in the current folder
- `atm agents install|status|uninstall`: atm's skills where the user's agent harnesses read them; the user's to run,
  never on your own
