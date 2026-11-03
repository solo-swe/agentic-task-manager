"""The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
has: tasks.json and the archive (files.py), the claims store shared by a clone's worktrees (claims.py), under the
task folder's lock (lock.py); its worktrees' copies (worktrees.py) and the project's logo (logo.py)."""

import contextlib
import json
import os

from ...common.core import AtmError, read_json, to_json, write_json_atomic, write_text_atomic
from ...common.errors import Conflict
from ...common.model import settings_of, vocabulary
from ..working import Working
from .claims import claims_store_path, locked_with_claims, read_store, write_store
from .files import archive_of, archive_path, load_archive, load_tasks, load_tasks_tolerant
from ...host.git import checked_out_branch
from .lock import locked
from .logo import find_logo, logo_file
from .merge import ensure_archive_merge
from .paths import find_paths
from .worktrees import worktrees


def file_stamp(p):
    """(size, nanosecond mtime) of a file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


class JsonGitStore:
    def __init__(self, paths, tolerant=False):
        """`tolerant`: read a malformed task file as far as it can be read, and say what is wrong (`problems()`),
        instead of refusing it: for showing it (the REST API's reads, T-377). Writes are strict either way."""
        self.paths = paths
        self.tolerant = tolerant
        self._data = None
        self._archived = None  # the lookup of this project's archive, made on first use
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

    def get_settings(self):
        return settings_of(self._loaded().get("meta"))

    def list_sprints(self):
        return self.get_vocabularies()["sprints"]

    def list_releases(self):
        return self.get_vocabularies()["releases"]

    def list_components(self):
        return self.get_vocabularies()["components"]

    def list_tasks(self):
        return self._loaded()["tasks"]

    def get_task(self, task_id):
        for task in self._loaded()["tasks"]:
            if task.get("id") == task_id:
                return task
        return None

    def get_archived_task(self, task_id):
        """This project's archived task with this ID, or None; its own archive, read once per store."""
        if self._archived is None:
            self._archived = archive_of(self.paths)
        return self._archived(task_id)

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

    def exists(self):
        return os.path.isfile(self.paths["tasks"])

    def create(self, document):
        """Write the project's first task data (`atm init`); Conflict if it has some already."""
        folder = self.paths["dir"]
        with locked(folder) if os.path.isdir(folder) else contextlib.nullcontext():
            if self.exists():
                raise Conflict(f"{self.paths['tasks']} exists already")
            os.makedirs(folder, exist_ok=True)
            write_json_atomic(self.paths["tasks"], document)
        self._data = None

    def stamp(self):
        """What changes when the data does, for the REST API's ETags: per part ({tasks, archive, claims, head, copies}),
        a value that changes with it (here, each file's size and mtime; `head` the checked-out branch's; `copies` the
        worktrees and their task files'), None for a part that isn't there."""
        relative = os.path.relpath(self.paths["tasks"], self.paths["root"])
        return {
            "tasks": file_stamp(self.paths["tasks"]),
            "archive": file_stamp(archive_path(self.paths)),
            "claims": file_stamp(claims_store_path(self.paths)),
            "head": file_stamp(os.path.join(self.paths["root"], ".git", "HEAD")),
            "copies": [(t, file_stamp(os.path.join(t["path"], relative))) for t in worktrees(self.paths["root"])],
        }

    def logo(self, meta):
        """The project's logo, a file in the working copy: {path (relative), file, type, source: "set" (meta.logo) or
        "found" (in the usual places)}, or None."""
        found = find_logo(self.paths["root"], meta)
        return {"path": found[0], "file": found[1], "type": found[2], "source": found[3]} if found else None

    def check_logo(self, relative):
        """Refuse (AtmError, exit code 2) a logo the dashboard can't be given: not an image atm serves, outside the
        project, missing or too large."""
        logo_file(self.paths["root"], relative)

    def working_copies(self):
        """The project's working copies, main first: its git worktrees, [{path, branch, main}]; [] outside git."""
        return worktrees(self.paths["root"])

    def copy_tasks(self, folder):
        """The tasks in the working copy at `folder` as its tasks.json has them (entries that aren't tasks left out),
        for showing beside the main one's; [] when it has none."""
        data = read_json(os.path.join(folder, os.path.relpath(self.paths["tasks"], self.paths["root"])))
        tasks = data.get("tasks") if isinstance(data, dict) else None
        return [t for t in tasks or [] if isinstance(t, dict) and isinstance(t.get("id"), str)]

    def copy_store(self, folder):
        """The working copy at `folder` as a store of its own (its tasks.json and archive), read tolerantly, for its
        tasks' derived values. Its task folder's symlink checks run as for any project (docs/security-review.md F8)."""
        return JsonGitStore(find_paths(folder), tolerant=True)

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
                # The branch as everywhere else (§11.8): from HEAD, a detached one as its short SHA
                lambda: {"worktree": root, "branch": checked_out_branch(root)},
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
        self._data = self._archived = None  # this store's snapshot is out of date now
        work.raise_deferred()
        return result
