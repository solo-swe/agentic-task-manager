<p align="center">
  <img src="branding/atm-logo.svg" alt="atm logo: a board with three columns of cards, the last one done" width="96" height="96">
</p>

<h1 align="center">Agentic Task Manager</h1>

A **Claude Code plugin** that gives any project file-based task management with a live HTML dashboard. Agents decompose requirements into granular tasks, execute them in a loop, and humans watch progress in real time.

**One command to install. Nothing to set up beyond Python 3. Works with any language.**

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

---

## Tenets

- **The data lives with the project.** Tasks are a file in your repository (`.agentic-task-manager/`), versioned with
  your code by your own version control: branches, pull requests and history work for tasks as they do for code
  ([Project files](#project-files)).
- **One skill install.** Installing the plugin is all it takes: no service, no account, no database. It needs only
  Python 3, which macOS and Linux already have ([Prerequisites](#prerequisites)).
- **Actions through agents.** Every change to the tasks is made by an agent running a skill, or by `atm`, the command
  the skills use; never by editing the files by hand. That keeps the file valid, locked and merged task by task
  ([Running agents in parallel](#running-agents-in-parallel)).
- **Visibility through the dashboard.** The dashboard shows everything, across all your projects, and changes
  nothing: it is read-only, so what you see is always what the agents wrote ([Dashboard](#dashboard)).

---

## What It Does

1. **Decompose requirements into tasks** - Point the agent at your PRD, use cases, ADRs, or just describe what you need. It creates a `tasks.json` file with granular, dependency-aware tasks.

2. **Execute tasks one by one** - The agent picks up a task, marks it in-progress, implements it, verifies it, and marks it done (or asks for a review, or asks you a question when it can't go on without a decision). Every change goes through `atm`, the plugin's command, which writes `tasks.json` immediately.

3. **Loop through all tasks** - "Ralph Wiggum mode" 🫠 - the agent continuously picks up the next available task and works through the entire backlog while you watch.

4. **Push and get CI green** - `/atm:push` pushes the work straight to your main branch or through a pull request, watches CI and fixes what fails.

5. **Follow an agile Software Development Process along the way** - Either start going right away and use Kanban with the dashboard, or use a more structured agile SDL with a backlog, sprint planning, tracking, etc

6. Keep track of task data - Rick [CompMD](https://compmd.hakanserce.com) powered task descriptions and notes with code highlighting, mermaid diagram, task relations, commit/pr links, related files/documents.

7. Manage project data - releases, components, labels, types, statuses, priorities

8. **Watch progress live** - A self-contained HTML dashboard reads `tasks.json` over HTTP and shows progress, an interactive dependency graph, a Kanban board, task cards and a per-agent timeline, sprints, etc all searchable and filterable. It refreshes itself every few seconds, so you can watch tasks turn green as the agent works, and it tells you when a task needs you.

9. **Track Metrics** - Burndown, velocity, agent time, completion percentage, task per hour/day/week, task time, etc

10. **A multi-project dashboard** - Gives you a birds eye view of all ongoing projects.

11. **Omniboard** - A combined Kanban board allows you to visualize your projects in one board.

---

## Quick Start

### Install the plugin

In Claude Code (terminal or VS Code), add the marketplace and install the `atm` plugin:

```
/plugin marketplace add solo-swe/agentic-task-manager
/plugin install atm@solo-swe-plugins
```

Then run `/reload-plugins` so the skills load (no restart needed).

### Create tasks

```
/atm:create-tasks
```

The agent searches your project for requirement documents (PRD, use cases, ADRs, roadmap) and decomposes them into granular tasks. If it can't find any, it asks you where to look or lets you describe requirements directly.

Tasks go to `.agentic-task-manager/tasks.json` in your project.

### Start the dashboard

```
/atm:serve --open
```

This starts a small local server in the background and opens the dashboard in your browser. One dashboard shows all
your projects: switch with the arrow next to the title, or open **All projects** for an overview of what each one is
doing and what needs you. It refreshes every 5 seconds, so it stays live while agents work. `/atm:serve stop` stops
it; it otherwise keeps running after you close Claude Code.

### Execute tasks

Do a single task:

```
/atm:do-task T-001
```

Or let the agent pick the next available task:

```
/atm:do-task
```

Or loop through everything:

```
/atm:loop-tasks
```

---

## Installation

### Prerequisites

- [Claude Code](https://claude.com/code) (CLI or VS Code extension)
- **Python 3.9 or newer** (`python3`). The skills run a small helper script (`plugin/scripts/atm.py`) that moves task
  files and serves the dashboard. The libraries it uses come with the plugin, so there is nothing to install. On macOS,
  `python3` comes with the Command Line Tools (the same install that provides `git`). macOS and Linux are supported.

### Method 1: Via marketplace (recommended)

This repo is its own marketplace (`solo-swe-plugins`) containing the `atm` plugin:

```
/plugin marketplace add solo-swe/agentic-task-manager
/plugin install atm@solo-swe-plugins
```

Then run `/reload-plugins` to load the skills. To get a new version later:

```
/plugin marketplace update solo-swe-plugins
/reload-plugins
```

No restart is needed. The dashboard comes with the plugin, so this updates it too: a running dashboard server
restarts on the new version by itself (at the next tool call, or the next session), and the open dashboard offers a
**Reload**. Its version label opens what's new.

**Installed it before 0.21.0?** The marketplace was called `hakanserce-plugins` and lived at
`hakanserce/agentic-task-manager`. Remove it and add it again under its new name:

```
/plugin marketplace remove hakanserce-plugins
/plugin marketplace add solo-swe/agentic-task-manager
/plugin install atm@solo-swe-plugins
/reload-plugins
```

### Method 2: Local development

Clone and load directly:

```bash
git clone https://github.com/solo-swe/agentic-task-manager.git
cd your-project
claude --plugin-dir /path/to/agentic-task-manager/plugin
```

### Verify installation

Type `/atm:` in Claude Code - you should see seven skills in autocomplete:

- `/atm:create-tasks`
- `/atm:plan-sprint`
- `/atm:close-sprint`
- `/atm:do-task`
- `/atm:loop-tasks`
- `/atm:push`
- `/atm:serve`

---

## Skills Reference

### `/atm:create-tasks`

**Decompose requirements into tasks.**

```
/atm:create-tasks                          # Auto-detect requirements
/atm:create-tasks docs/PRD.md              # Use specific file
/atm:create-tasks Build a REST API with    # Describe requirements directly
  user auth, CRUD for posts, and pagination
```

**What it does:**

1. Searches for PRD, use cases, ADRs, roadmap, README, or existing tasks.json
2. If nothing found, asks you where to look
3. Decomposes requirements into granular tasks (~10 min each)
4. Assigns phases, types, dependencies
5. Writes `.agentic-task-manager/tasks.json` and sets up the folder (dashboard copy, `.gitignore`)
6. Outputs a summary with task counts and suggested starting tasks

**Supported requirement sources** (searched automatically):

- `**/PRD.md`, `**/requirements.md`
- `**/use-cases*`, `**/usecases*`
- `**/adrs/**`, `**/ADR-*`
- `**/ROADMAP.md`
- `**/TODO.md`, `**/BACKLOG.md`
- `README.md`

### `/atm:do-task`

**Execute a single task.**

```
/atm:do-task T-005     # Execute a specific task
/atm:do-task           # Auto-pick the next available task
```

**What it does:**

1. Starts and claims the task with `atm task state <id> in_progress` (or `atm loop next --claim` to auto-pick), which checks its dependencies
2. Reads the task and its notes (a reviewer's rejection, a previous agent's handoff) with `atm task details`
3. Implements the task according to its description, updating later tasks with `atm task edit` if it has to
4. Runs build/test/lint verification
5. Commits exactly the files it changed with `atm vcs commit`, while the task is still in progress
6. Only then ends it: `atm task state <id> done --note "<outcome>"`, or `atm task state <id> in_review` when a review is asked for, or
   `atm task state <id> blocked --on user --question "..."` when it can't go on without your decision. A task is never marked done with its work
   uncommitted; one marked done by mistake goes back with `atm task state <id> todo --reason "..."`
7. Reports what was done and what's now unblocked, and suggests `/atm:push` when there are unpushed commits

**Task selection** (when auto-picking): a task of the active sprint (any task, in a project without sprints): to-do,
all dependencies done, not claimed by another agent; the most urgent priority first, then the lowest task ID.

### `/atm:loop-tasks`

**Loop through all available tasks. Ralph Wiggum mode.** 🫠

```
/atm:loop-tasks                           # The active sprint's tasks (all tasks, without sprints)
/atm:loop-tasks --sprint "Ada Lovelace"   # Another sprint's tasks
/atm:loop-tasks --tasks T-4,T-7           # These tasks, in this order
/atm:loop-tasks --all                     # Active sprint, planned sprints, then the backlog
/atm:loop-tasks --backlog                 # Tasks in no sprint
/atm:loop-tasks --continue                # On to the next planned sprint when this one is finished
/atm:loop-tasks --type test               # Only test tasks
/atm:loop-tasks --max 5                   # Stop after 5 tasks
/atm:loop-tasks --pause                   # Ask before each task
/atm:loop-tasks --no-commit               # Skip auto-commit
/atm:loop-tasks --push                    # Git commit + push after each
/atm:loop-tasks --batch-push              # Commit each, push at the end
/atm:loop-tasks --push-every 3            # Commit each, push every 3
/atm:loop-tasks --dry-run                 # Preview execution order
```

**Flags:**

| Flag                | Description                                                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `--sprint <S>`      | Sprint S's tasks (default: the active sprint)                                                                                   |
| `--tasks <T-1,T-5>` | These tasks, in this order                                                                                                      |
| `--all`             | Every task: the active sprint, the planned ones in order, then the backlog                                                      |
| `--backlog`         | Only tasks in no sprint                                                                                                         |
| `--continue`        | When the sprint is finished, go on to the next planned one; the loop never closes a sprint (`/atm:close-sprint` does, with you) |
| `--type <type>`     | Only tasks of a specific type (feature, test, etc.)                                                                             |
| `--max <N>`         | Stop after N completed tasks                                                                                                    |
| `--dry-run`         | Preview execution order without doing anything                                                                                  |
| `--pause`           | Ask for confirmation before each task                                                                                           |
| `--no-commit`       | Disable the default auto-commit after each task                                                                                 |
| `--push`            | `git commit` + `git push` after each task                                                                                       |
| `--batch-push`      | `git commit` after each, single `git push` at the end                                                                           |
| `--push-every <N>`  | `git commit` after each, `git push` every N tasks                                                                               |
| `--no-watch`        | With a push flag: don't watch CI after the last push either                                                                     |

Pushes go through [`/atm:push`](#atmpush): those in the middle of the loop don't wait for CI (`--no-watch`); the last
one watches CI and fixes failures.

**Commit message format:** `feat(T-001): Short task name`

**The loop re-reads tasks.json every iteration**, so you can change tasks while the agent runs: ask another agent
session, or run `atm` yourself (`atm task edit T-005 --priority P0`, `atm task state T-007 skipped --reason '…'`), and the loop picks
it up with its next task.

### `/atm:plan-sprint`

**Plan a sprint with you.** A sprint is a named set of tasks the agents work through next; tasks in no sprint are
the backlog.

1. Shows the active sprint and what's left in it, the planned sprints, the backlog by label and component, and what
   the last sprint carried over
2. Agrees with you on the sprint: a new or planned one, its name, goal, release and due date
3. Goes through the candidates with you: what to pull in from the backlog, what to push back, and tasks whose
   dependencies come later
4. Starts it when you say so; `/atm:loop-tasks` then works through it

It proposes and you decide: nothing is moved, started or dated without your yes, and new tasks go through
`/atm:create-tasks`.

### `/atm:close-sprint`

**Close a sprint with you.**

1. Reviews what the sprint got done with you: the done tasks and their outcomes (follow-ups go through
   `/atm:create-tasks`)
2. Decides with you what happens to each unfinished task: carried over (the default), back to the backlog, or skipped
3. Chooses with you where carried tasks go: the next planned sprint, another one, or a new one
4. Closes it, which keeps the sprint's report (done, carried, added mid-sprint), and offers `/atm:plan-sprint` for the
   next one

### `/atm:push`

**Push the work and get CI green.**

```
/atm:push                    # push (or open/update the PR), watch CI, fix failures
/atm:push --draft            # open the PR as a draft
/atm:push --no-watch         # push and open the PR, don't watch CI
/atm:push --no-verify        # skip the local verify before pushing
/atm:push --max-rounds 5     # allow up to 5 rounds of CI fixes (default 3)
```

It follows the project's **workflow**: `direct` (push straight to the base branch) or `pull-request` (push a feature
branch and open or update its PR). Your project's `CLAUDE.md` comes first if it says how to push; otherwise the
workflow saved in `tasks.json` (`atm project workflow` shows it, `atm project workflow set direct|pull-request` changes it); if there
is none yet, `/atm:push` asks once and saves your answer.

**What it does:**

1. Checks the branch fits the workflow and nothing uncommitted belongs to someone else, and runs your verify
2. Pushes, and in `pull-request` opens the PR (body: the task commits, grouped by phase) and records it on the tasks
   (`atm vcs pr`): the dashboard shows them as **In PR**
3. Watches CI with plain `gh` calls and fixes failures by kind: test, lint and build failures are fixed and pushed
   again; budget overruns are measured and fixed, never just retried; flaky jobs are re-run once; infrastructure
   problems stop it
4. Records merged PRs on their tasks (`atm vcs pushed`): the dashboard shows them as **Merged**
5. Reports in three lines when CI is green, in detail when it stopped

**Never, without asking you:** skip or weaken a test, raise a budget, edit the CI workflow to pass, force-push or
rewrite pushed history, or merge the PR. Without `gh` or a GitHub remote it only pushes.

### `/atm:serve`

**Run the dashboard server, one for all your projects.**

```
/atm:serve              # start it (or show the one already running), and give this project's address
/atm:serve --open       # start it and open the browser
/atm:serve --all        # the overview of all projects
/atm:serve status       # is it running, and where?
/atm:serve stop         # stop it
```

The server runs in the background on `localhost`, port 8765 or the next free port, and keeps running after the
session ends. It serves the dashboard that comes with the plugin, so updating the plugin updates it: the server
restarts on the new version by itself. It shows every project atm has run in (see [All your projects](#all-your-projects)).

---

## All your projects

One dashboard shows every project you use atm in. A project is added the first time atm runs there (any skill), and
the dashboard's project selector, next to the title, switches between them. **All projects** is an overview with a
card per project: its progress, current phase, agents at work, what needs you, and recent activity. With two or more
projects the dashboard opens on it.

- **The list:** `~/.agentic-task-manager/projects.json` (set `ATM_HOME` to keep it elsewhere). `atm project list` lists the
  projects, `atm project add [path]` adds one, `atm project remove <id|path>` removes one, and `atm project prune`
  drops those whose folder is gone.
- **Opting out:** a project with `"dashboard": { "listed": false }` in its `tasks.json` meta isn't listed.
- **What the server exposes:** it listens on `127.0.0.1` only and answers only requests addressed to `localhost`. For
  each listed project it serves just three files from `.agentic-task-manager/`: `tasks.json`, `claims.json` and
  `tasks-archive.json`. Nothing else in your projects is reachable through it. The routes are documented in
  [docs/multi-project-api.md](docs/multi-project-api.md).
- **Git worktrees:** the project's main worktree is listed, and its board shows all its worktrees at once: each task
  as it is on the branch where it got furthest, with a chip naming that branch, and the agents at work in each.

---

## Project files

atm keeps everything in one folder at the root of your project (the top of the git repository):

```
.agentic-task-manager/
├── tasks.json             # the task list - committed
├── .gitignore             # written by atm - committed
├── .gitattributes         # merges tasks.json task by task (see below) - committed
├── claims.json            # who works on what, for the dashboard - not committed
└── .lock/                 # the write lock - not committed
```

The dashboard isn't in your project at all: it comes with the plugin and is served by `/atm:serve` for all your
projects.

**Upgrading from an earlier version:** projects that keep `tasks.json` in `docs/` (or the project root) are moved
automatically the first time any atm skill runs. The move uses `git mv`, so history is kept; the old
`tasks-dashboard.html` is removed from git. The change is staged for you to commit - `/atm:loop-tasks` commits it on
its own (`chore: move atm files to .agentic-task-manager/`) before its first task. Projects set up by 0.10.0–0.14.0
had their own copy of the dashboard and their own server; atm deletes the copy and stops that server the next time
it runs there (both were never committed).

---

## Task Schema

### tasks.json format

`tasks.json` is plain, indented JSON you can read, and review in a diff like any file. Nobody edits it directly:
agents, and you, change it through `atm` (see [atm-cli's commands](#atm-clis-commands)), which locks it, checks it
against the project's vocabularies and keeps the timestamps and actors right.

```json
{
  "meta": {
    "project": "My Project",
    "id": "3f9c2a1b",
    "created": "2026-04-03",
    "description": "Project description",
    "phases": [{ "id": 0, "name": "Foundations" }],
    "labels": [{ "id": "ux", "label": "UX" }],
    "workflow": { "type": "pull-request", "base": "main" }
  },
  "tasks": [
    {
      "id": "T-001",
      "name": "Short descriptive name",
      "phase": 0,
      "type": "feature",
      "priority": "P1",
      "labels": ["ux"],
      "description": "Detailed description of what to implement.",
      "requirements": ["FR-1.1"],
      "use_cases": ["UC-001"],
      "status": "done",
      "dependencies": ["T-000"],
      "files": ["src/file.ts"],
      "created_at": "2026-04-05T12:00:00Z",
      "created_by": "Hakan Serce",
      "updated_at": "2026-04-05T14:30:00Z",
      "started_at": "2026-04-05T14:22:00Z",
      "started_by": "claude:e5ea9db0",
      "completed_at": "2026-04-05T14:30:00Z",
      "completed_by": "claude:e5ea9db0",
      "commit": "a1b2c3d…",
      "pr": { "number": 12, "url": "https://github.com/me/my-project/pull/12" },
      "notes": [
        {
          "at": "2026-04-05T14:30:00Z",
          "by": "claude:e5ea9db0",
          "kind": "outcome",
          "text": "Done."
        }
      ]
    }
  ]
}
```

### Field reference

| Field                                                    | Type                     | Required | Description                                                                                                              |
| -------------------------------------------------------- | ------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------ |
| `id`                                                     | string                   | ✅       | Unique task ID (e.g., `T-001`)                                                                                           |
| `name`                                                   | string                   | ✅       | Short name, ~60 chars max                                                                                                |
| `phase`                                                  | integer                  | ✅       | Execution phase (0, 1, 2, ...)                                                                                           |
| `type`                                                   | string                   | ✅       | Task type (see below)                                                                                                    |
| `description`                                            | string                   | ✅       | Detailed description for the agent                                                                                       |
| `status`                                                 | string                   | ✅       | Current status (see below)                                                                                               |
| `dependencies`                                           | string[]                 | ✅       | Task IDs that must be done first                                                                                         |
| `requirements`                                           | string[]                 | ❌       | Requirement IDs (FR-x.x, etc.)                                                                                           |
| `use_cases`                                              | string[]                 | ❌       | Use case IDs (UC-xxx, etc.)                                                                                              |
| `files`                                                  | string[]                 | ❌       | Expected file paths to create/modify                                                                                     |
| `created_at`                                             | string                   | ❌       | ISO 8601 UTC timestamp when task was created                                                                             |
| `updated_at`                                             | string                   | ❌       | ISO 8601 UTC timestamp of last status change                                                                             |
| `started_at`                                             | string                   | ❌       | ISO 8601 UTC timestamp when the task moved to `in_progress`                                                              |
| `completed_at`                                           | string                   | ❌       | ISO 8601 UTC timestamp when the task moved to `done`                                                                     |
| `priority`                                               | string                   | ❌       | `P0` (critical) … `P3` (low), or the project's own; none counts as the default, `P2`                                     |
| `labels`                                                 | string[]                 | ❌       | Tags that group tasks across phases and types, e.g. `spike-2`, `ux` (lowercase, digits, `-`, `_`)                        |
| `created_by`, `updated_by`, `started_by`, `completed_by` | string                   | ❌       | Who did it: an agent (`claude:e5ea9db0`) or a person (the git user name). `started_by` replaces the older `agent`        |
| `review_requested_at`                                    | string                   | ❌       | When the task asked for a review (`atm task state <id> in_review`); its work ended there                                                    |
| `waiting_on`, `questions`, `blocked_at`                  | string, string[], string | ❌       | A blocked task: who decides (`user` or `agent`), the open questions, since when                                          |
| `notes`                                                  | object[]                 | ❌       | `{at, by, kind, text}`: handoffs, outcomes, review requests, rejections, failures, answers (at most 500 characters each) |
| `commit`                                                 | string                   | ❌       | The SHA of the task's commit (`atm vcs commit`)                                                                              |
| `pr`, `merged_at`                                        | object, string           | ❌       | The pull request that contains the task (`{number, url}`) and when it was merged                                         |

atm maintains the timestamps and actors: `started_at`/`started_by` when a task starts, `completed_at`/`completed_by`
when it is done, and both are removed if a task goes back to `todo`. The dashboard uses them for completion history
and estimates; files without them still work (`updated_at` is used instead). Older files' `agent` and
`last_failure` fields are still read.

**Vocabularies.** A project can name its own statuses, phases, task types, priorities and labels in `meta`
(`/atm:create-tasks` asks you). Each status has a **kind** - `todo`, `active`, `review`, `blocked`, `done` or
`skipped` - and atm and the dashboard work by kind, so `awaiting_user` and `awaiting_agent` can both be review
statuses, say. **About** in the dashboard shows the vocabularies a project uses.

### Task types

| Type       | Color     | Description            |
| ---------- | --------- | ---------------------- |
| `feature`  | 🟢 Green  | New functionality      |
| `bugfix`   | 🔴 Red    | Bug fix                |
| `test`     | 🟣 Purple | Test writing           |
| `infra`    | 🟠 Orange | Infrastructure/tooling |
| `docs`     | 🔵 Cyan   | Documentation          |
| `refactor` | 🟡 Yellow | Code restructuring     |
| `polish`   | 🩷 Pink   | UI/UX improvements     |

### Task statuses

| Status        | Kind      | Description                                                                                                                                   |
| ------------- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `todo`        | `todo`    | Not started                                                                                                                                   |
| `in_progress` | `active`  | Currently being worked on                                                                                                                     |
| `in_review`   | `review`  | Finished, waiting for approval by you or another agent (`atm task state <id> in_review`; you decide it with `atm task review approve` or `request-changes`)              |
| `blocked`     | `blocked` | Waiting on a decision: the agent asked (`atm task state <id> blocked --on user\|agent --question ...`), whoever decides answers with `atm task state <id> todo --answer ...` |
| `done`        | `done`    | Completed and verified; only done tasks let their dependents start                                                                            |
| `skipped`     | `skipped` | Intentionally skipped                                                                                                                         |

A done task can be further along, as **stages** derived from `pr` and `merged_at`: **In PR** and **Merged**. They
never hold dependent tasks up.

### The archive

Finished tasks pile up: at 10,000 tasks, `tasks.json` is several megabytes and slow for everyone to read. `atm
archive` moves done and skipped tasks older than two weeks (and not in an open PR) to `tasks-archive.json` next to
it. atm suggests it when `tasks.json` passes 1 MB or 500 archivable tasks (`meta.archive` changes the thresholds,
`"auto": false` stops agents from doing it on their own), and the agents archive at a break between tasks, in a
commit of its own. Archived tasks still count as done for dependencies, `atm task details` and `atm task list --archived` read
them, and `atm db unarchive <id>` brings one back. The dashboard loads the archive only when a view needs history.

---

## Dashboard

The dashboard is a single self-contained HTML file, served by atm for all your projects. It reads each project's
`tasks.json` from the server and renders an interactive view. Task text, Help and What's new are rendered with
[CompMD](https://compmd.hakanserce.com/) (Markdown with components), which comes with the plugin and is served by the
same local server, loaded the first time a page shows Markdown; nothing is fetched from anywhere else.

### Features

- **All your projects** - A selector next to the title switches projects (a badge counts what needs you in the others); **All projects** shows a card per project with its progress, current phase, agents at work, what needs you (click it for that project's Kanban, filtered) and recent activity. **Active only** hides projects with nothing open and nothing done in two weeks
- **Top bar** - Stays in view while you scroll. A progress ring (click for completion overall, by phase and by type) and an activity sparkline (click for tasks completed by type, agents working, time per task and idle stretches, over a period you pick: hour, day, week, month, all time or custom, with ‹ › to step back); reload, auto-refresh, mode and theme menus on the right
- **Needs you** - The header counts the tasks waiting for you: in review, or blocked on a question for you. Click it for the Kanban board
- **Search and filters** - Keyword search over ID, name, description, type and labels, and filters by phase, type, status, priority, stage, label and who started the task (multi-select) in one line that shows what's selected and how many tasks match. Every filter button shows how many tasks it would show given the other filters, and empty choices are dimmed. **Include archived** adds the archived tasks
- **Rich task text** - Descriptions, notes and questions render as [CompMD](https://compmd.hakanserce.com/): tables, task lists, alerts, layout components, ` ```mermaid ` diagrams (drawn in a sandboxed frame, in light or dark), and the task's own values (`{{md.data.task.status}}`)
- **Detail panel** - Click a task to see its full description, dependencies (an archived one shows as "archived, done"), files and timestamps, who created, started and completed it (and who tried before), its notes newest first, its commit, and a blocked task's questions with how to answer them
- **Priorities, labels and stages** - Each card shows its priority, its labels as small chips, and its stage once done: **PR #12** (linking to the pull request) or **Merged**
- **Views** - Tabs for the Dependency Graph, Kanban Board, Task List, Agents and Progress; the view fills the rest of the window and the selected tab is kept in the URL
- **Dependency graph** - Interactive SVG graph with topological layout. Tasks outside the search and filters are hidden or, if you choose, greyed out. Opens at a readable scale centered on the active task; drag to pan, Ctrl/⌘ + scroll (or pinch) to zoom, zoom/fit/1:1 buttons, keyboard shortcuts (`+` `-` `0` `F`, arrows) when focused, and a clickable minimap when the graph doesn't fit. Filtering and auto-refresh keep your zoom level
- **Kanban board** - To Do / In Progress / In Review / Blocked (when used) / Done columns, the same in every workflow; to-do cards show whether they're ready or what they wait on, and **Next up** marks the task `/atm:loop-tasks` will pick next. In Review cards say how long they have waited; Blocked cards say whether they wait on you or an agent, for how long, and how many questions. In a pull-request project, Done is split into In PR (per pull request), Done and Merged. Optional swimlanes per phase
- **Task List** - Task cards, color-coded by status and type, with dependency links
- **Agents** - A timeline with a lane per agent (🤖) or person (👤) and a bar per task (start to completion or to the review request, colored by type; work in progress runs to now). A green mark at the end of a bar means someone else completed it, usually a reviewer. Zoom with the buttons, Ctrl/⌘ + scroll or "last hour" presets, drag to move in time, or pick a range on the overview; **Live** keeps it at the current time. Tasks outside the search and filters are greyed out. Agents are recorded by `atm` since 0.12.0; older tasks show as "Unattributed"
- **Progress** - Burndown chart (remaining vs. scope, with a forecast) and per-phase completion estimates in agent working time; click a phase to focus the chart. Agent utilisation compares agent work time with calendar time (24/7) and working time (8 h, Monday to Friday), overall and per agent. Estimates are most accurate with `started_at`/`completed_at`, which `/atm:do-task` and `/atm:loop-tasks` write since 0.7.0
- **Saved view per project** - Filters and search, the tab, each tab's options (Kanban, Progress focus, graph hide/grey out) and the zoom of the graph and the Agents timeline come back when you reopen the dashboard. Each project keeps its own (by `meta.id` in `tasks.json`); **Reset view** clears it. A `#view=` link still opens that tab
- **Light and dark mode** - Follows your system by default; switch between System, Light and Dark in the menu at the top right
- **Themes** - Default, Nord, Solarized, Catppuccin, Tokyo Night, Ayu, Night Owl, Gruvbox, Noctis, Sublime Material and High contrast, each in light and dark, checked for readable contrast (see [docs/theme-validation.md](docs/theme-validation.md)). Theme and mode are remembered per browser
- **Live claims** - In-progress tasks show which agent works on them, on which branch, and since when
- **Auto-refresh** - On by default every 5s; the toggle and interval (in the menu next to the reload button) are remembered per browser. A poll that finds `tasks.json` unchanged transfers and redraws nothing (`304 Not Modified`)
- **The archive** - The header counts the archived tasks, and the footer says when `tasks.json` has grown big enough to archive. The archive is loaded only for the Progress tab, the activity details and **Include archived**
- **What's new, Help and About** - The version next to the title opens the release notes (a "new" marker appears after an upgrade); the footer's **Help** explains the dashboard, and **About** shows the plugin and the project's vocabularies and workflow
- **Manual reload** - Click reload button for instant refresh

### Starting the dashboard

Run `/atm:serve --open` (see [Skills Reference](#atmserve)). The dashboard needs atm's server: it loads the list of
projects and their files from it, so opening the HTML file another way doesn't work.

### Watching live progress

1. Start the dashboard (auto-refresh is on by default, every 5s)
2. In another terminal, start Claude Code and run `/atm:loop-tasks`
3. Watch tasks move from todo → in_progress → done in real time

---

## Git Push Strategies

**Workflows.** A project pushes its work in one of two ways, saved in `tasks.json` (`atm workflow set
direct|pull-request`; `/atm:create-tasks` and `/atm:push` ask once): **direct** pushes to the main branch,
**pull-request** pushes a branch and opens or updates its pull request. Your project's `CLAUDE.md` comes first if it
says how to push. Either way, `/atm:push` watches CI and fixes failures; see [`/atm:push`](#atmpush).

The `/atm:loop-tasks` skill supports flexible git strategies (its pushes go through `/atm:push`):

| Strategy              | Command                          | When to use                           |
| --------------------- | -------------------------------- | ------------------------------------- |
| Commit only (default) | `/atm:loop-tasks`                | Local history, push manually later    |
| No git                | `/atm:loop-tasks --no-commit`    | You want full manual control          |
| Commit + push         | `/atm:loop-tasks --push`         | Real-time backup, CI after every task |
| Batch push            | `/atm:loop-tasks --batch-push`   | Clean remote history, single CI run   |
| Periodic push         | `/atm:loop-tasks --push-every 5` | Balance: CI every 5 tasks             |
| Human-in-the-loop     | `/atm:loop-tasks --pause --push` | Review each task before pushing       |

---

## Tips & Best Practices

### Ask the agents

You can use natural language to trigger the plugin skills: just say let's plan a sprint for instance, instead of /atm:plan-sprint. If they don't understand mention atm and ask them to read the skills once. Or ask for your agent to help fix, if you need to fix something with the tasks or project.

For instance, if you restructured your project and all file references are now stale, just ask the agent to go through each task and fix the references. With a project with hundreds of tasks, this takes only a minute or two.

### Task granularity

Each task should be completable by a single agent in under 10 minutes. If a task description is longer than a paragraph, it should probably be split.

### Dependencies

Use dependencies to enforce execution order. A task won't be picked up until all its dependencies are `done`. This prevents issues like "write tests for code that doesn't exist yet."

### Software Development Process

If you are familiar and comfortable with Scrum or agile software development processes, follow a clean Backlog, Sprint separation. Add tasks to a Backlog. Ask your agent to plan a sprint, they will recommend tasks that relate together to create a sprint with a recommended name, goal, etc.

If you rather don't bother with all this, just go straight into creating your tasks and starting them directly. Using sprints give you a lot more flexibility, focus and visibility, though. I'd recommend trying it out.

### Manual intervention

You can change tasks while the loop runs: ask an agent in another session, or run `atm` yourself, which locks the
file so your change and an agent's can't overwrite each other. Changes are picked up on the next iteration. Use this
to:

- Skip a task that's no longer needed (`atm task state <id> skipped --reason '…'`)
- Block a task so the agent doesn't pick it up (`atm task state <id> blocked --on user --question '…'`)
- Add new tasks mid-loop (`/atm:create-tasks`)
- Change a task's description or dependencies (`atm task edit <id> --description … --add-dep …`)

### Multiple agents

See [Running agents in parallel](#running-agents-in-parallel).

---

## Running agents in parallel

You can run several `/atm:loop-tasks` (or `/atm:do-task`) sessions on one project at the same time. atm makes sure
they don't take the same task or overwrite each other's changes to `tasks.json`.

**Recommended setup: one git worktree per agent.** Agents that edit code in the same folder step on each other's
files and builds, which atm can't prevent. Give each agent its own worktree and branch from the same clone:

```bash
git worktree add ../myproject-agent2 -b agent2
cd ../myproject-agent2 && claude      # then /atm:loop-tasks
```

Merge each branch back as usual (or open a PR per branch).

**What atm guarantees**

- **One writer at a time.** Every status change goes through the plugin's helper (`atm`), which takes a lock,
  re-reads `tasks.json`, changes one task and writes the file atomically. The dashboard and other agents never see a
  half-written file, and two changes never overwrite each other.
- **Claims.** `atm loop next --claim` picks the next task and claims it in one step. Claims record the agent (each Claude
  Code session has its own identity, and so does each worktree a session's subagents work in, e.g.
  `claude:e5ea9db0@agent-2`), branch and worktree. They are stored in the clone's `.git` directory, so agents
  in different worktrees of the clone see each other's claims straight away. A task finished, blocked or sent for
  review in one worktree isn't handed out again in another before the branches merge, and a block or a review can be
  answered from any worktree: the one where the task waits picks the answer up with its next atm command. The
  dashboard shows every worktree's tasks and claims on the project's board, before anything merges.
- **Leases.** A claim lasts two hours (`"lease_minutes"` in `tasks.json`'s `meta` changes the length). Every atm
  command the agent runs renews it, and so does the plugin's hook after each tool call. A long command (a full test suite, a build) runs after
  `atm claim hold <id>` in the same shell (`atm claim hold T-1 && npm test`): atm runs nothing, it keeps the claim
  alive while that shell runs. If an agent crashes, its task goes back to
  `todo` when the lease runs out, with a failure note saying whose lease expired.
- **Merging `tasks.json`.** A git merge driver (registered automatically) merges `tasks.json` task by task. Status
  changes from different branches combine (the further-along status wins, e.g. `done` over `in_progress`). Tasks
  added on both branches are both kept, and the incoming ones are renumbered if their IDs collide. Only conflicting
  edits to the same task's definition still need you.
- **Clean commits.** The skills commit only the files a task changed plus `tasks.json`, never `git add -A`, so a
  commit doesn't carry another agent's unfinished work.
- **Live view.** In-progress tasks on the dashboard show who works on them, on which branch, and since when.

**What it doesn't do**

- Two agents in the **same** working tree can still edit the same source files. Use worktrees.
- Agents on **different machines or clones** don't see each other's claims; they may start the same task, and the
  merge driver then reconciles the statuses.

### atm-cli's commands

`atm` (atm-cli) is the command the skills run, and you can too: run it from the project. Claude Code puts the
plugin's `bin/atm` on its Bash tool's PATH; in your own terminal, use the full path
(`~/.claude/plugins/cache/solo-swe-plugins/atm/<version>/bin/atm`) or an alias. Every command takes `--json`, and
`atm help` lists them all.

| Command                                                                                                   | What it does                                                                                                                                                                              |
| --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `atm project status`                                                                                              | The overview: counts, what is in progress, in review or blocked, what's next                                                                                                              |
| `atm task details <id>...`, `atm task deps <id>`                                                                       | Tasks in full (with notes); what a task waits for and what waits for it                                                                                                                   |
| `atm task list [--status S] [--sprint S] [--backlog] [--label L] [--ready] [--blocked] [--in-pr] [--archived]` | One line per task                                                                                                                                                                         |
| `atm loop next [--sprint S \| --tasks T-1,T-5 \| --all \| --backlog] [--claim]`                                | The task the loop would take next (and claim it); the active sprint by default                                                                                                            |
| `atm sprint list \| add \| edit \| start \| close`, `atm sprint move <id>... --to <S>`, `atm sprint backlog`                 | Sprints, moving tasks between them and the backlog                                                                                                                                        |
| `atm task state <id> in_progress \| done \| todo \| skipped`                                                                     | Change a task's status (`--note` for an outcome or handoff; back to `todo` with `--reason` when it failed, or to reopen it)                                                                                                        |
| `atm task state <id> in_review --note T`, `atm task review approve \| request-changes \| reject <id>`                                                  | Ask for a review; decide it (yours: no skill does it on its own). A task leaves review only by its decision                                                                                                                                               |
| `atm task state <id> blocked --on user\|agent --question Q`, `atm task state <id> todo --answer A`                             | Ask for a decision; answer it                                                                                                                                                             |
| `atm task note <id> TEXT`                                                                                      | Add a note to a task                                                                                                                                                                      |
| `atm task add`, `atm task edit <id>`, `atm doctor`                                                                   | Add tasks (one, or many with `--json-file`), change a task's definition, check the file                                                                                                   |
| `atm vcs commit <id> --file F...`                                                                             | Commit exactly those files plus `tasks.json`, with the project's commit message                                                                                                           |
| `atm project workflow [set direct\|pull-request]`                                                                 | Show or set the project's workflow                                                                                                                                                        |
| `atm db archive`, `atm db unarchive <id>...`                                                                    | Move finished tasks to `tasks-archive.json`, or bring them back                                                                                                                           |
| `atm claim list`, `atm claim heartbeat <id>`                                                                        | Who works on what, with time left on each lease; extend your claim's lease                                                                                                                |
| `atm db init`                                                                                                | Register the `tasks.json` merge driver (the skills do this automatically)                                                                                                                 |
| `atm service [start\|stop\|status] [--open] [--all]`                                                        | The dashboard server, one for all your projects                                                                                                                                           |
| `atm project list \| add [PATH] \| remove <id\|PATH> \| prune`                                                 | The projects the dashboard lists                                                                                                                                                          |
| `atm vcs repository [URL [--from OLD]]`                                                                   | The repository's web address, for commit and PR links (`atm db init` takes it from `origin`); after a move, setting the new one also moves the tasks' recorded PR links from the old address |

The design is in [docs/v4/atm-cli.md](docs/v4/atm-cli.md).

The full analysis (scenarios, failure modes, the alternatives considered) is in
[docs/parallel-agents-design.md](docs/parallel-agents-design.md).

**Fewer permission prompts.** The skills run plain `atm …` and pre-approve the commands they use while they run
(their `allowed-tools`, one command at a time), so you don't need to do anything for them. That approval ends with
your next message, though: if you also want atm's reads approved when you ask Claude for something outside a skill
("what's the status?"), add rules for them to your project's `.claude/settings.json` (or `~/.claude/settings.json`);
they keep working across plugin upgrades:

```json
{
  "permissions": {
    "allow": ["Bash(atm project status:*)", "Bash(atm task list:*)", "Bash(atm task details:*)", "Bash(atm task deps:*)"]
  }
}
```

Don't allow `Bash(atm *)` or a whole group: `atm task review` (approving, rejecting or asking for changes) is yours to
decide, and an agent should have to ask before it runs one.

---

## Project Structure

```
agentic-task-manager/
├── .claude-plugin/
│   └── marketplace.json         # Self-hosted marketplace config
├── plugin/                      # The plugin (the only part that ships)
│   ├── .claude-plugin/
│   │   └── plugin.json          # Plugin manifest (version source of truth)
│   ├── skills/
│   │   ├── create-tasks/SKILL.md      # /atm:create-tasks
│   │   ├── do-task/SKILL.md           # /atm:do-task
│   │   ├── loop-tasks/SKILL.md        # /atm:loop-tasks
│   │   ├── push/SKILL.md              # /atm:push
│   │   └── serve/SKILL.md             # /atm:serve
│   ├── scripts/
│   │   ├── atm                  # Helper launcher: checks for Python 3.9+, runs atm.py
│   │   ├── atm.py               # The helper (built from atm-cli/)
│   │   └── vendor/              # Typer and Click, which its command line uses
│   ├── templates/
│   │   ├── tasks.json           # Example tasks.json schema
│   │   └── tasks-dashboard.html # Dashboard (build output of dashboard/, served by atm serve)
│   └── CLAUDE.md                # Plugin context for Claude Code
├── dashboard/                   # Dashboard source: Svelte 5 + TypeScript (development only)
├── .agentic-task-manager/       # This repo's own atm task list (we dogfood atm)
├── docs/                        # Requirements, designs and plans
├── .githooks/pre-commit         # Rebuilds the dashboard when its sources are committed
├── .github/workflows/ci.yml     # Lint, unit, build-freshness and e2e checks
├── CLAUDE.md                    # Development notes for Claude Code
├── README.md                    # This file
└── LICENSE                      # MIT
```

---

## Development

The dashboard is developed in `dashboard/` (Svelte 5, TypeScript, Vite) and built into a **single self-contained HTML
file** at `plugin/templates/tasks-dashboard.html`. That file is committed, because the plugin installs straight from
git with no build step.

It uses [pnpm](https://pnpm.io/installation) (installed with its standalone installer, not through corepack):

```bash
cd dashboard
pnpm install --frozen-lockfile   # also enables the pre-commit hook (git core.hooksPath = .githooks)
pnpm run dev                     # Vite dev server
pnpm run verify                  # check + unit tests with coverage + build + e2e - run before committing
```

| Script                                 | What it does                                                                                  |
| -------------------------------------- | --------------------------------------------------------------------------------------------- |
| `pnpm run build`                       | Builds `plugin/templates/tasks-dashboard.html`; the version meta tag comes from `plugin.json` |
| `pnpm test` / `pnpm run test:coverage` | Vitest unit and component tests (`tests/unit/`), 90% line threshold on `src/lib`              |
| `pnpm run test:e2e`                    | Build, then the Playwright regression suite (`tests/e2e/`)                                    |
| `pnpm run check`                       | Version consistency, svelte-check, ESLint, Prettier                                           |

**Tests.** e2e specs play atm serve's API with fixtures from `tests/fixtures/` as each project's `tasks.json`, and
fail on any external request.
They run against the build, with strict screenshot baselines per platform; CI runs them in the pinned Playwright
container and generates any missing Linux baselines for you to commit.

**Build artifact.** Never edit `plugin/templates/tasks-dashboard.html` by hand. The pre-commit hook rebuilds and stages
it when dashboard sources change, and CI fails if the committed file doesn't match a fresh build.

**Releasing.** Bump `version` in both `plugin/.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json`
(`pnpm run check` enforces that they match) and rebuild, so running dashboard servers restart on the new dashboard.

---

## Future Vision

### Multi-tool support

`tasks.json` is plain JSON - any tool that can read/write files can participate. Future adapters planned for:

- **OpenAI Codex CLI**
- **Cursor** (via `.cursorrules`)
- Open Code
- Pi
- ...

### Multi-machine coordination

Claims shared between clones on different machines, through refs on the shared remote (see the design doc).

### External tool sync

MCP servers to sync `tasks.json` bidirectionally with GitHub Issues, Linear, or Jira.

### AI task estimation

Optional `estimated_minutes` and `actual_minutes` fields with burndown chart in the dashboard.

---

## Contributing

Contributions welcome! Please open an issue or PR on [GitHub](https://github.com/solo-swe/agentic-task-manager).

## License

MIT - see [LICENSE](LICENSE).

## Author

**Hakan Serce** - [blog.hakanserce.com](https://blog.hakanserce.com) · [GitHub](https://github.com/hakanserce)
