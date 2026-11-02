# What's new

The short version, for people using atm. The full list of changes, for developers, is in CHANGELOG.md.

## [0.26.0] - 2026-11-01

- The dependency graph keeps itself fitted as you filter, so the tasks you're looking at stay in view.
- Fit the whole graph or only the tasks that match your filters.
- The graph glides smoothly to its new view instead of jumping.
- A tidier graph toolbar, with a compact zoom control.

## [0.25.0] - 2026-11-01

- The board opens on the active sprint, with its goal, due date and progress, and hints for what to do next.
- The task list groups tasks by sprint, label or component, so planning from the backlog is one view.
- See each sprint's velocity, and how far each part of your project has come.
- Search and filters now sit in the title bar, leaving more room for your tasks.
- Share a link that opens the board on a sprint.
- Tasks reopened after their sprint closed are flagged, so no work gets lost.

## [0.24.0] - 2026-10-31

- Phases are now sprints: plan one with a goal and a due date, and agents work through it, then close it and carry what's left.
- See what each release holds and what's still missing, with a notes draft to copy when it ships.
- Choose how work is reviewed: right away, as it goes, or all together at the end of the sprint.
- Mark the work you want to do yourself with an agent, and unattended loops leave it for you.
- The board's "Next up" is always the task the agent really takes next.
- A task's details show how it ended and the tasks it's related to or duplicates.

## [0.23.0] - 2026-10-30

- The Kanban board keeps all its columns when you filter by status, so the board doesn't change shape.
- Grey out finished tasks in the dependency graph with the new "Dim done" button, to see what's left.
- The dependency graph is laid out more clearly: chains run in straight lines, with fewer crossing arrows.
- Hover a task in the dependency graph to light up everything it waits for.
- A new Sticky notes theme: your Kanban on a blackboard or a whiteboard, with tasks as colourful sticky notes.
- A new Hand-drawn theme: your Kanban sketched in pencil on a notebook page, or in white pen on a black one.

## [0.22.0] - 2026-10-27

- A task's details no longer show a scrollbar: the title stays put, and hints say when there's more above or below.
- Jump around a task's details from the new handles on the left: Info, Dependencies, Notes, Activity and Ref.
- Dependencies in a task's details fit on one line each.
- What's new scrolls the same way, without a scrollbar.

## [0.21.0] - 2026-10-27

- atm moved to github.com/solo-swe/agentic-task-manager. Installed it before? Remove the old marketplace and add the new one, as the README explains.
- Shorter titles: "Projects Overview" for all projects, and just the project's name on each project.
- The Activity details no longer mix up the period you picked with the size of the bars.
- A tidier Project tab: no extra line under the name, and its two panels line up.
- The board of all your projects is now the Omniboard. It fits your window, and long columns scroll on their own, like a project's Kanban.

## [0.20.0] - 2026-10-27

- All projects has a Board tab: what's being worked on, waiting for you and just finished, across every project, with a search box.
- A new Project tab shows who works on a project, its vocabulary, and stats such as agent time and how long tasks take.
- Activity and Progress now show where the time went: agents working, waiting on you, or idle.
- The Kanban opens first and no longer makes the page scroll; long columns show how many more cards are above or below.
- atm is hardened against hostile task files and repositories, and agents treat task text as information, not commands.

## [0.19.0] - 2026-10-26

- Notes, questions and answers show formatting: code, lists, bold text and links.
- The Kanban uses your whole screen height, so you see more cards before scrolling.
- Long or many labels on a Kanban card stay on one line; click "+N" to see them all.
- With many phases, the Phase filter shows only the active ones, with a button for the finished ones.
- The About dialog names the project you're looking at, with its logo.
- The Idle number in Activity fits on one line, and its colour now matches the charts.

## [0.18.0] - 2026-10-26

- Project cards show what agents are working on right now, by task name, or what comes next.
- Each card shows its branch and latest pull request, with a link.
- See the last task each project finished, and how long ago.
- The cards are laid out the same way, so a row of projects lines up neatly.

## [0.17.0] - 2026-10-26

- Your projects can have logos: on their cards, in the project selector and next to the title.
- Project cards show rings for the phases you're working on, and line up neatly.
- "Needs you" takes you straight to the task waiting for you, or lists them.
- "Active phases only" now shows what's in progress plus the phase you just finished, and is on by default.
- Agents working in parallel no longer take each other's tasks, and a question answered once counts everywhere.

## [0.16.0] - 2026-10-26

- Agents working in separate git worktrees all show on one board, each task with the branch it's on.
- Commits and pull requests in a task's details link to GitHub, GitLab, Bitbucket or Codeberg.
- Progress leaves out skipped tasks, so a phase with only skipped tasks left counts as finished.
- Task details show when work started, when it finished and how long it took.
- When an agent's time on a task ran out, it now says so and how to carry on.

## [0.15.0] - 2026-10-25

- One dashboard now shows all your projects: switch between them with the arrow next to the title.
- **All projects** shows every project at a glance: its progress, what it's working on, and what needs you.
- The dashboard comes with the plugin, so updating the plugin updates it. No more updating each project.
- When a new version is ready, the open dashboard offers to reload.
- Open it with `/atm:serve`, which you only need once for all your projects.

## [0.14.0] - 2026-10-25

- Five more colour themes: Ayu, Night Owl, Gruvbox, Noctis and Sublime Material, each in light and dark.
- Pick a theme with the arrow keys; the menu opens on the one you use.
- The Progress tab can show only the phases you're working on, hiding ones finished a while ago.
- Task descriptions show formatting: headings, lists, code and links.
- Activity titles say which period you picked, such as "this week".
- What's new (this list) is now short and written for you; the full details stay in the changelog.

## [0.13.0] - 2026-10-24

- Agents can ask you a question instead of guessing, or hand finished work to you for review. The dashboard tells
  you when a task needs you.
- Push your work and get CI green with `/atm:push`, straight to your main branch or through a pull request.
- Tasks can have priorities and labels, and your project can name its own statuses, phases and types.
- Each task shows who created, started and finished it, notes from the agents, and its pull request.
- Old finished tasks can be archived, so big projects stay fast.
- Help and About in the footer explain the dashboard.

## [0.12.0] - 2026-10-24

- Light and dark mode, and six colour themes.
- A new top bar with your progress and recent activity; click either for the details.
- Search tasks by keyword, and see how many tasks each filter would show.
- An Agents tab shows who worked on what, and when.
- The dashboard remembers your filters, tab and view for each project.

## [0.11.0] - 2026-10-23

- Several agents can work on one project at once without getting in each other's way.
- The dashboard shows which agent is working on each task.
- Branches that changed different tasks now merge cleanly.

## [0.10.0] - 2026-10-23

- `/atm:serve` starts the dashboard for you in the background.
- Everything atm needs now lives in one folder in your project, `.agentic-task-manager/`.
- atm now needs Python 3.9 or newer, which macOS already has with its developer tools.

## [0.9.0] - 2026-10-23

- A Progress tab with a burndown chart and an estimate of the work left in each phase.

## [0.8.0] - 2026-10-23

- A Kanban board, with a "Next up" marker on the task the agent will pick next.

## [0.7.0] - 2026-10-22

- This What's new panel.
- The dashboard refreshes itself every 5 seconds by default.

## [0.6.0] - 2026-10-22

- Zoom and pan the dependency graph, with a minimap for large ones.

## [0.5.0] - 2026-10-22

- A rebuilt dashboard that looks the same, and is now tested automatically.

## [0.4.0] - 2026-04-05

- An activity timeline, and `/atm:update-dashboard` to get the latest dashboard.

## [0.3.0] - 2026-04-04

- Internal changes only.

## [0.2.0] - 2026-04-04

- The plugin is now called `atm`: `/atm:create-tasks`, `/atm:do-task`, `/atm:loop-tasks`.

## [0.1.0] - 2026-04-03

- First release: turn requirements into tasks, let agents work through them, and watch it all on a live dashboard.
