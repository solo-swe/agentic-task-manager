# Commands: atm's commands, by what you are doing

Every command takes `--json` and `--dry-run` (`-n`), anywhere on the line; `atm help` lists them all and
`atm <command> --help` describes one. Exit codes: 0 ok, 1 an error (the message says what), 2 a usage error, 3 nothing
to do (`loop next`), 4 the task is claimed by another agent.

## Reading
- `atm project status`: the overview (the active sprint, what is in progress, in review or blocked, what's next)
- `atm task details <id>…`: tasks in full, with their notes
- `atm task deps <id>`: what a task waits for, and what waits for it
- `atm task list [--status S] [--sprint NAME | --backlog] [--type T] [--priority P] [--label L] [--ready]
  [--blocked] [--in-pr] [--archived]`: one line per task
- `atm claim list`: who works on what; `atm project workflow`: how the project pushes
- `atm docs <topic>`: this reference (`atm docs` lists the topics)

## Working on a task
- `atm loop next --claim`: the next task, started and claimed in one step (`atm docs claims`)
- `atm task state <id> <state>`: every change of status (`atm docs states`)
- `atm task note <id> '<text>'`: a note on a task
- `atm claim hold <id> && <command>`: keep your claim alive while a long command runs
- `atm vcs commit <id> --file F …`: commit the task's files (`atm docs rules`)

## Pushing
- `atm vcs pr <id>… --number N --url U`: record the pull request that contains the tasks
- `atm vcs pushed --pr N [--at TIME]`: record that it merged

## Defining work
- `atm task add` (`--json-file F` for many) and `atm task edit <id>`: a task's definition (never its status)
- `atm sprint list|add|edit|start|close|delete|move|backlog`: sprints (planned, started and closed with the user)
- `atm release …`, `atm component …`: releases, and the project's parts by file globs
- `atm db init --meta-file F`: the project and its vocabularies; `atm project workflow set`: how it pushes

## Maintenance
- `atm db migrate`: move an old task file into `.agentic-task-manager/` and set the folder up
- `atm db archive` (when atm suggests it) and `atm db unarchive <id>…`
- `atm doctor`: check the task file
- `atm service [start|stop|status]`: the dashboard's server
- `atm agents install|status|uninstall`: atm's skills where the user's agent harnesses read them; the user's to run,
  never on your own
