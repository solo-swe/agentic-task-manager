"""The JSON-in-git store's files (docs/v4/data-model.md §1): reading tasks.json strictly (every command) or tolerantly
(showing a malformed file, T-377), and the archive, tasks-archive.json, which the rules look archived tasks up in."""

import os

from ...common.core import AtmError, read_json
from ...common.model import archive_hint_for, tolerant_vocabulary, valid_task_id, vocabulary

ARCHIVE_FILE = "tasks-archive.json"


def archive_path(paths):
    return os.path.join(paths["dir"], ARCHIVE_FILE)


def load_archive(paths):
    """tasks-archive.json, or an empty archive when there is none (DM-1.2)."""
    p = archive_path(paths)
    if not os.path.isfile(p):
        return {"meta": {}, "tasks": []}
    data = read_json(p)
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        raise AtmError(f"{p} is not a valid archive (expected an object with a 'tasks' list)")
    return data


def archive_of(paths):
    """A lookup of the archived tasks in the archive of `paths` (`lookup(task_id)` -> task or None), read on the first
    lookup: what the store hands the rules for its project."""
    found = {}

    def lookup(task_id):
        if "tasks" not in found:
            found["tasks"] = {t.get("id"): t for t in load_archive(paths)["tasks"] if isinstance(t, dict)}
        return found["tasks"].get(task_id)

    return lookup


def read_task_file(paths):
    if not os.path.isfile(paths["tasks"]):
        raise AtmError(f"no tasks.json in {paths['dir']}. Run /atm:create-tasks first.")
    data = read_json(paths["tasks"])
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        raise AtmError(f"{paths['tasks']} is not a valid task file (expected an object with a 'tasks' list)")
    return data


def load_tasks(paths):
    data = read_task_file(paths)
    vocabulary(data)  # an unusable vocabulary stops every command before anything is written
    # Agents put task IDs into shell commands: an ID with shell characters stops every command (atm check lists them)
    for i, task in enumerate(data["tasks"]):
        # Every command looks tasks up by ID and follows dependencies: a malformed task stops them all with a clear
        # message instead of a traceback halfway through (docs/security-review.md F12). atm check lists them all.
        if not isinstance(task, dict) or not isinstance(task.get("id"), str):
            raise AtmError(f"{paths['tasks']}: task #{i + 1} isn't a task (an object with an \"id\"); run atm doctor")
        if not valid_task_id(task["id"]):
            raise AtmError(
                f"{paths['tasks']}: the task ID {task['id']!r} has characters atm doesn't allow (letters, digits, '.', "
                "'_' and '-' only): fix it in the file, then run atm doctor"
            )
        deps = task.get("dependencies", [])
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            raise AtmError(f"{paths['tasks']}: {task['id']}'s dependencies aren't a list of IDs; run atm doctor")
    return data


def load_tasks_tolerant(paths):
    """tasks.json read as far as it can be, for showing it (the dashboard): what `load_tasks` refuses is kept where it
    can be, and said. Entries that aren't tasks (no string "id") are left out; a task with an ID atm doesn't allow is
    kept as it is, one whose dependencies aren't a list of IDs with the IDs that can be read (so every task read fits
    the task schema). The data is for reading only. Returns (data, problems): problems as {"file": [...],
    "tasks": {id: [...]}}, in `atm check`'s words. A file that isn't a task file at all is still an error."""
    data = read_task_file(paths)
    problems = {"file": [], "tasks": {}}
    kept = []
    for i, task in enumerate(data["tasks"]):
        if not isinstance(task, dict) or not isinstance(task.get("id"), str):
            problems["file"].append(f"task #{i + 1} isn't a task (an object with an \"id\"): left out")
            continue
        own = []
        if not valid_task_id(task["id"]):
            own.append(f"the ID {task['id']!r} has characters atm doesn't allow (letters, digits, '.', '_' and '-')")
        deps = task.get("dependencies", [])
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            # Shown with the IDs that can be read, so what reads it gets a task of the schema's shape
            task["dependencies"] = [d for d in deps if isinstance(d, str)] if isinstance(deps, list) else []
            own.append("its dependencies aren't a list of task IDs (shown with the IDs that can be read)")
        if own:
            problems["tasks"][task["id"]] = own
        kept.append(task)
    data["tasks"] = kept
    # The rules read the vocabularies from meta: what doesn't work is left out of it (the problems say what)
    _, vocabulary_problems, data["meta"] = tolerant_vocabulary(data.get("meta"))
    problems["file"] += vocabulary_problems
    return data, problems


def archive_hint(paths, data):
    """The suggestion to archive (CLI-5.1) for the task file of `paths`, or None (common/model.py archive_hint_for)."""
    return archive_hint_for(data, os.path.getsize(paths["tasks"]) if os.path.isfile(paths["tasks"]) else 0)
