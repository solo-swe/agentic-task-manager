"""Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
`pnpm run build:cli` in dashboard/ (the pre-commit hook does it too). TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has
no typing.Required)."""

from typing import Dict, List, Literal, Optional, TypedDict

StatusEntry = TypedDict(
    "StatusEntry",
    {
        # required
        "id": str,
        "label": str,
        # required; What the status means
        "kind": Literal['todo', 'active', 'review', 'blocked', 'done', 'skipped'],
    },
    total=False,
)

VocabularyTerm = TypedDict(
    "VocabularyTerm",
    {
        # required
        "id": str,
        "label": str,
    },
    total=False,
)

LabelEntry = TypedDict(
    "LabelEntry",
    {
        # required
        "id": str,
        "label": str,
        "description": str,
    },
    total=False,
)

PhaseEntry = TypedDict(
    "PhaseEntry",
    {
        # required
        "id": float,
        "name": str,
    },
    total=False,
)

# How the project pushes (docs/v4/skills-and-workflows.md §1)
Workflow = TypedDict(
    "Workflow",
    {
        # direct or pull-request
        "type": str,
        "base": str,
        "verify": bool,
        "max_rounds": int,
        "commit_template": str,
        # "forbid" or "none", or {"require": [the lines]}: what commit messages end with
        "trailers": object,
    },
    total=False,
)

# When atm suggests archiving, and whether agents may do it (DM-5)
ArchiveSettings = TypedDict(
    "ArchiveSettings",
    {
        "threshold_kb": float,
        "threshold_tasks": float,
        "keep_days": float,
        "auto": bool,
    },
    total=False,
)

Sprint = TypedDict(
    "Sprint",
    {
        # required; The sprint's name
        "id": str,
        "goal": str,
        "state": Literal['planned', 'active', 'closed'],
        "release": str,
        # ISO 8601 UTC
        "start": str,
        # ISO 8601 UTC
        "end": str,
        # YYYY-MM-DD
        "due": str,
        "report": "SprintReport",
    },
    total=False,
)

# What a closed sprint did (SR-2.3)
SprintReport = TypedDict(
    "SprintReport",
    {
        # The tasks done in it
        "done": List[str],
        # The unfinished tasks carried on
        "carried": List[str],
        # Where the carried tasks went
        "to": str,
    },
    total=False,
)

Release = TypedDict(
    "Release",
    {
        # required
        "id": str,
        "name": str,
        "state": Literal['planned', 'released', 'archived'],
        # YYYY-MM-DD
        "target": str,
        # ISO 8601 UTC
        "released_at": str,
    },
    total=False,
)

Component = TypedDict(
    "Component",
    {
        # required
        "id": str,
        "label": str,
        # Globs over its files
        "paths": List[str],
        # Its tasks' default review mode
        "review": str,
    },
    total=False,
)

# A project's meta in tasks.json: its settings, vocabularies, sprints, releases and components (docs/v4/data-model.md §2). Every key is optional and falls back to its default.
Meta = TypedDict(
    "Meta",
    {
        # The project's ID (8 hex characters, set by atm init)
        "id": str,
        # The project's name
        "project": str,
        # One line about the project
        "description": str,
        "created": str,
        # The repository's web address, for commit and PR links
        "repository": str,
        # github, gitlab, bitbucket or gitea, for a host whose name doesn't say
        "repository_type": str,
        # The project's logo, relative to its root
        "logo": str,
        # How long a claim lasts without a heartbeat
        "lease_minutes": float,
        "statuses": List["StatusEntry"],
        "types": List["VocabularyTerm"],
        # Most urgent first
        "priorities": List["VocabularyTerm"],
        # The priority of tasks without one
        "default_priority": str,
        "labels": List["LabelEntry"],
        # Before sprints (atm migrate turns them into sprints)
        "phases": List["PhaseEntry"],
        "workflow": "Workflow",
        "archive": "ArchiveSettings",
        # How many tasks tasks-archive.json holds (DM-5.4)
        "archived": int,
        # The number the next new task gets
        "next_id": int,
        # In order
        "sprints": List["Sprint"],
        # In order
        "releases": List["Release"],
        "components": List["Component"],
        # How a sprint created without a name is named: people or time
        "sprint_names": str,
        # The project's default review mode: blocker, async, batch or none
        "review": str,
    },
    total=False,
)

Note = TypedDict(
    "Note",
    {
        # required; ISO 8601 UTC
        "at": str,
        # Who wrote it
        "by": str,
        # required; handoff, outcome, review, rejection, failure, answer or note
        "kind": str,
        # required; Markdown, at most 500 characters
        "text": str,
    },
    total=False,
)

# The PR that contains the task (atm pr): the task is In PR until merged_at (DM-4.3)
PullRequest = TypedDict(
    "PullRequest",
    {
        # required
        "number": int,
        "url": str,
    },
    total=False,
)

BlockRound = TypedDict(
    "BlockRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        # user or agent
        "on": str,
    },
    total=False,
)

ReviewRound = TypedDict(
    "ReviewRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        "outcome": str,
    },
    total=False,
)

SprintHistoryEntry = TypedDict(
    "SprintHistoryEntry",
    {
        # required
        "sprint": str,
        # required; ISO 8601 UTC
        "from": str,
        # ISO 8601 UTC; absent while it is in the sprint
        "to": str,
    },
    total=False,
)

# Before v4: the last failed attempt (now a failure note; still read)
LastFailure = TypedDict(
    "LastFailure",
    {
        "at": str,
        "reason": str,
    },
    total=False,
)

# One task, as tasks.json and tasks-archive.json keep it (docs/v4/data-model.md §4). Unknown fields are kept as they are.
Task = TypedDict(
    "Task",
    {
        # required; Letters, digits, '.', '_' and '-' (T-001)
        "id": str,
        # required
        "name": str,
        # required; A type ID from the vocabulary (meta.types)
        "type": str,
        # required; A status ID from the vocabulary (meta.statuses); atm acts on its kind
        "status": str,
        # Markdown
        "description": str,
        # Before sprints: the task's phase (atm migrate turns phases into sprints)
        "phase": float,
        # A priority ID from the vocabulary ("P0"…"P3" by default); missing means the default priority
        "priority": str,
        # Free-form tags that group tasks across sprints and types, e.g. ["spike-2", "ux"] (DM-4.6)
        "labels": List[str],
        # Task IDs it waits for
        "dependencies": List[str],
        "requirements": List[str],
        "use_cases": List[str],
        # Files it creates or changes
        "files": List[str],
        # ISO 8601 UTC
        "created_at": str,
        # ISO 8601 UTC
        "updated_at": str,
        # ISO 8601 UTC; set when the task moves to in progress
        "started_at": str,
        # ISO 8601 UTC; set when the task is done
        "completed_at": str,
        # ISO 8601 UTC; set when the task is skipped (atm skip)
        "skipped_at": str,
        # Before v4: the agent that started the task. Now started_by (readers fall back to it)
        "agent": str,
        # Who created it (docs/v4/data-model.md §4.1)
        "created_by": str,
        # Who changed it last
        "updated_by": str,
        # Who started it: the claim's owner
        "started_by": str,
        # Who finished it
        "completed_by": str,
        # The SHA of the task's commit
        "commit": str,
        "pr": "PullRequest",
        # ISO 8601 UTC; when that PR was merged (atm merged)
        "merged_at": str,
        # ISO 8601 UTC; when the task asked for a review (atm review). Its work ended here; review time isn't work
        "review_requested_at": str,
        # A blocked task waits on a decision by the user or an agent (atm block --on)
        "waiting_on": str,
        # The open questions of a blocked task
        "questions": List[str],
        # ISO 8601 UTC; when it was blocked
        "blocked_at": str,
        # Past blocks, oldest first, with who each waited on (T-244)
        "blocks": List["BlockRound"],
        # Past reviews, oldest first
        "reviews": List["ReviewRound"],
        # Handoffs, outcomes, review requests, answers, rejections (docs/v4/data-model.md §4)
        "notes": List["Note"],
        "last_failure": "LastFailure",
        # The sprint it is in (a sprint's name, meta.sprints); absent: the backlog
        "sprint": str,
        # Each sprint it was in, from when to when; the open entry has no `to` (SR-2.5)
        "sprint_history": List["SprintHistoryEntry"],
        # The release it ships in; when absent, its sprint's (SR-2.6)
        "release": str,
        # Component IDs (meta.components)
        "components": List[str],
        # How it is reviewed: blocker, async, batch or none
        "review": str,
        # In the review kind: waiting, changes_requested, approved or rejected (SR-6.1, T-438)
        "review_state": str,
        # ISO 8601 UTC; when the review was decided
        "reviewed_at": str,
        # A batch task done by an agent: pending until the end-of-sprint review, then reviewed (SR-4.4)
        "sprint_review": str,
        # `user`: the work is done together with the user (SR-6.5)
        "with": str,
        # Related tasks (a link that doesn't block), stored on one side and shown on both (SR-2.8)
        "related": List[str],
        # How a finished task ended, when it isn't the default: wont_do, duplicate, obsolete or done (SR-2.8)
        "resolution": str,
        # The task this one duplicates (with the duplicate resolution)
        "duplicate_of": str,
    },
    total=False,
)

# atm's suggestion to archive (CLI-5.1), present when the working tasks are over meta.archive's thresholds (T-372)
ArchiveHint = TypedDict(
    "ArchiveHint",
    {
        # required; How big the working task data is
        "bytes": int,
        # required; How many working tasks are finished
        "finished": int,
        # required; How many of them `atm archive` would move
        "archivable": int,
        # required; Whether agents may archive on their own (meta.archive.auto)
        "auto": bool,
        # required; The hint as atm says it
        "text": str,
    },
    total=False,
)

# A task as stored, and what atm derives for it
TaskView = TypedDict(
    "TaskView",
    {
        # required
        "task": "Task",
        # required
        "derived": "TaskDerived",
    },
    total=False,
)

# What atm-lib works out for a task, so no front end applies atm's rules itself
TaskDerived = TypedDict(
    "TaskDerived",
    {
        # required; The status's kind: todo, active, review, blocked, done or skipped (absent for an unknown status)
        "kind": str,
        # required; To do, and every dependency done (an archived one counts as done)
        "ready": bool,
        # required; A to-do task's dependencies that aren't done yet
        "waiting_for": List[str],
        # Who a blocked task waits on: user or agent
        "waiting_on": str,
        # required; Its priority, or the project's default
        "priority": str,
        # Its review mode: its own, else its component's, else the project's
        "effective_review": str,
        # Its release: its own, else its sprint's
        "effective_release": str,
        # A done task's stage: done, in_pr or merged (DM-4.3)
        "stage": str,
        # required; From the archive
        "archived": bool,
        # What atm finds wrong with the task (T-377); it isn't ready then
        "problems": List[str],
    },
    total=False,
)

DependencyItem = TypedDict(
    "DependencyItem",
    {
        # required
        "id": str,
        # required; Its status (", archived" added for an archived task), or missing
        "status": str,
        # required
        "name": str,
    },
    total=False,
)

# atm deps: what a task waits for, and what waits for it
DepsResult = TypedDict(
    "DepsResult",
    {
        # required
        "id": str,
        # required
        "waits_for": List["DependencyItem"],
        # required
        "waited_on_by": List["DependencyItem"],
    },
    total=False,
)

# Who works on a task, from which working copy, until when
Claim = TypedDict(
    "Claim",
    {
        # required
        "task": str,
        # required
        "agent": str,
        "branch": Optional[str],
        "worktree": Optional[str],
        # required
        "claimed_at": str,
        # required
        "lease_expires_at": str,
        "renewed_at": str,
    },
    total=False,
)

ShownDependency = TypedDict(
    "ShownDependency",
    {
        # required
        "id": str,
        # required; null: no such task
        "status": Optional[str],
        # required
        "archived": bool,
    },
    total=False,
)

# A task in full, as atm show gives it (show_tasks): with its effective priority, its dependencies' statuses, the tasks related to it either way and those that duplicate it
TaskShown = TypedDict(
    "TaskShown",
    {
        # required
        "task": "Task",
        # required
        "archived": bool,
        # required
        "priority": Optional[str],
        # required
        "started_by": Optional[str],
        # required
        "dependencies": List["ShownDependency"],
        # required
        "related": List[str],
        # required
        "duplicated_by": List[str],
    },
    total=False,
)

# What a status change (start_task, done_task, ...) answers
StatusChange = TypedDict(
    "StatusChange",
    {
        # required
        "task": "Task",
        # required; Its status before
        "previous": Optional[str],
        # required; Its claim now: the new one after start, else null
        "claim": Optional["Claim"],
        # required; After done: the tasks now ready
        "unblocked": List[str],
        # required; After done: the archive hint, at most once a day
        "hint": Optional["ArchiveHint"],
        # required; After reopen in a closed sprint: where it is now open, and how to move it (T-330)
        "stranded": Optional[str],
        # required; The claims whose lease ran out on the way (their tasks back to to do)
        "expired": List["Claim"],
        # required
        "view": "TaskShown",
    },
    total=False,
)

# A sprint as the overview shows it: done of its tasks but the skipped
SprintNumbers = TypedDict(
    "SprintNumbers",
    {
        # required
        "id": str,
        # required
        "state": Literal['planned', 'active', 'closed'],
        # required
        "due": Optional[str],
        # required
        "release": Optional[str],
        # required
        "done": int,
        # required; Its tasks but the skipped
        "total": int,
        # required
        "percent": int,
    },
    total=False,
)

# The active sprint for the overview's cards (SR-7.7), with its progress
ActiveSprint = TypedDict(
    "ActiveSprint",
    {
        # required
        "id": str,
        # required
        "state": Literal['planned', 'active', 'closed'],
        # required
        "due": Optional[str],
        # required
        "release": Optional[str],
        # required
        "done": int,
        # required; Its tasks but the skipped
        "total": int,
        # required
        "percent": int,
        # required
        "goal": Optional[str],
    },
    total=False,
)

# A live claim: who works on what
WorkingClaim = TypedDict(
    "WorkingClaim",
    {
        # required
        "task": str,
        # required; The task's name, capped at 80 characters
        "name": str,
        # required
        "agent": Optional[str],
        # required
        "branch": Optional[str],
    },
    total=False,
)

# A task by its ID and name (capped at 80 characters in a summary)
TaskName = TypedDict(
    "TaskName",
    {
        # required
        "id": str,
        # required
        "name": str,
    },
    total=False,
)

# The task completed last
LastDone = TypedDict(
    "LastDone",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "completed_at": str,
    },
    total=False,
)

# How many tasks need the user, by why
NeedsYou = TypedDict(
    "NeedsYou",
    {
        # required; In review, waiting for the reviewer
        "review": int,
        # required
        "blocked_on_user": int,
        # required; To do with the user now (SR-6.6)
        "together": int,
    },
    total=False,
)

# A task that needs the user, and why: what the "need you" chips list
NeedsYouItem = TypedDict(
    "NeedsYouItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "reason": Literal['review', 'blocked', 'together'],
    },
    total=False,
)

# The latest PR recorded on the tasks (atm pr)
GitPr = TypedDict(
    "GitPr",
    {
        # required
        "number": int,
        # required; Only a web address (docs/security-review.md F9)
        "url": Optional[str],
        # required
        "state": Literal['open', 'merged'],
    },
    total=False,
)

# The project's git line for its card (T-206)
GitSummary = TypedDict(
    "GitSummary",
    {
        # required; The main worktree's branch
        "branch": Optional[str],
        # required
        "pr": Optional["GitPr"],
    },
    total=False,
)

# A card on the All projects board: what a project's Kanban card shows, plus its column's fields
BoardCard = TypedDict(
    "BoardCard",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "type": Optional[str],
        # required; The task's sprint; null in the backlog
        "sprint": Optional[str],
        # required
        "priority": Optional[str],
        # required
        "labels": List[str],
        # In progress: who works on it
        "by": Optional[str],
        # In progress, review, blocked: since when
        "since": Optional[str],
        # Blocked: who decides
        "waiting_on": Optional[str],
        # Blocked: how many questions
        "questions": int,
        # Done: when
        "completed_at": Optional[str],
        # To do (T-349): done with the user
        "with_user": bool,
        # To do (T-349): the dependencies it waits for
        "waiting_for": List[str],
    },
    total=False,
)

# A project's column: its first cards, and how many there are in all
BoardColumn = TypedDict(
    "BoardColumn",
    {
        # required
        "count": int,
        # required
        "cards": List["BoardCard"],
    },
    total=False,
)

# The project's cards for the All projects board (T-257), by column
ProjectBoard = TypedDict(
    "ProjectBoard",
    {
        # required
        "todo": "BoardColumn",
        # required
        "active": "BoardColumn",
        # required
        "review": "BoardColumn",
        # required
        "blocked": "BoardColumn",
        # required
        "done": "BoardColumn",
    },
    total=False,
)

# The overview card's line, archived tasks included (T-255)
ProjectStats = TypedDict(
    "ProjectStats",
    {
        # required
        "started": Optional[str],
        # required
        "tasks": int,
        # required
        "agent_minutes": int,
    },
    total=False,
)

# A project's numbers for the overview and the project selector (api/projects' summary, atm summary)
ProjectSummary = TypedDict(
    "ProjectSummary",
    {
        # required
        "total": int,
        # required
        "done": int,
        # required
        "skipped": int,
        # required; done / (total - skipped)
        "percent": int,
        # required; null when none is active
        "active_sprint": Optional["ActiveSprint"],
        # required; Up to 4 sprints with tasks, in the project's order: the open ones, then the latest closed; null without sprints
        "recent_sprints": Optional[List["SprintNumbers"]],
        # required
        "agents_working": int,
        # required; Live claims, oldest first
        "working": List["WorkingClaim"],
        # required; Up to 2 tasks the loop takes next, in its order
        "next": List["TaskName"],
        # required
        "last_done": Optional["LastDone"],
        # required
        "needs_you": "NeedsYou",
        # required
        "needs_you_tasks": List["NeedsYouItem"],
        # required
        "git": "GitSummary",
        # required; meta.description, capped at 200 characters
        "description": Optional[str],
        # required
        "last_activity": Optional[str],
        # required; Oldest day first
        "completions_14d": List[int],
        # required
        "board": "ProjectBoard",
        # required; The same, of the active sprint's tasks only; null without an active sprint (T-340)
        "board_active_sprint": Optional["ProjectBoard"],
        # required
        "stats": "ProjectStats",
    },
    total=False,
)

# atm status: the active sprint and its progress
StatusSprint = TypedDict(
    "StatusSprint",
    {
        "id": str,
        "goal": str,
        "due": str,
        "release": str,
        "start": str,
        # required
        "done": int,
        # required; Its tasks but the skipped
        "tasks": int,
        # required; Batch tasks waiting for the end-of-sprint review (SR-4.4)
        "review_pending": List[str],
    },
    total=False,
)

InProgressItem = TypedDict(
    "InProgressItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required; The agent holding its claim, else who started it
        "by": Optional[str],
    },
    total=False,
)

InReviewItem = TypedDict(
    "InReviewItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "since": Optional[str],
        # In review in another worktree, on this branch (T-204)
        "branch": Optional[str],
    },
    total=False,
)

BlockedItem = TypedDict(
    "BlockedItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "waiting_on": Optional[str],
        # required
        "questions": List[str],
        # Blocked in another worktree, on this branch (T-204)
        "branch": Optional[str],
    },
    total=False,
)

WaitingItem = TypedDict(
    "WaitingItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required; Its dependencies not done yet
        "on": List[str],
    },
    total=False,
)

LargestTask = TypedDict(
    "LargestTask",
    {
        # required
        "id": str,
        # required
        "bytes": int,
    },
    total=False,
)

# An unfinished task whose sprint is closed (T-330)
OpenInClosedSprint = TypedDict(
    "OpenInClosedSprint",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "status": str,
        # required
        "sprint": str,
    },
    total=False,
)

StatusSuggestions = TypedDict(
    "StatusSuggestions",
    {
        # required
        "archive": "ArchiveHint",
    },
    total=False,
)

# atm status --json: the overview, in the scope's tasks
StatusReport = TypedDict(
    "StatusReport",
    {
        # required; null when none is active
        "sprint": Optional["StatusSprint"],
        # required
        "tasks": int,
        # required
        "by_status": Dict[str, int],
        # required; By sprint ID; backlog for the tasks in none
        "by_sprint": Dict[str, int],
        # required
        "by_type": Dict[str, int],
        # required
        "by_label": Dict[str, int],
        # required
        "in_progress": List["InProgressItem"],
        # required
        "in_review": List["InReviewItem"],
        # required
        "blocked": List["BlockedItem"],
        # required
        "waiting_on_dependencies": List["WaitingItem"],
        # required
        "next": Optional["TaskName"],
        # required; Why there is no next task, as atm next says it
        "next_reason": Optional[str],
        # required
        "file_bytes": int,
        # required
        "largest_tasks": List["LargestTask"],
        # required; How many tasks the archive holds
        "archived": int,
        # required
        "archive_bytes": int,
        # Unfinished tasks left in closed sprints, whatever the scope; absent without any
        "open_in_closed_sprints": List["OpenInClosedSprint"],
        "suggest": "StatusSuggestions",
    },
    total=False,
)

# GET /api/v1/projects/<p>/tasks: the tasks the query's filters keep, each with its derived values (docs/layered-architecture.md §6.2)
TaskViewList = TypedDict(
    "TaskViewList",
    {
        # required; ISO 8601 UTC
        "generated_at": str,
        # required
        "meta": "Meta",
        # required; How big the project's working task data is (tasks.json), for the archive notice
        "bytes": int,
        # required
        "tasks": List["TaskView"],
        # What atm finds wrong with the task file as a whole: entries left out, vocabularies that fell back to their defaults (T-377)
        "problems": List[str],
        "archive_hint": "ArchiveHint",
    },
    total=False,
)
