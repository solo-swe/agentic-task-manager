This dashboard shows the tasks of every project you use atm in, from each project's
`.agentic-task-manager/tasks.json`, live: it reloads every 5 seconds while agents work through the tasks (and does
nothing when nothing changed). It only reads the files; agents and the `atm` command change them.

Jump to: [your projects](#your-projects), [the views](#views), [filters and saved views](#filters-and-saved-views),
[statuses](#statuses-and-kinds), [waiting on you](#waiting-on-you), [priorities and labels](#priorities-and-labels),
[the task details](#the-task-details), [workflows and stages](#workflows-and-stages), [the archive](#the-archive),
[the skills](#the-skills), [agents in parallel](#running-agents-in-parallel).

## Your projects

- **The arrow next to the title** switches projects. Type to filter the list; a bar shows each project's progress and
  a dot marks the ones with something waiting for you. A badge on the arrow counts what waits for you in the other
  projects.
- **All projects** (first in that list) shows a card per project: its progress, the phase it is in, the agents at work,
  what needs you and when it last moved. Click a card to open the project, or its "need you" to see those tasks on its
  Kanban board. **Active only** hides projects with nothing open and nothing done in the last two weeks.
- A project appears here once atm has run in it. `atm projects` lists them, and `atm projects remove` takes one off.
- Updating the plugin updates this dashboard: when a new version is ready, a bar offers to reload.

## The top bar

- **Progress** — the share of tasks done. Click it for the split by status, phase and type.
- **Activity** — tasks completed over time. Click it for the details: completions by type, agents at work, time per
  task and idle stretches, for a period you choose.
- **N need you** — tasks waiting for your review or for your answer. Click it for the Kanban board.
- **Refresh** — reload now, or change how often the dashboard reloads by itself.
- **Appearance** — light or dark mode, and one of 11 themes, picked from a drop-down list.

## Views

- **Dependency Graph** — tasks as boxes, in the order their dependencies allow. Drag to move, use the buttons or
  Ctrl/⌘ + scroll to zoom. Click a task for its details.
- **Kanban Board** — a column per status. To Do shows which tasks are ready and which one the loop takes next.
  Group the board by phase, or show skipped tasks, with the options above it.
- **Task List** — every task as a card, in the file's order or by priority.
- **Agents** — who worked on what, and when: a lane per agent (🤖) or person (👤), with a bar per task. A green mark
  at the end of a bar means someone else completed it, usually a reviewer approving it. Zoom and move the time range,
  or tick **Live** to follow the current time.
- **Progress** — each phase's progress and an estimate of the agent work left, a burndown chart, and how busy the
  agents have been.

## Filters and saved views

**Filters** narrows every view by phase, type, status, priority, stage, label and who started the task (the last
three appear when the project uses them); the search box matches task IDs, names, descriptions, types and labels.
Within a group, any selected value matches; across groups, all must match. Each button says how many tasks it would
show.

The dashboard remembers your filters, search, tab and options **per project**, so they are back the next time you
open it. **Reset view** forgets them.

## Statuses and kinds

Each status has a **kind**, which decides what it means:

- **To Do** (kind `todo`) — not started.
- **In Progress** (`active`) — an agent is working on it.
- **In Review** (`review`) — the work is finished and waits for approval by you or another agent. Cards say how long
  it has waited. It doesn't count as agent work.
- **Blocked** (`blocked`) — it can't go on until someone decides something (below).
- **Done** (`done`) — finished. Only done tasks let the tasks that depend on them start.
- **Skipped** (`skipped`) — won't be done.

A project can name its own statuses in `tasks.json`; they look like the built-in status of their kind. **About**
lists the ones this project uses.

## Waiting on you

An agent that can't go on without a decision asks instead of guessing: the task is **Blocked**, and its card says
**Waiting on you** (or **on an agent**), for how long, and how many questions. The task's details list the questions.
Answer them in Claude Code, or with `atm unblock <id> --answer "…"` (one answer per question); the task goes back to
To Do with your answers attached for the next agent.

A task **In Review** waits for you to look at it: pass it with `atm done <id>`, or send it back with
`atm reject <id> --reason "…"`.

## Priorities and labels

Agents take tasks phase by phase and, within a phase, the most urgent first:

- **P0** Critical / Required
- **P1** High
- **P2** Medium (tasks without a priority count as this)
- **P3** Low / Nice to have

**Labels** are tags that group tasks across phases and types, such as `spike-2` or `ux`. They show as small chips on
the cards, and the filter and the search find them.

## The task details

Click a task for everything about it: its description, what it depends on and what depends on it, and

- **Who** — who created it, started it and completed it, and who worked on it before (from their handoffs);
- **Notes** — newest first: handoffs from an agent that had to stop, the outcome, review requests and rejections,
  your answers, and failures;
- **Commit** — the commit that holds the task's work.

Descriptions, notes and questions are written in [CompMD](https://compmd.hakanserce.com/), Markdown with components,
and render as (this Help and What's new render with CompMD too):

- **Markdown and GFM** — headings, lists (nested too), tables, task lists, code, quotes, alerts, footnotes and links;
- **Diagrams** — a `mermaid` code block draws a diagram with [Mermaid](https://mermaid.js.org/), in light or dark
  with the dashboard, in a sandboxed frame;
- **The task's own values** — `{{md.data.task.status}}` shows the task's status as it is now;
- **References** — a link to a file of the project opens it in the document viewer; a requirement, a use case or a
  sentence linked by its text (a link ending in `#:~:text=…`) opens the document there, marked;
- **As written** — math, for now, and anything the dashboard won't run: scripts, raw HTML, images from elsewhere.

A diagram is a fenced code block whose language is `mermaid`, holding the diagram's source, such as:

```
flowchart LR
  todo[To do] --> active[In progress] --> done[Done]
```

In a `mermaid` fence, it draws:

```mermaid {alt="To do, then In progress, then Done"}
flowchart LR
  todo[To do] --> active[In progress] --> done[Done]
```

## Workflows and stages

A project pushes its work in one of two ways (**About** shows which; `/atm:push` asks the first time):

- **direct** — commits go straight to the main branch.
- **pull-request** — work goes to a branch and reaches the main branch through a pull request.

A done task can be further along: **In PR** (its pull request is open; the card links to it) and **Merged**. In a
pull-request project the Done column is split into these stages. Stages never hold anything up: the tasks that
depend on a task can start as soon as it is done.

## The archive

Finished tasks pile up. `atm archive` moves old ones to `tasks-archive.json`, so `tasks.json` stays small and quick
to read; agents do it when `atm` suggests it, and the footer says when the file has grown big enough. The header
counts the archived tasks.

The dashboard reads the archive only when you need it: the **Progress** tab and the activity details count archived
work, and **Include archived** in the filter bar adds the archived tasks to the search, the lists, the graph and the
Agents tab. A task that depends on an archived one shows it as "archived, done".

## The skills

Run these in Claude Code:

- `/atm:create-tasks` — turn your requirements into tasks.
- `/atm:do-task` — do one task: the next available one, or the one you name.
- `/atm:loop-tasks` — keep doing tasks until none is left.
- `/atm:push` — push the work (or open its pull request), watch CI and fix what fails.
- `/atm:serve` — start or stop this dashboard's server, one for all your projects.

## Running agents in parallel

Several agents can work on one project at once, in one checkout or in separate git worktrees: each agent claims the
task it works on, so no two take the same task, and the Kanban board shows who works on what. The board shows every
worktree of the project at once: a task is shown as it is on the branch where it got furthest, and a branch chip on
its card names that branch when it isn't the main worktree's. See
[Running agents in parallel](https://github.com/solo-swe/agentic-task-manager#running-agents-in-parallel) in the
README.
