"""The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
has. For now it wraps the code from before the layers (`load_tasks`, the archive, the claims store); that code moves
in here as the migration's slices do (§9)."""

import os

from ...claims import locked_with_claims, read_store, write_store
from ...core import git, read_json, write_json_atomic
from ...model import archive_path, archived_task, load_archive, load_tasks, vocabulary
from ..working import Working


class JsonGitStore:
    def __init__(self, paths):
        self.paths = paths
        self._data = None

    def _loaded(self):
        # Read once per store: one atm command works on one snapshot of the file
        if self._data is None:
            self._data = load_tasks(self.paths)
        return self._data

    def get_vocabularies(self):
        return vocabulary(self._loaded())

    def list_tasks(self):
        return self._loaded()["tasks"]

    def get_task(self, task_id):
        for task in self._loaded()["tasks"]:
            if task.get("id") == task_id:
                return task
        return None

    def get_archived_task(self, task_id):
        self._loaded()  # the archive is looked up for the file it belongs to
        return archived_task(task_id)

    def list_archived(self):
        return [t for t in load_archive(self.paths)["tasks"] if isinstance(t, dict)]

    def read_claims(self):
        return read_store(self.paths)

    def document(self):
        return self._loaded()

    def sizes(self):
        archive = archive_path(self.paths)
        return {
            "tasks": os.path.getsize(self.paths["tasks"]),
            "archive": os.path.getsize(archive) if os.path.isfile(archive) else 0,
        }

    def working_copy(self):
        return self.paths["root"]

    def change(self, fn):
        """Run `fn(work)` under the task folder's lock (and the claims store's), as every atm write always has: read
        tasks.json and the claims store, let `fn` change them, write what changed. If `fn` raises, nothing is written.
        (docs/layered-architecture.md §3.3: the transitional form of `apply`.)"""
        root, tasks_file = self.paths["root"], self.paths["tasks"]
        with locked_with_claims(self.paths):
            work = Working(
                load_tasks(self.paths),
                read_store(self.paths),
                lambda: {"worktree": root, "branch": git(["rev-parse", "--abbrev-ref", "HEAD"], root)},
                lambda: os.path.getsize(tasks_file),
                # The files still hold what was read: nothing is written before fn returns
                lambda: (read_json(tasks_file), read_store(self.paths)),
            )
            result = fn(work)
            # tasks.json is written on every change, as atm always has (comparing first would cost more than writing)
            write_json_atomic(self.paths["tasks"], work.document)
            if work.store_changed():
                write_store(self.paths, work.claims_store)
        self._data = None  # this store's snapshot is out of date now
        work.raise_deferred()
        return result
