---
name: serve
description: >
  Start, stop or check the local server for the task dashboard, one for all your projects. Use when the user says
  "serve the dashboard", "start the dashboard", "open the dashboard", "show all projects", "stop the dashboard
  server", or "is the dashboard running".
allowed-tools: Bash(atm service:*), Bash(atm project list:*), Bash(atm project add:*), Bash(atm project remove:*), Bash(atm project prune:*), Bash(atm vcs repository:*), Bash(atm project status:*), Bash(atm task details:*), Bash(atm task list:*)
---

# Serve the Dashboard

You start, stop or report the background HTTP server that shows the task dashboard. There is **one server for all of
the user's projects**: it shows this project, the others atm has run in, and an overview of all of them. The plugin's
`atm` command does the work; you run it and relay what it prints. Never read or edit `tasks.json` or
`tasks-archive.json` directly: `atm project status`, `atm task details` and `atm task list` answer questions about the tasks.

## Step 1: Parse arguments

`$ARGUMENTS` may contain:

| Argument | Meaning |
|---|---|
| `start` (default) | Start the server, or report the one already running, and give this project's address |
| `stop` | Stop the server (for all projects) |
| `status` | Say whether it runs, and where |
| `--open` | With `start`: also open the dashboard in the default browser |
| `--all` | With `start`: give the overview of all projects instead of this project |
| `--port N` | With `start`: try port N first (default 8765; the next free port is used if it's busy) |
| `--dir PATH` | Show the project at PATH instead of the current one |

## Step 2: Run the helper

From the project root, run:

```bash
atm service <action> [--open] [--all] [--port N] [--dir PATH]
```

(`atm` is atm-cli, the plugin's command; its `bin/` is on your PATH while the plugin is enabled. If the shell says
`atm: command not found`, write `"${CLAUDE_SKILL_DIR}/../../bin/atm"` instead.)

The helper:
- moves an old `docs/tasks.json` (or `tasks.json` in the root) into `.agentic-task-manager/` first, and cleans up
  after older versions (a project's own copy of the dashboard and its own server);
- adds the project to the dashboard's list of projects (`atm project list` shows them);
- serves the dashboard on `localhost` in a background process that keeps running after this session ends. The
  dashboard comes with the plugin, so updating the plugin updates it: the server restarts itself on the new version.

If it exits with code 127, Python 3.9 or newer is not installed: atm needs it, so tell the user to install it.
For any other error, show its message.

## Step 3: Report

Relay the helper's output. After a successful `start`, add:

> **Dashboard:** <the URL it printed>
>
> It shows all your projects: switch with the arrow next to the title, or open **All projects** for the overview.
> It refreshes itself every 5 seconds, so you can watch an agent work through tasks with `/atm:loop-tasks`.
> The server keeps running in the background; stop it with `/atm:serve stop`.

If the output included a line about moving `tasks.json`, also say the move is staged (not committed) and that it
should be committed.

Then run `atm vcs repository`. If it says there is no address yet, add one line: commits and pull requests on the
dashboard become links once the repository's address is set, with `atm vcs repository <url>` (or ask me to set it).
