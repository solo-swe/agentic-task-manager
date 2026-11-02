"""The in-memory store: a project's data in plain lists, for atm-lib's tests. It also shows that atm-lib depends on
the store's interface only: the contract tests (tests/lib/) run the same checks on it and on the JSON-in-git store."""

import json

from ..model import vocabulary


def empty_claims():
    return {"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}}


class MemoryStore:
    def __init__(self, tasks=None, archive=None, meta=None, claims=None):
        self.meta = dict(meta or {})
        self.tasks = list(tasks or [])
        self.archive = list(archive or [])
        self.claims = {**empty_claims(), **(claims or {})}

    def get_vocabularies(self):
        return vocabulary(self.document())

    def list_tasks(self):
        return self.tasks

    def get_task(self, task_id):
        return next((t for t in self.tasks if t.get("id") == task_id), None)

    def get_archived_task(self, task_id):
        return next((t for t in self.archive if t.get("id") == task_id), None)

    def list_archived(self):
        return self.archive

    def read_claims(self):
        return self.claims

    def document(self):
        return {"meta": self.meta, "tasks": self.tasks}

    def sizes(self):
        # What the task files would be: the same JSON, indented as atm writes it
        size = lambda value: len(json.dumps(value, indent=2, ensure_ascii=False).encode("utf-8"))
        return {"tasks": size(self.document()), "archive": size({"meta": {}, "tasks": self.archive}) if self.archive else 0}

    def working_copy(self):
        return None
