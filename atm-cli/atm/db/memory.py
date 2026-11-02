"""The in-memory store: a project's data in plain lists, for atm-lib's tests. It also shows that atm-lib depends on
the store's interface only: the contract tests (tests/lib/) run the same checks on it and on the JSON-in-git store."""

from ..model import vocabulary


class MemoryStore:
    def __init__(self, tasks=None, archive=None, meta=None):
        self.meta = dict(meta or {})
        self.tasks = list(tasks or [])
        self.archive = list(archive or [])

    def get_vocabularies(self):
        return vocabulary({"meta": self.meta, "tasks": self.tasks})

    def list_tasks(self):
        return self.tasks

    def get_task(self, task_id):
        return next((t for t in self.tasks if t.get("id") == task_id), None)

    def get_archived_task(self, task_id):
        return next((t for t in self.archive if t.get("id") == task_id), None)
