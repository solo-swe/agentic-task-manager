"""The in-memory store: a project's data in plain lists, for atm-lib's tests. It also shows that atm-lib depends on
the store's interface only: the contract tests (tests/lib/) run the same checks on it and on the JSON-in-git store."""

import copy
import json

from ..common.errors import Conflict, Invalid
from ..common.model import settings_of, vocabulary
from .working import Working


def empty_claims():
    return {"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}}


class MemoryStore:
    def __init__(self, tasks=None, archive=None, meta=None, claims=None, problems=None, created=True):
        """`problems`: what a tolerant read would have found ({"file": [...], "tasks": {id: [...]}}), for the tests of
        what atm-lib makes of them; the memory holds data, not a file to read. `created` false: a project with no
        task data yet (`atm init` creates it)."""
        self.created = created
        self.version = 0  # counts the changes: the stamp
        self.meta = dict(meta or {})
        self.tasks = list(tasks or [])
        self.archive = list(archive or [])
        self.claims = {**empty_claims(), **(claims or {})}
        self._problems = {"file": [], "tasks": {}, **(problems or {})}

    def problems(self):
        return self._problems

    def get_vocabularies(self):
        return vocabulary(self.document())

    def get_settings(self):
        return settings_of(self.meta)

    def list_sprints(self):
        return self.get_vocabularies()["sprints"]

    def list_releases(self):
        return self.get_vocabularies()["releases"]

    def list_components(self):
        return self.get_vocabularies()["components"]

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

    def files_for_commit(self):
        return []  # nothing of the memory travels with code commits

    def exists(self):
        return self.created

    def create(self, document):
        if self.created:
            raise Conflict("the project has task data already")
        self.meta, self.tasks, self.created = dict(document.get("meta") or {}), list(document["tasks"]), True
        self.version += 1

    def stamp(self):
        return {"tasks": self.version, "archive": self.version, "claims": self.version, "head": None, "copies": []}

    def logo(self, meta):
        return None  # no files: a logo would be uploaded to a store like this one

    def check_logo(self, relative):
        raise Invalid(f"{relative}: this project keeps no files for a logo")

    def working_copies(self):
        return []

    def copy_tasks(self, folder):
        return []

    def raw(self):
        return {"document": self.document(), "archive": self.archive, "archive_error": None}

    def change(self, fn):
        """Run `fn(work)` on copies, and keep them only when it succeeds: a failed change leaves the store as it was."""
        original, original_claims = self.document(), self.claims
        document, claims = copy.deepcopy(original), copy.deepcopy(original_claims)
        work = Working(
            document,
            claims,
            {"worktree": None, "branch": None},
            lambda: self.sizes()["tasks"],
            lambda: (original, original_claims),  # replaced, not changed, when the change is kept
            lambda: {"meta": {}, "tasks": copy.deepcopy(self.archive)},
        )
        result = fn(work)
        self.meta, self.tasks, self.claims = document["meta"], document["tasks"], claims
        if work.archive_changed():
            self.archive = work.archive()["tasks"]
        self.changes = work.changes()  # what the last change did, for the tests
        self.version += 1
        work.raise_deferred()
        return result
