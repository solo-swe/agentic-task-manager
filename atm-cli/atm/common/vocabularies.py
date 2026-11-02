"""The status kinds and the default vocabularies, generated from common/schema/vocabularies.json by
atm-cli/build.py: don't edit it, change that file and run `npm run build:cli` in dashboard/ (the pre-commit hook does it too)."""

KINDS = ('todo', 'active', 'review', 'blocked', 'done', 'skipped')
DEFAULT_STATUSES = [
    {
        "id": "todo",
        "label": "To Do",
        "kind": "todo"
    },
    {
        "id": "in_progress",
        "label": "In Progress",
        "kind": "active"
    },
    {
        "id": "in_review",
        "label": "In Review",
        "kind": "review"
    },
    {
        "id": "blocked",
        "label": "Blocked",
        "kind": "blocked"
    },
    {
        "id": "done",
        "label": "Done",
        "kind": "done"
    },
    {
        "id": "skipped",
        "label": "Skipped",
        "kind": "skipped"
    }
]
DEFAULT_TYPES = [
    {
        "id": "feature",
        "label": "Feature"
    },
    {
        "id": "bugfix",
        "label": "Bugfix"
    },
    {
        "id": "test",
        "label": "Test"
    },
    {
        "id": "infra",
        "label": "Infra"
    },
    {
        "id": "docs",
        "label": "Docs"
    },
    {
        "id": "refactor",
        "label": "Refactor"
    },
    {
        "id": "polish",
        "label": "Polish"
    }
]
DEFAULT_PRIORITIES = [
    {
        "id": "P0",
        "label": "Critical"
    },
    {
        "id": "P1",
        "label": "High"
    },
    {
        "id": "P2",
        "label": "Medium"
    },
    {
        "id": "P3",
        "label": "Low"
    }
]
DEFAULT_PRIORITY = "P2"
