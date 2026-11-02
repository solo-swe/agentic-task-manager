"""Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
`npm run build:cli` in dashboard/ (the pre-commit hook does it too). TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has
no typing.Required)."""

from typing import List, TypedDict

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
        "pr": PullRequest,
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
        "blocks": List[BlockRound],
        # Past reviews, oldest first
        "reviews": List[ReviewRound],
        # Handoffs, outcomes, review requests, answers, rejections (docs/v4/data-model.md §4)
        "notes": List[Note],
        "last_failure": LastFailure,
        # The sprint it is in (a sprint's name, meta.sprints); absent: the backlog
        "sprint": str,
        # Each sprint it was in, from when to when; the open entry has no `to` (SR-2.5)
        "sprint_history": List[SprintHistoryEntry],
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
