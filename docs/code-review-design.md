# Code reviews in atm (ATM-337)

Status: **draft, for review with Hakan.** A code review tool built into atm, recorded in the project, to replace Crit
for the reviews atm's work needs. Hakan's requirements (2026-11-17) are in ATM-337's description; this document works
them out, and [the appendix](#appendix-use-cases) walks through the use cases it has to cover.

## 1. Problem

Every task that needs a review today goes through two tools that don't know each other:

- **atm** knows the task is in review (`in_review`, `review_state`), who decides (I23), and what the decision means
  for the loop (`atm loop next` hands back a task with changes asked for). It knows nothing of *what* was reviewed:
  the decision is one note.
- **Crit** holds the review itself: the diff or the document, the inline comments, the rounds. Its records live in
  Crit's own folder, outside the project, and end when the review does. Nothing links them to the task, so a task
  says "approved" with no trace of what was said about it.

So:

1. **The review is lost.** A month later, "why is this written this way?" has no answer in the project: the comment
   chain that decided it was in Crit's session.
2. **The review is a side trip.** The agent runs Crit in the background, the user switches to a browser tab, and the
   decision comes back to atm by hand (`atm task review approve`), separately from the comments that justified it.
3. **Agents can't take part on their own terms.** An agent sees comments only when the user clicks Finish in Crit;
   it can't list what is open, ask what changed, or wait for the next round with its claim kept alive, through atm.
4. **The app has a document viewer but no review.** The atm app already renders the project's documents with CompMD
   (the viewer, T-336), the same renderer a review of a Markdown design needs, but a review opens elsewhere.

## 2. Context

What this builds on, as atm is today:

- **Tasks and their review** ([task-states.md](task-states.md)): a status of kind `review`, a `review_state`
  (`waiting`, `approved`, `changes_requested`, `rejected`), review modes (`blocker`, `async`, `batch`, `none`) and the
  decisions `atm task review approve|request-changes|reject`, which are the user's (I23: no skill pre-approves them).
- **The store** ([layered-architecture.md](layered-architecture.md) §3): atm-lib reads and changes task data through
  atm-db's `Store` trait, whose one real implementation is the JSON-in-git store (files under
  `.agentic-task-manager/`, a lock, a merge driver); a store can `subscribe`, pushing changes (T-555).
- **Keys** ([ADR-007](adr/ADR-007-task-keys-uuids-and-dependencies-across-projects.md)): tasks have UUIDs and
  per-project numbers (`ATM-337`); a number is provisional (`~`) until merged into the base branch, and the merge driver
  renumbers the incoming side's clashes.
- **The document viewer** (T-336, [compmd-viewer-design.md](compmd-viewer-design.md)): atm-lib's `documents.rs` reads
  a project's file with its checks (I11, I12: relative, no hidden, ignored or key file, under 1 MB, through no
  symlink); the dashboard's `lib/documents/` shows it, Markdown rendered by CompMD (`CompMd.svelte`). Both components
  read the dashboard's state (`state.svelte`) and `CompMd.svelte` takes a `Task`: the viewer isn't separable yet.
- **The app's window** ([app-desktop-ui-design.md](app-desktop-ui-design.md)): tabs are addresses (a tab is a page
  address; a new page is a new kind of tab for free), a sidebar listing the workspace's projects, deep links.
- **Writes:** the REST API changes nothing (I11, one exception for the last opened workspace); the app's commands are
  its reads (I27), plus two writes on the user's click. Everything else changes through the CLI.
- **Crit**, which this replaces, has: reviews of a file (a plan), of the branch's diff, of a commit range, of a PR;
  inline comments on lines or a quoted selection, replies, resolving (the reviewer's); rounds (the agent revises, the
  reviewer sees the next round); anchors that follow edited text and say when they drifted; source and rendered views
  of Markdown, split and unified diffs; sharing a review on the web; syncing comments with a GitHub PR.

## 3. Proposal

### 3.1 Three components

Three parts, each its own Rust crate and its own Svelte area, with the dependencies one way:

```mermaid
flowchart LR
    Tasks["1 · Task manager<br/><i>atm-db, atm-lib's tasks;<br/>lib/kanban, tasks, …</i>"]
    Docs["2 · Document viewer<br/><i>atm-docs;<br/>lib/documents</i>"]
    Reviews["3 · Code review<br/><i>atm-review;<br/>lib/reviews</i>"]
    Link["The link<br/><i>atm-lib's reviews module;<br/>lib/reviews/links</i>"]
    Vcs["atm-vcs<br/><i>git: diffs, revisions</i>"]

    Reviews --> Docs
    Reviews --> Vcs
    Docs --> Vcs
    Link --> Reviews
    Link --> Tasks
```

- **Document viewer** (`atm-docs`, new): what `atm-lib/src/documents.rs` does now, a project's file read with its
  checks, plus a file **at a revision** (through atm-vcs), which a review's old side needs. In the dashboard,
  `lib/documents/` loses its reads of the dashboard's state and the `Task` type: it takes a **document source** (load
  a path, at a revision or the working copy) and the links' handler as props. The task manager passes one that reads
  through the project's routes; the review passes one that reads a round's sides.
- **Code review** (`atm-review`, new): the review records, their store interface and its file implementation (§3.2),
  the rounds and the diffs, the anchors and how they follow the code (§3.5). It knows a task only as a key (a string):
  it doesn't depend on atm-db or the task model.
- **The link** (atm-lib's `reviews` module): the only place the two meet. A review names its tasks; a decision on a
  review becomes the task's review decision (§3.6); a task lists its reviews. The front ends (CLI, REST API, app) call
  atm-lib, as the layer rule has it (ADR-003 §4).

**Separable without much effort:** a review tool on its own is `atm-review` and `atm-docs` with a front end of its
own, and `lib/reviews` with `lib/documents`; nothing in them imports the task manager. The boundaries are **checked
now**, so they hold until then: `layers.test.mjs` (which checks the crates' dependencies today) gains the two crates,
and an ESLint rule (`no-restricted-imports`) keeps `lib/documents` from importing anything outside itself and `lib/ui`,
and `lib/reviews` from importing the task areas except through `lib/reviews/links`.

### 3.2 The records

One folder per review, under the task folder:

```text
.agentic-task-manager/reviews/
  CR-1/
    review.md          the header and the summary
    comments.md        every comment chain, in file order
    rounds/
      1.diff           round 1's diff, as git prints it
      1.json           what round 1 is: its base and head, its files
      2.diff
      2.json
```

**`review.md`**: a header atm writes and reads, then the summary in Markdown, which the author writes:

```markdown
+++
id = "0f3c…"                 # a UUID: merges match reviews by it, not by number
number = 7
title = "Claim hold on Windows"
status = "changes_requested"   # draft · open · changes_requested · approved · closed
tasks = ["ATM-482"]
author = "claude:0f56a916"
reviewers = ["Hakan Serce"]
created = 2026-11-17T03:20:00Z
updated = 2026-11-17T04:02:00Z
+++

The watcher found its shell as pid 0 on Windows…
```

The header is TOML (`+++`, as Hugo's front matter): readable, typed (dates, lists), and its parser (`toml`) is in
`Cargo.lock` already, through Tauri; as atm-review's own dependency it is still a new one, so it's a decision (§5).
The statuses:

| Status | Means | Who moves it there |
|---|---|---|
| `draft` | the author is still putting it together; not shown as waiting on anyone | the author (`atm review create --draft`) |
| `open` | waiting on the reviewers | the author, with a round |
| `changes_requested` | sent back for fixes | a reviewer, when submitting |
| `approved` | approved | a reviewer, when submitting |
| `closed` | ended without approval (abandoned, replaced) | the author, or a reviewer |

**`comments.md`**: every comment chain of the review, **one file for all rounds** (a chain started on round 1 is
answered and resolved in round 3; per-round files would split it). Each chain starts with what it is about, then its
comments, oldest first:

```markdown
## crates/atm-cli/src/process.rs:40-52 · round 1 · open · t4
> let shell = 0;

**Hakan Serce** · 2026-11-17 03:31 · c9
On Windows this follows the idle process. Find the parent instead?

**claude:0f56a916** · 2026-11-17 03:40 · c10
Done in round 2: `shell_pid()`, past Git Bash's forked copy of itself.

## docs/ci-review.md · round 2 · resolved · t5
…

## (the review) · round 1 · open · t1
Overall: …
```

- the heading: the file and its lines **on the round's new side** (or `(the review)` for a chain about the whole
  review), the round it was made on, the chain's state (`open`, `resolved`), its ID;
- the quote: the lines (or the selection) it was made on, which the anchor follows (§3.5);
- each comment: its author, time, ID, then its text (Markdown, rendered with CompMD as task text is).

atm writes this file (through `atm review comment` and the app); people read it in any editor and in `git log -p`. It
is parsed back by its headings and IDs; text the format doesn't expect is kept and reported by `atm doctor`, as an
unreadable task is.

**`rounds/<n>.diff` and `<n>.json`**: what the reviewers saw. The diff is git's (`git diff --full-index -M`, binary
files listed, not included); the JSON says what it was made from: `{"kind": "range", "base": "<sha>", "head": "<sha>"}`
for a commit range, `{"kind": "working", "base": "<sha>", "snapshot": "<sha>"}` for uncommitted changes, and the
files with their status and sizes. Storing the diff keeps the review whole when its commits are rebased away or
squashed in a merge. A diff above a limit (1 MB, as a document) is stored as its file list only, with a note.

**Uncommitted changes** have no commit to name: atm records them as a snapshot commit, made without touching the
user's index or working tree: the changes, new untracked files included (not ignored ones), are added to a temporary
index (`GIT_INDEX_FILE`), written as a tree and a commit on `HEAD` (`git write-tree`, `git commit-tree`), and kept from
git's garbage collection by a ref, `refs/atm/reviews/<uuid>/<round>`. With it, a later round compares with this one exactly
(§3.4), as for commits.

**What isn't recorded:** who viewed which file. That is one reviewer's progress, kept with the user's other UI state
(the app's state folder), by review, round and file content, so a file that changes in a new round is unviewed again.

### 3.3 The store

`atm-review` has a **`ReviewStore` trait**, as atm-db has `Store`: what atm-lib reads and changes reviews through,
whatever holds them.

```rust
pub trait ReviewStore {
    fn list(&self) -> Result<Vec<ReviewHeader>>;
    fn get(&self, number: u32) -> Result<Option<Review>>;           // header, summary, threads, rounds
    fn round_diff(&self, number: u32, round: u32) -> Result<Patch>;
    fn create(&mut self, new: NewReview, first: NewRound) -> Result<u32>;
    fn change(&mut self, number: u32, change: &mut dyn FnMut(&mut Review) -> Result<()>) -> Result<()>;
    fn subscribe(&self, on_change: OnChange) -> Result<Subscription>;
}
```

The first implementation is the folder above: one lock per review (a writer changes `review.md` and `comments.md`
together, or not at all), writes to a temporary file and renamed into place, as `tasks.json`'s are; `subscribe`
watches the `reviews/` folder (the JSON store's watcher, T-555). The number of a new review comes from a counter
shared by the clone's worktrees (`.git/atm/`, beside the claims), so two worktrees never take the same one. A store
that isn't files (a server, later) implements the same trait.

**Worktrees:** a review made in a worktree is in that worktree's files until its branch merges. The reviews list
reads the main working copy and the project's worktrees, as the dashboard shows the worktrees' tasks
(`copy_tasks`), each review marked with its branch.

### 3.4 Rounds and what you see

A **round** is what the author asks the reviewers to look at: the first when the review is opened, then one each time
the author has addressed the comments (`atm review round`). It names its base and head (or snapshot).

- **The whole change:** the latest round's diff, base to head.
- **Between two rounds** (round 2 against round 1): `git diff <round 1's head> <round 2's head>`, limited to the
  review's files: what changed in response to the comments. Both heads are commits (or snapshots), so this is exact.
  When a head is gone from this clone (another clone's snapshot, pruned), atm says so and offers the whole change.
- **A document review** (a plan, a design: Crit's plan reviews) is a review of one file, its rounds the file at each
  round (committed, or a snapshot): the same views, and the rendered view by default.

### 3.5 Comments and anchors

A chain is anchored to **a round, a file, a side (new or old), a line range, and the quoted text** (the selected text
or the lines). In a later round, atm moves the anchor through the diff between the two rounds: lines unchanged around
it move with it; when the quoted text is no longer there, the chain shows as **drifted** (Crit's word), at its last
known place, and its quote shows what it was about. The chain's heading in `comments.md` keeps the line range of the
round it was made on; where it shows in a later round is worked out, not stored.

**Resolving is the reviewer's** (as in Crit): the author (an agent too) replies "done in round 3"; the reviewer
resolves, or answers.

### 3.6 Tasks and reviews

- A task records its reviews: `reviews: ["CR-7"]`, a list in the task (the schema gains it), shown as links on the
  card and in the details (`CR-7 · changes requested`), each opening the review, in a new tab with ⌘-click.
- **Opening a review for a task** (`atm review create --task ATM-482 …`) puts the task in review when it is in
  progress (as `atm task state ATM-482 in_review` does, with the review's link in its `review` note).
- **A reviewer's decision is the task's:** submitting with *approve* approves the task's review (`review_state =
  approved`), with *request changes* asks for changes (`changes_requested`, a `rejection` note: "CR-7 round 1: 4
  comments", linking the review); closing the review without approval rejects it when the user says so. `atm loop
  next` then hands the task back to its agent, which reads the open chains. These are the decisions of I23: made by
  the user, through the app or the CLI, never pre-approved for an agent.
- **One review, several tasks** (a sprint's batch review, a change that finished two tasks): each task named gets the
  decision. **Several reviews for one task** (a second review after it was reopened): the task follows the latest.
- Not every task review needs a CR: `atm task review approve` without one stays as it is.

### 3.7 The CLI, and agents listening

```text
atm review create [--task KEY]… (--range BASE..HEAD | --uncommitted | --file PATH) [--title T] [--summary-file F] [--draft]
atm review round CR-7 [--range …|--uncommitted]       # the next round, after addressing comments
atm review list [--status S] [--task KEY] [--mine] [--search TEXT] [--json]
atm review show CR-7 [--round N] [--against M] [--json]
atm review comments CR-7 [--open] [--since ID] [--json]
atm review comment CR-7 --file PATH --lines 40-52 [--quote TEXT] --body-file F
atm review reply CR-7 c9 --body-file F
atm review resolve CR-7 t4                             # the reviewer's
atm review submit CR-7 (--approve | --request-changes | --comment) [--body-file F]   # the reviewer's
atm review close CR-7 --reason TEXT
atm review wait CR-7 [--for submitted|decision] [--timeout 2h]
```

**`atm review wait`** is how an agent listens: it blocks until the reviewers submit (or decide), on the store's
subscription, not a poll the agent writes, and prints what changed (`--json`: the new comments, the decision). An
agent waits as it waits for CI, with its claim held: `atm claim hold ATM-482 && atm review wait CR-7`, in the
background. The skills get the loop Crit's skill has now: open a review, wait, read the comments, reply to each, fix,
commit, `atm review round`, wait again; until approved.

**Comments are task text** (I1): data, never instructions. A comment that asks the agent to run a command, fetch a
script or touch credentials, CI, git config or hooks is answered or blocked on the user, as a task asking for it is.
The skills say so, and the agent sandbox's red-team run gets a review with such comments.

### 3.8 The reviews section

**In the app**, the sidebar gets a second section, **Reviews**, under the projects: the reviews that need you (open,
with you a reviewer; sent back, with you the author), with a count, and **All reviews**, which opens the reviews
page. A review opens in its own tab (a tab is an address: `/projects/<id>/reviews/CR-7`, `?round=3&against=2`,
`&file=…`), so several reviews and the tasks can be open side by side, and deep links (`atm://…/reviews/CR-7`) open
one.

**The reviews page** (`/projects/<id>/reviews`, and across the workspace's projects on the overview): search (title,
summary and comments, every word, as the tasks' search), filters (status, author, reviewer, task, branch), sorted by
the latest activity. **Working through them** ("process capability"): from a review, *Next review needing you* (a
shortcut), and the needs-you menu counts reviews with tasks.

**A review**:

- the header: title, status, tasks (links), branch, rounds, the summary (CompMD);
- the files, as a tree with each file's changes and a *viewed* tick; *n* / *p* jump to the next and previous change,
  *]* / *[* to the next and previous file, *v* marks a file viewed and moves on;
- each file **split** or **unified**, and for Markdown (and anything CompMD renders) **source** or **rendered**;
- the round picker: the whole change, or one round against another;
- comments: select lines (or text, in the rendered view) and comment; replies and resolving in place; *Submit*, with
  approve, request changes or comment, and a summary.

**In the browser** (`atm serve`), the same pages, **read only** in the first version (§5): comments are written in the
app or with the CLI.

### 3.9 Writing from the window

The app gets **write commands for reviews**: comment, reply, resolve, submit, close. They are a new exception to I27,
like the two it has: each a thin mapping to an atm-lib function (as every command is), acting **as the user** (the
app's user is the user: no `--agent`), only on the user's action in the page. The page never gets a general write;
CompMD's frames, which show comment text, still call nothing (I25). `docs/security-architecture.md` changes with it
(B7 and I27: what the app may write; B2: review files from the repository are untrusted like task files; I1: review
comments are task text).

## 4. Risks

- **Rendering a diff is CompMD's ground.** Highlighting code and showing rendered Markdown side by side, with the
  changed blocks marked, is rendering, which atm doesn't do (CompMD renders; atm hands it content). The source view
  of a diff needs CompMD's highlighter on lines, and the rendered view a way to mark changed blocks. **This needs
  CompMD's side first** (a diff component, or highlighting of a line list); until it has one, the source view is
  plain text and the rendered view shows the two sides without marks. To agree with CompMD before the UI tasks.
- **The repository grows.** Every round's diff is committed. Most reviews are small, but a large refactor's rounds
  add up. The 1 MB limit per round keeps the worst out; `atm db archive` could later move old reviews' diffs out, as
  it moves finished tasks.
- **Merges.** Reviews made on two branches merge as files: the rounds never conflict (new files), `comments.md` and
  `review.md` can. atm's merge driver gets them (`.gitattributes`, as for `tasks.json`): chains and comments by ID,
  the header by its latest `updated`. **Numbers can clash** across clones (two people's CR-8); within a clone the
  shared counter prevents it. ADR-007's answer (provisional until merged, the incoming side renumbered) needs a
  folder renamed, which a merge driver can't do: the merge then stops on `CR-8`, and atm renames the incoming one
  (`atm review renumber`, which `atm doctor` points to). Rare for one user with worktrees; real for a team.
- **Forged records.** A review's approval is a file anyone with commit access writes, as a task's is today. It records
  a review, it doesn't authorize anything: `main`'s ruleset (I21) stays the gate.
- **Anchors drift on big rewrites.** A file rewritten between rounds leaves its chains drifted. Crit has the same
  limit; the quote keeps them readable.
- **Scope.** This is a product, not a feature: storage, a diff UI, rounds, anchors, an agent protocol, a sidebar, a
  page. Phased (§6) so each phase is useful on its own.

## 5. Open issues

1. **The header's format:** TOML front matter (`toml`, in `Cargo.lock` already, new to atm's crates), or JSON
   (`review.json` beside a `summary.md`), or atm's own `key: value` lines (no dependency, more to parse by hand).
   Proposed: TOML.
2. **Writes in the browser:** read-only there (proposed for the first version), or a write route (loopback, the Host
   check, a per-session token against CSRF) and a change to I11.
3. **Who may decide:** the user only (I23, proposed), or also a reviewer agent when the project hands reviews to one
   (`blocked --on agent`'s counterpart).
4. **The snapshot refs** (`refs/atm/reviews/…`): kept local (proposed), or pushed, so another clone can compare rounds
   of uncommitted changes exactly.
5. **Number clashes across clones:** the renumbering above, or numbers per clone (`CR-h7`: no clash, less readable).
6. **What of Crit can wait:** sharing a review on the web (`crit share`) and syncing comments with a GitHub PR are not
   in this design; proposed later, if wanted.
7. **CompMD's diff support** (§4, the first risk): what CompMD offers, and when.
8. **Comments in a pushed PR:** when a branch with reviews becomes a GitHub PR, the records go with it; whether
   `/atm-push` links the CRs in the PR body.

## 6. Phases (the follow-up tasks, once approved)

1. **The records and the CLI:** `atm-review` (model, `ReviewStore`, the folder), `atm review create|round|list|show|comments|comment|reply|resolve|submit|close|wait`,
   the task link (§3.6), the merge driver for the two files, `atm doctor`'s checks. Agents and the user can run a
   review in the terminal; the skills move from Crit to it for diffs and documents.
2. **The viewer split:** `atm-docs` out of atm-lib; `lib/documents` on a document source; the boundary checks (§3.1).
3. **The review in the app:** the reviews page and a review's page (files, split and unified, source and rendered,
   viewed, keys), the sidebar section, comments written through the app's commands, the security document's update.
4. **Rounds:** comparing rounds, anchors moving and drifting, uncommitted changes' snapshots.
5. **Later:** the browser's writes, CompMD's diff marks, sharing and PR sync if wanted, splitting the components into
   apps.

## 7. Alternatives considered

- **Keep Crit, link it to tasks.** Record Crit's review file path on the task. Cheapest, but the records stay outside
  the project and Crit's lifecycle, and agents still go through Crit's own protocol. Doesn't meet "stored locally, in
  the project".
- **Reviews in `tasks.json`** (threads as notes on the task). One store, merged by the existing driver; but diffs and
  long chains in the task file would make it large and slow for every read, and a review spans tasks.
- **Reviews out of git** (in `.git/atm/`, like the claims). No repository growth, no merges; but a review would be
  lost with the clone and invisible to anyone else, the opposite of the goal.
- **GitHub PR reviews as the store.** Rich, shared; but online, GitHub only, one PR per branch (atm reviews tasks and
  documents, several per branch), and agents would need GitHub credentials to take part.
- **One file per comment chain** (`threads/t4.md`). Merges never conflict; but the review is no longer one file to
  read top to bottom, which is what was asked for. The merge driver gives the single file the same merges.
- **A comments file per round.** Simple to append to; but a chain crosses rounds, and reading the review means
  reading every round's file.
- **Store only SHAs, no diffs.** Small; but a rebase or a squash merge loses what was reviewed, and uncommitted
  changes have no SHA.
- **Gerrit-style patch sets as refs only.** Exact and git-native; but refs aren't pushed or merged by default, so the
  record wouldn't travel with the branch.

## Appendix: use cases

Each with how the proposal covers it.

**U1. An agent's task needs a review before it is done** (`blocker` mode). The agent commits, runs `atm review create
--task ATM-482 --range origin/dev..HEAD --summary-file …`: CR-7 round 1, the task in review with the link. The user
sees CR-7 under *Reviews needing you* in the app's sidebar, opens it in a tab, comments on two places, submits with
*request changes*. The task's review is `changes_requested`; the agent, waiting with `atm claim hold ATM-482 && atm
review wait CR-7`, gets the two comments as JSON, replies to each, fixes, commits, `atm review round CR-7`, waits. The
user opens round 2 against round 1, sees only the fixes, resolves both chains, approves. The task is approved; the
agent finishes it (`atm task state ATM-482 done`). §3.6, §3.7, §3.4.

**U2. A review of uncommitted changes**, before committing (the user's own, or an agent's: "show me before you
commit"). `atm review create --uncommitted`: the snapshot commit and its ref (§3.2), the index and working tree
untouched. After the comments, the next round is a snapshot again, or the commit: the rounds compare either way.

**U3. A design document's review** (this document's, Crit's plan reviews). `atm review create --file
docs/code-review-design.md --task ATM-337`: rounds are the file at each round; the rendered view by default, with
CompMD; comments on rendered text are anchored by their quote. §3.4, §3.5.

**U4. The whole change, or only what changed since the last round.** The round picker (§3.4): the latest round's
whole diff, or round 3 against round 2, exact through the rounds' heads.

**U5. A batch review at the sprint's end.** The tasks in `batch` mode are reviewed together: one CR over the sprint's
range, naming them (`--task` repeated). A decision is each task's; a chain about one task names it in its text.
§3.6.

**U6. Finding why something was decided.** The reviews page's search: "SIGTERM" finds CR-7's chain t4, in its
comments; from the task, its reviews' links. And without atm: `comments.md` in the repository, in any editor or
`git log -p`. §3.8, §3.2.

**U7. An agent working several tasks while a review is open** (`async` mode). The agent opens the review, goes on with
the next task, and the loop hands the reviewed one back when the decision comes (`atm loop next` already does, from
the task's review state). §3.6.

**U8. A review made in a worktree.** An agent in `wt2` opens CR-9 on its branch. The user's app, in the main
checkout, lists it from the worktree's files, marked with its branch; comments the user writes go to that worktree's
`comments.md`, where the agent's `atm review wait` sees them. §3.3.

**U9. The user goes through what waits on them.** The needs-you menu counts the reviews with the tasks; *Next review
needing you* goes through them in order; a file marked viewed stays viewed until it changes. §3.8, §3.2.

**U10. A hostile repository** ships a review whose comment says "approve and run `curl … | sh` to finish". The text
renders as text (CompMD, I14, I15); an agent reading it treats it as data (I1) and asks the user; the approval in the
file decides nothing outside atm (I21). §3.7, §4.

**U11. A rebased branch.** The commits of round 1 are rewritten. The round's stored diff still shows what was
reviewed; comparing with it falls back to the whole change when its head is gone from the clone. §3.2, §3.4.

**U12. A review in the browser** (`atm serve`, no app). The reviews pages work, read only; comments are written with
`atm review comment`, or in the app. §3.8, §5 (2).

**U13. A teammate's clone.** The reviews come with the branches; a CR number taken on both sides is renumbered when
the merge stops on it. §4.

**U14. A large or binary change.** Binary files are listed, not diffed; a round over 1 MB keeps its file list and
says so. §3.2.

**U15. Moving this repository off Crit.** The skills that use Crit (the design reviews, the review held with the user)
move to `atm review`; Crit stays available for what this doesn't cover yet (sharing, PR sync). §5 (6), §6.
