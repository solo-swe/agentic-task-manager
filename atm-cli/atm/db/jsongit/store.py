"""The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
has. For now it wraps the code from before the layers (`load_tasks`, the archive, the claims store); that code moves
in here as the migration's slices do (§9)."""

import os

from ...claims import read_store
from ...model import archive_path, archived_task, load_archive, load_tasks, vocabulary


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
