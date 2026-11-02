"""Opening a project: its task files (the JSON-in-git store) and git; and the user's registry of projects. Boot is the
one place that knows which store and which version control atm runs on, so it is also where the commands that are
about the JSON-in-git store itself (`atm paths`, the lock test, the hook's registering) reach it
(docs/layered-architecture.md §3.2)."""

import os
import time

from ..db.jsongit.gitrun import in_git
from ..db.jsongit.lock import locked
from ..db.jsongit.merge import ensure_merge_driver, merge_files
from ..db.jsongit.migrate import describe_migration, migrate
from ..db.jsongit.paths import check_task_folder, find_paths
from ..db.jsongit.store import JsonGitStore
from ..db.registry import FileRegistry, register, register_if_needed, registered_id
from ..lib.project import Project
from ..vcs.git import GitVcs


def open_project(cwd, actor=None, tolerant=False):
    """The project at or above `cwd`. `tolerant`: read a malformed task file as far as it can be read (T-377)."""
    return project_at(find_paths(cwd), actor, tolerant)


def project_at(paths, actor=None, tolerant=False):
    """The project whose task files `paths` locates: atm serve finds them once per project (`task_paths`, which runs
    git) and opens the project afresh for each request. The task folder's symlink checks run each time, so a file
    linked after the paths were found is refused too (docs/security-review.md F7, F8)."""
    check_task_folder(paths["root"], paths["dir"])
    return Project(JsonGitStore(paths, tolerant=tolerant), GitVcs(paths["root"]), actor)


def open_registry(home=None):
    """The user's registry of projects (the dashboard's list)."""
    return FileRegistry(home)


def register_project(project, wait=True):
    """List the project on the dashboard (every command that sets a project up does). Returns what the registry did."""
    return register(project.store.paths, wait)


def dashboard_id(project):
    """(id, listed) of the project on the dashboard, after registering it: the id the registry gave it (meta.id, or
    another when that one is taken) and whether it is listed (meta.dashboard.listed)."""
    register(project.store.paths)
    return registered_id(project.store.paths)


def set_up_store(project):
    """`atm init`: set the store up in its working copy: in git, the merge driver for the task files (registered in
    the clone, `.gitattributes` staged). Returns {git, gitattributes_added}."""
    paths = project.store.paths
    in_repository = in_git(paths["root"])
    return {"git": in_repository, "gitattributes_added": ensure_merge_driver(paths) if in_repository else False}


def migrate_folder(cwd, register_it=True):
    """`atm migrate` (and `atm serve`, before it shows a project): bring the task folder at or above `cwd` up to date
    (docs/v4/atm-cli.md). Returns {result, lines: what it did, in words, tasks_exists}; with `register_it`, the
    project is listed on the dashboard too (every skill runs migrate first)."""
    paths = find_paths(cwd)
    result = migrate(paths)
    exists = os.path.isfile(paths["tasks"])
    if register_it and exists:
        register(paths)
    return {"result": result, "lines": describe_migration(result, paths), "tasks_exists": exists}


def merge_versions(base, ours, theirs):
    """`atm merge-driver` (git runs it): merge the three versions of a task file task by task, writing the result
    over `ours`. Returns the conflicts (this branch's version was kept for each)."""
    return merge_files(base, ours, theirs)


def task_paths(cwd):
    """`atm paths`: where the project's task files are (the JSON-in-git store's own locations)."""
    return find_paths(cwd)


def hook_project(cwd, actor):
    """The project the plugin's hook (`heartbeat --auto`) works on in `cwd`, after listing it on the dashboard (one
    read of the registry when it is listed already); None when there are no task files there."""
    paths = find_paths(cwd)
    if not paths["tasks_exists"]:
        return None
    try:
        register_if_needed(paths)
    except Exception:  # noqa: BLE001 - the hook must never fail on the registry
        pass
    return project_at(paths, actor)


def hold_lock_for_test(cwd, seconds):
    """`atm _lock-test`: hold the task folder's lock for `seconds`, logging to .lock-test.log, so tests can check that
    two processes never hold it at once."""
    task_dir = find_paths(cwd)["dir"]
    log = os.path.join(task_dir, ".lock-test.log")
    with locked(task_dir):
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"acquire {os.getpid()}\n")
        time.sleep(seconds)
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"release {os.getpid()}\n")
