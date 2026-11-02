"""The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
has. For now it wraps the code from before the layers (`load_tasks`, the archive, the claims store); that code moves
in here as the migration's slices do (§9)."""

import json
import os

from ...claims import locked_with_claims, read_store, write_store
from ...core import AtmError, git, read_json, to_json, write_json_atomic, write_text_atomic
from ...merge import ensure_archive_merge
from ...model import archive_path, archived_task, load_archive, load_tasks, load_tasks_tolerant, vocabulary
from ..working import Working


class JsonGitStore:
    def __init__(self, paths, tolerant=False):
        """`tolerant`: read a malformed task file as far as it can be read, and say what is wrong (`problems()`),
        instead of refusing it: for showing it (the REST API's reads, T-377). Writes are strict either way."""
        self.paths = paths
        self.tolerant = tolerant
        self._data = None
        self._problems = {"file": [], "tasks": {}}

    def _loaded(self):
        # Read once per store: one atm command works on one snapshot of the file
        if self._data is None:
            if self.tolerant:
                self._data, self._problems = load_tasks_tolerant(self.paths)
            else:
                self._data = load_tasks(self.paths)
        return self._data

    def problems(self):
        """What a tolerant read found wrong: {"file": [...], "tasks": {id: [...]}}; nothing for a strict one."""
        self._loaded()
        return self._problems

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

    def files_for_commit(self):
        """tasks.json travels with the code: `atm commit` commits it with the task's files (relative to the root)."""
        return [os.path.relpath(self.paths["tasks"], self.paths["root"])]

    def raw(self):
        """The task data as stored, unchecked, for `atm check` to report on: {"document": the file's JSON (None when
        it isn't a task file), "archive": the archived tasks, "archive_error": why the archive can't be read}."""
        if not os.path.isfile(self.paths["tasks"]):
            raise AtmError(f"no tasks.json in {self.paths['dir']}. Run /atm:create-tasks first.")
        data = read_json(self.paths["tasks"])
        if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
            data = None
        try:
            archive, error = load_archive(self.paths)["tasks"], None
        except AtmError as e:
            archive, error = [], str(e)
        return {"document": data, "archive": archive, "archive_error": error}

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
                lambda: load_archive(self.paths),
            )
            result = fn(work)

            def write_tasks():
                # Written only when its text changed: a read-only change (atm next, a heartbeat) leaves the file and
                # its mtime alone, so open dashboards don't reload for nothing
                text = to_json(work.document) + "\n"
                with open(tasks_file, encoding="utf-8") as f:
                    stored = f.read()
                # The same data in another layout (a file edited by hand) is unchanged too
                if stored != text and json.loads(stored) != work.document:
                    write_text_atomic(tasks_file, text)

            def write_archive():
                if work.archive_changed():
                    write_json_atomic(archive_path(self.paths), work.archive())
                    if work.archive_first:
                        ensure_archive_merge(self.paths)  # tasks moved in: the archive merges task by task too

            # A move into the archive writes it first; anything else writes tasks.json first (DM-5.2)
            for write in (write_archive, write_tasks) if work.archive_first else (write_tasks, write_archive):
                write()
            if work.store_changed():
                write_store(self.paths, work.claims_store)
        self._data = None  # this store's snapshot is out of date now
        work.raise_deferred()
        return result
