# atm's terminal output: alignment, grouping and styling (T-488)

- **Status:** proposal, for review with Hakan
- **Task:** T-488
- **Scope:** the plain output (no `--json`) of `atm project status`, `atm task list`, `atm task details`,
  `atm sprint list`, `atm release list` and `atm doctor`. Then, once these six are agreed, the other commands
  follow the same rules. `--json` doesn't change.

## 1. The problem

atm's plain output was written one fact per `println!`, and it shows:

- `project status` packs the per-sprint, per-type and per-label counts into one comma-separated line each, 30
  entries long. The sprint line isn't even in sprint order. Lists of tasks (blocked, waiting) are joined with `;` on
  one line.
- `task list` lines up its first columns only by luck (the status is padded, the labels aren't). The labels sit
  between the priority and the name, so the names start at a different column on every line.
- `sprint list --all` and `release list` repeat their words ("released · released 2026-10-31"). The releases are in
  no useful order: 0.24.0 comes before 0.5.0.
- `doctor` prints 32 lines that say the same thing about 32 tasks.
- Nothing is bold or coloured. In a terminal everything has the same weight, so the one blocked task looks like
  the 300 done ones.

## 2. Rules

1. **Same words to a terminal and to a pipe.** A terminal and a pipe or file get the same lines, the same words in
   the same order. Only two things differ: colour, and cutting long lines to the terminal's width. Agents run atm
   with standard output piped, so they get the full text, uncut and plain, which is what the scenarios check.
2. **Colour only on a terminal**, and never with `NO_COLOR` set, `TERM=dumb`, `--json`, or output going to a pipe or
   file. A new global `--color auto|always|never` (default `auto`, as git and cargo have) overrides the check, and
   `--color always` is how the tests see the styled output. `CLICOLOR_FORCE` works as it does for clap.
3. **One fact per line, findable by its words.** A list is a heading with one indented line per item. No
   `a, b, c` runs of things an agent would act on, such as tasks. Short runs of counts may stay on one line.
4. **Columns padded to the widest value** in the list (IDs, statuses, priorities). The free text (a name) goes
   last but one, and secondary text (labels, who) goes last, dimmed.
5. **Cut, never wrap, in tables.** On a terminal, a name too long for the width is cut with `…`. A pipe never cuts.
   Descriptions and notes in `task details` are wrapped to the width on a terminal, and left as written in a pipe.
6. **What agents match stays exactly as it is:**
   - the lines the skills relay or look for: "Nothing to migrate.", "No tasks.json found", "Now ready:",
     "`atm db archive` would move …", "Released …", "your lease expired at …", `next:`'s reason;
   - error and warning prefixes (`error:`, `warning:`);
   - exit codes.

## 3. Styling

| What | Style |
|---|---|
| Section headings ("In progress", "Blocked"), a task's ID and name in `details` | bold |
| Status, by **kind** (a project's own statuses get their kind's colour) | `todo` plain · `active` blue · `review` magenta · `blocked` yellow · `done` green · `skipped` dimmed |
| Priority, by **position** in `meta.priorities` (works for a project's own) | first red bold · second yellow · the default plain · the last dimmed |
| Secondary text: labels, agents, timestamps, sizes, hints ("`atm … --all` shows …") | dimmed |
| `error:` / `warning:` | red bold / yellow bold |
| A provisional key (`~ATM-538`) | the `~` dimmed |

No background colours, and nothing that only works on a dark theme: the 8 standard colours, which every terminal
theme maps to something readable.

## 4. Before and after

The "after" blocks show the text as a pipe gets it. On a terminal it's the same, with the styles above, and lines
cut to the width (`…`).

### `atm project status`

It leads with the active sprint, what is being worked on, what waits, and what's next. The project-wide counts come
after them, in short. The breakdowns by sprint, type and label move behind a new `--all`, and a dimmed line says so.

Before:

```text
sprint: Keys and reach (active): 12 of 15 done · release 0.38.0
  goal: Design task keys like ATM-123 with dependencies across projects, atm in every agent harness, readable CLI output, and live updates in the browser
339 tasks: todo 6, in_progress 1, blocked 2, done 328, skipped 2
by sprint: Insights, stats and security review 13, Security fixes 17, Progress and insights 23, Org move and fixes 12, Maintenance 2, Overview and polish 30, Testing together 4, Themes 1, Rich Markdown 8, Sprints and releases 36, Sprints on the dashboard 6, backlog 9, Layered Architecture 29, Document Viewer 14, Viewer fixes 6, Keys and reach 15, Rust and distribution 7, CLI ergonomics 20, The Rust port 31, The atm app 20, The atm app, continued 8, Updates and renames 17, The desktop app 11
by type: bugfix 45, docs 23, polish 31, feature 122, infra 36, test 13, refactor 69
by label: activity 12, security 23, interactive 18, progress 9, stats 9, overview 22, kanban 9, filters 1, org-move 3, archive 9, ux 18, theme 1, sprints 40, compmd 18, code-review 1, git 1, testing 1, cli 16, architecture 38, docs 2, atm2 13, atm-cli 1, distribution 24, svelte-guide 10, dashboard 18, rust 38, app 42, api 1, desktop-ui 16, task-keys 10
in progress: T-488 Overhaul atm's human-readable output: alignment, grouping and styling (claude:0f56a916)
blocked: T-413 Support the harnesses beyond Claude Code through atm agents install (on user); T-507 The browser reacts to changes too: atm serve pushes them, and the refresh settings go (on user)
waiting on dependencies: T-482 Windows: the rest of the suite (claim hold watcher, serve, dev server), and the suite back in Windows CI (on T-481); T-519 Desktop app spike on Windows and Linux: the undecorated window and the hidden menu (on T-482)
next: none (nothing ready in the sprint “Keys and reach”: the rest is waiting (1 in progress, 2 blocked))
file: 1010 KB; largest tasks: T-277 5 KB, T-391 5 KB, T-483 5 KB
archive: 209 tasks, 385 KB
```

After:

```text
Keys and reach · active · release 0.38.0 · 12 of 15 done
  Design task keys like ATM-123 with dependencies across projects, atm in every agent harness, readable CLI output, and live updates in the browser

In progress (1)
  T-488  P2  Overhaul atm's human-readable output: alignment, grouping and styling           claude:0f56a916

Blocked (2)
  T-413  P1  Support the harnesses beyond Claude Code through atm agents install             on user
  T-507  P3  The browser reacts to changes too: atm serve pushes them, and the refresh settings go  on user

Waiting on dependencies (2)
  T-482  P2  Windows: the rest of the suite (claim hold watcher, serve, dev server), and the suite back in Windows CI  on T-481
  T-519  P2  Desktop app spike on Windows and Linux: the undecorated window and the hidden menu  on T-482

Next
  none: nothing ready in the sprint “Keys and reach”: the rest is waiting (1 in progress, 2 blocked)

339 tasks: todo 6 · in progress 1 · blocked 2 · done 328 · skipped 2
file: 1010 KB (largest T-277 5 KB, T-391 5 KB, T-483 5 KB) · archive: 209 tasks, 385 KB
`atm project status --all` adds the counts by sprint, type and label
```

On a terminal 100 columns wide, the long names are cut so `on user` and `on T-481` stay on the line:

```text
  T-482  P2  Windows: the rest of the suite (claim hold watcher, serve, dev server), and t…  on T-481
```

With `--all`, the breakdowns follow as two-column lists. Sprints are in the project's order, open ones marked.
Types and labels are sorted by count, largest first:

```text
By sprint
  …
  Updates and renames   17
  Keys and reach        15  active
  backlog                9

By type
  feature   122
  refactor   69
  …
```

A section with nothing in it is left out. In review and Waiting for your review follow the same pattern as Blocked.

### `atm task list`

The columns are ID, status, priority, name, labels. Every column but the last two is padded to the widest value.
Labels move to the end, dimmed, without brackets. There is no header row: the columns explain themselves, and an
agent counting lines counts tasks. A dimmed summary line ends it.

Before:

```text
T-413  blocked      P1  [atm-cli, distribution] Support the harnesses beyond Claude Code through atm agents install
T-488  in_progress  P2  [cli, rust] Overhaul atm's human-readable output: alignment, grouping and styling
T-507  blocked      P3  [dashboard, api] The browser reacts to changes too: atm serve pushes them, and the refresh settings go
T-534  done         P1  [architecture, cli] Design: two task IDs (a UUID and a project key, ATM-123) and dependencies across projects
```

After:

```text
T-413  blocked      P1  Support the harnesses beyond Claude Code through atm agents install                    atm-cli, distribution
T-488  in_progress  P2  Overhaul atm's human-readable output: alignment, grouping and styling                  cli, rust
T-507  blocked      P3  The browser reacts to changes too: atm serve pushes them, and the refresh settings go  dashboard, api
T-534  done         P1  Design: two task IDs (a UUID and a project key, ATM-123) and dependencies across projects  architecture, cli
4 tasks: 1 in progress, 2 blocked, 1 done
```

The name column is as wide as the longest name that fits. Past that, a name pushes its labels to the right rather
than wrapping. On a terminal the name is cut so the labels fit. With keys (`ATM-538`, `~ATM-540`), the ID column is
padded the same way.

### `atm task details`

The heading is the ID and name, bold. The facts line is as today. The fields are labelled in a padded column, and
the long ones (description, notes) are sections. The text is unchanged: only the layout and the labels' case
change.

Before:

```text
T-488  Overhaul atm's human-readable output: alignment, grouping and styling
  in_progress · Keys and reach · polish · P2 · labels: cli, rust
  files: crates/atm-cli/src/commands/, tests/scripts/scenarios/, docs/agent/
  created 2026-11-08T08:30:53Z by Hakan Serce · started 2026-11-16T05:45:19Z by claude:0f56a916
  description:
    atm's plain output (without `--json`) is hard to scan. …
  notes:
    2026-11-16T05:45:18Z answer (claude:0f56a916): T-488 starts with a design you approve: … → Yes: …
```

After:

```text
T-488  Overhaul atm's human-readable output: alignment, grouping and styling
  in_progress · Keys and reach · polish · P2

  labels    cli, rust
  files     crates/atm-cli/src/commands/
            tests/scripts/scenarios/
            docs/agent/
  created   2026-11-08T08:30:53Z by Hakan Serce
  started   2026-11-16T05:45:19Z by claude:0f56a916

  description
    atm's plain output (without `--json`) is hard to scan. …

  notes
    2026-11-16T05:45:18Z  answer  claude:0f56a916
      T-488 starts with a design you approve: … → Yes: …
```

Lists (files, dependencies, requirements) go one per line. Several tasks (`atm task details T-1 T-2`) are separated
by a blank line, as today.

### `atm sprint list`

Before, with `--all`, 37 lines in one long run:

```text
Phase 0 · closed: 2 tasks (done 2)
…
Updates and renames · closed · release 0.37.0: 17 tasks (done 17)
  goal: The app updates smoothly and uses CompMD 0.7.0 as recommended, tab and title polish, releases marked released by hand, and renames in the CLI
Keys and reach · active · release 0.38.0: 15 tasks (active 1, blocked 2, done 12)
  goal: Design task keys like ATM-123 with dependencies across projects, atm in every agent harness, readable CLI output, and live updates in the browser
backlog: 6 tasks (atm sprint backlog)
```

After: a table, with the goals shown only for open sprints (a closed sprint's goal is in `atm sprint list --json` and
on the dashboard):

```text
Phase 0               closed           2 tasks  done 2
…
Updates and renames   closed  0.37.0  17 tasks  done 17
Keys and reach        active  0.38.0  15 tasks  done 12 · active 1 · blocked 2
  Design task keys like ATM-123 with dependencies across projects, atm in every agent harness, readable CLI output, and live updates in the browser
backlog                                6 tasks  (atm sprint backlog)
```

The columns are name, state, release, tasks, then the counts by kind, done first. A due date, when there is one,
goes after the release (`due 2026-10-31`).

### `atm release list`

Releases are sorted by version (semver), the planned ones last. The word "released" appears once. Merged counts
show only where they differ from done.

Before:

```text
0.24.0 “Sprints and releases” · released · released 2026-10-31 · PR #29: 36 tasks (done 36); merged 36
0.5.0 · released · released 2026-10-22: 0 tasks
…
0.35.0 · released · released 2026-11-10: 8 tasks (done 7, skipped 1); done 1, merged 6
0.38.0 · planned: 15 tasks (active 1, blocked 2, done 12); done 12
```

After:

```text
0.5.0                           released 2026-10-22           0 tasks
…
0.24.0  “Sprints and releases”  released 2026-10-31  PR #29  36 tasks  done 36
…
0.35.0                          released 2026-11-10           8 tasks  done 7 · skipped 1 · 1 not merged
0.37.0                          released 2026-11-13  PR #49  17 tasks  done 17
0.38.0                          planned                      15 tasks  done 12 · active 1 · blocked 2
```

"1 not merged" replaces "done 1, merged 6": it names the one fact that matters, a done task not on the base branch
yet.

### `atm doctor`

Errors come first, one per line, as today, since each is a fix. Warnings that say the same thing are grouped: the
message once, then the tasks. The last line is unchanged.

Before:

```text
warning: T-235: its files match no component (meta.components' paths)
warning: T-325: its files match no component (meta.components' paths)
… 30 more lines like these …
tasks.json is fine (339 tasks, 32 warnings)
```

After:

```text
warning: 32 tasks' files match no component (meta.components' paths):
  T-235 T-325 T-327 T-399 T-444 T-447 T-448 T-449 T-450 T-453 T-455 T-456 T-462 T-463 T-464 T-465
  T-466 T-467 T-468 T-469 T-470 T-471 T-472 T-473 T-474 T-475 T-476 T-491 T-522 T-536 T-538 T-539
tasks.json is fine (339 tasks, 32 warnings)
```

A warning on one task stays on one line (`warning: T-235: …`). The IDs are wrapped at 16 a line in a pipe, and to
the width on a terminal. This is the one place where a list of tasks stays a run: each ID is a token an agent can
find, and 32 lines of the same sentence help no one.

## 5. How it's built

- **One module, `crates/atm-cli/src/output.rs`**, that every command prints through:
  - `Out`: colour on or off, and the width (`None` in a pipe);
  - `style::{heading, dim, kind(Kind), priority(position)}`;
  - `Table`, which pads its columns and cuts one chosen column to the width;
  - `Section`, a heading with indented lines, left out when empty;
  - `wrap` for descriptions.

  The commands build lines from these instead of `format!`-ing their own. atm-lib's views and `--json` are not
  touched.
- **The terminal check and the colour rules, from a crate rather than by hand.** Two crates do it, both in
  `Cargo.lock` already through clap, so nothing new is downloaded:
  - **`anstream`** (stdout with colour on or off: it applies `NO_COLOR`, `CLICOLOR_FORCE`, `TERM=dumb` and "is a
    terminal", and strips styles otherwise, on Windows' console too);
  - **`anstyle`** (the styles).

  clap's own help already uses them, so `--color` works the same for both.
- **The width needs one new crate, or none.**
  - (a) Recommended: `terminal_size`. It's small, and it uses `rustix` and `windows-sys`, both already in the tree.
    The crate itself is new to the lockfile.
  - (b) Without it, the width comes from `COLUMNS` only. Shells don't export that, so in practice nothing would be
    cut.
  - (c) Don't cut at all: the terminal wraps long lines.

  Alignment is the same in all three. Only how long lines look on a terminal differs.
- **Order of work:**
  - `output.rs` and `--color`, with tests;
  - then one command at a time, each with its scenarios' expected `stdout` updated in the same commit:
    `task list`, `project status`, `task details`, `sprint list`, `release list`, `doctor`;
  - then the other commands' lists (`task deps`, `sprint backlog`, `claim list`, `label list`, …) in one pass.

## 6. Agents and security

- **Agents get plain text.** Their harnesses pipe standard output, so they get no styles and no cutting (rule 1).
  The words they match stay (rule 6).
- **`project status` loses the breakdowns by default.** No skill reads them: `/atm-plan-sprint` takes the backlog by
  label from `atm sprint backlog` and `atm task list --label`. It says so in a line, and `--all` brings them back.
- **`docs/agent/commands.md`** gets `--all` and `--color`.
- **The skills** that describe this output ("it has the counts, what is in review, blocked or waiting, and what's
  next") still hold, so they don't change.
- This changes output agents act on, so the release's security checklist applies: a written review in
  `docs/security-architecture.md` §3.3 that the skills still find what they look for. Nothing here is a new entry
  point.

## 7. Tests

- **Scenarios** (`tests/scripts/scenarios/*.json`) check the pipe output, as now. Their `stdout` for these six
  commands changes once, per command, with that command's commit.
- **A new `output.test.mjs`:**
  - a pipe gets no escape codes;
  - `--color always` gives them, for a heading, a status kind and a priority;
  - `NO_COLOR=1` with `--color auto` gives none;
  - `--json` never does, even with `--color always`.
- **Unit tests in `output.rs`:** padding, cutting with `…` at a width (counted in characters, so `“”` and `…` count as
  one), wrapping, and an empty section being left out.

## 8. Decisions for review

1. **Crates:** `anstream` and `anstyle` as direct dependencies of atm-cli (in the tree already), and
   `terminal_size` for the width (new; option (a) in §5)? Or (b) or (c)?
2. **`project status`:** the breakdowns behind `--all`, as above? Or keep the open sprints' counts by default?
3. **`task list`:** labels last and no header row?
4. **`sprint list`:** goals only for open sprints?
5. **`release list`:** sorted by version, and "N not merged" in place of the merged count?
6. **`doctor`:** grouping same-message warnings?
7. **`--color auto|always|never`** as a global flag?
