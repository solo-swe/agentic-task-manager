"""Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
`npm run build:cli` in dashboard/ (the pre-commit hook does it too). TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has
no typing.Required)."""

from typing import List, Literal, TypedDict

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
        # In the review kind: waiting, changes_requested or approved (SR-6.1)
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
