"""The user's project registry ($ATM_HOME/projects.json): the projects the dashboard server shows (the `Registry` of
docs/layered-architecture.md §3.2).

One file per user, guarded by $ATM_HOME/.lock (not by any project's lock, and never taken while holding one), written
atomically. A linked git worktree registers its repository's main worktree. Format: docs/multi-project-api.md, 5.1.
"""

import hashlib
import os
import re

from ..common.core import TASK_DIR, TASKS_FILE, AtmError, atm_home, ensure_home, now, now_iso, parse_iso, read_json
from ..common.core import write_json_atomic
from .jsongit.lock import locked
from .jsongit.paths import find_paths
from .jsongit.worktrees import main_worktree

REGISTRY_FILE = "projects.json"
REGISTRY_VERSION = 1
SEEN_EVERY = 24 * 3600  # seconds between seen_at updates, so the registry isn't rewritten on every command

# A project id is a key in URLs (#project=<id>, p/<id>/...): letters, digits, '.', '_' and '-'
PROJECT_ID = re.compile(r"[A-Za-z0-9._-]{1,64}")


def unique_id(registry, path, wanted):
    """The id for the project at `path`: `wanted` unless another project already has it or it isn't a valid id; then
    one made from the path, so two projects never shadow each other (F18)."""
    taken = {e.get("id") for e in registry["projects"] if e.get("path") != path}
    if PROJECT_ID.fullmatch(wanted) and wanted not in taken:
        return wanted
    suffix = hashlib.sha1(path.encode("utf-8")).hexdigest()[:7]
    base = wanted if PROJECT_ID.fullmatch(wanted) and len(wanted) <= 56 else "p"
    return f"{base}-{suffix}"


def project_meta(path):
    """The meta of the project at `path`, or None without a readable tasks.json."""
    data = read_json(os.path.join(path, TASK_DIR, TASKS_FILE))
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        return None
    return data.get("meta") if isinstance(data.get("meta"), dict) else {}


def project_id(meta, path):
    """meta.id, else "p" and 7 hex characters of the path's SHA-1 (stable until meta.id is set)."""
    if isinstance(meta.get("id"), str) and meta["id"]:
        return meta["id"]
    return "p" + hashlib.sha1(path.encode("utf-8")).hexdigest()[:7]


def project_name(meta, path):
    return meta.get("project") if isinstance(meta.get("project"), str) and meta["project"] else os.path.basename(path)


def listed(meta):
    dashboard = meta.get("dashboard")
    return not (isinstance(dashboard, dict) and dashboard.get("listed") is False)


def is_missing(entry):
    return project_meta(entry["path"]) is None


def empty_registry():
    return {"version": REGISTRY_VERSION, "projects": []}


def read_registry(home):
    """(registry, damaged): an unreadable projects.json is moved aside to projects.json.damaged-<time> (its path is
    returned as `damaged`) and replaced by an empty registry. Call it holding the registry lock."""
    path = os.path.join(home, REGISTRY_FILE)
    if not os.path.exists(path):
        return empty_registry(), None
    data = read_json(path)
    entries = data.get("projects") if isinstance(data, dict) else None
    if isinstance(entries, list) and all(isinstance(e, dict) and isinstance(e.get("path"), str) for e in entries):
        return data, None
    damaged = f"{path}.damaged-{now().strftime('%Y%m%dT%H%M%SZ')}"
    os.replace(path, damaged)
    return empty_registry(), damaged


def write_registry(home, registry):
    write_json_atomic(os.path.join(home, REGISTRY_FILE), registry)


def find_entry(registry, path):
    return next((e for e in registry["projects"] if e["path"] == path), None)


def entry_for(registry, path, meta, stamp):
    return {
        "id": unique_id(registry, path, project_id(meta, path)),
        "name": project_name(meta, path),
        "path": path,
        "added_at": stamp,
        "seen_at": stamp,
    }


def seen_long_ago(entry):
    """True when the entry's seen_at is a day old (or unreadable): time to refresh it."""
    try:
        return (now() - parse_iso(entry.get("seen_at"))).total_seconds() >= SEEN_EVERY
    except (TypeError, ValueError):
        return True


def update_entry(registry, path, meta):
    """Add, refresh or (when the project opted out) remove the entry for `path`. Returns what happened: "added",
    "updated", "unchanged", "removed" or "unlisted"."""
    entry = find_entry(registry, path)
    if not listed(meta):
        if entry:
            registry["projects"].remove(entry)
            return "removed"
        return "unlisted"
    stamp = now_iso()
    if entry is None:
        registry["projects"].append(entry_for(registry, path, meta, stamp))
        return "added"
    fresh = {"id": unique_id(registry, path, project_id(meta, path)), "name": project_name(meta, path)}
    changed = any(entry.get(k) != v for k, v in fresh.items())
    if not (changed or seen_long_ago(entry)):
        return "unchanged"
    entry.update(fresh)
    entry["seen_at"] = stamp
    return "updated"


def register(paths, wait=True):
    """Register the project of `paths` (its main worktree). With wait=False the registry lock is tried once and the
    project is skipped if the lock is taken (the plugin's hook must never wait). Returns update_entry's result,
    "busy" when skipped, or None when the project has no tasks.json."""
    path = main_worktree(paths["root"])
    meta = project_meta(path)
    if meta is None:
        meta = project_meta(paths["root"])
    if meta is None:
        return None
    home = atm_home()
    ensure_home(home)
    try:
        with locked(home, timeout=None if wait else 0):
            registry, _ = read_registry(home)
            result = update_entry(registry, path, meta)
            if result not in ("unchanged", "unlisted"):
                write_registry(home, registry)
            return result
    except AtmError:
        if wait:
            raise
        return "busy"


def register_if_needed(paths):
    """The plugin hook's cheap path, run after every tool call: one read of projects.json, and registering (without
    waiting for the lock) only when the project isn't listed yet or was last seen a day ago."""
    path = main_worktree(paths["root"])
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE))
    entries = registry.get("projects") if isinstance(registry, dict) else None
    entry = next((e for e in entries or [] if isinstance(e, dict) and e.get("path") == path), None)
    if entry is not None and not seen_long_ago(entry):
        return "unchanged"
    return register(paths, wait=False)


def registered_id(paths):
    """(id, listed) of the project at `paths` as the registry has it: the id it gave the project (meta.id, or another
    when that one is taken, unique_id), and whether it is listed on the dashboard."""
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"]) or {}
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE)) or {}
    entry = next((e for e in registry.get("projects", []) if isinstance(e, dict) and e.get("path") == path), None)
    return (entry or {}).get("id") or project_id(meta, path), listed(meta)


def resolve_project(target):
    """The main worktree folder of the project at `target` (a path), and its meta; refuses a folder without tasks."""
    folder = os.path.abspath(os.path.expanduser(target or os.getcwd()))
    if not os.path.isdir(folder):
        raise AtmError(f"{target}: not a directory", 2)
    paths = find_paths(folder)
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"])
    if meta is None:
        raise AtmError(f"no tasks.json in {os.path.join(path, TASK_DIR)}. Run /atm:create-tasks there first.")
    return path, meta


class FileRegistry:
    """The registry in $ATM_HOME/projects.json. Each method holds the registry's lock while it reads and writes, and
    returns `damaged`: where an unreadable projects.json was moved (read_registry), or None."""

    def __init__(self, home=None):
        self.home = home or atm_home()

    def _locked(self):
        ensure_home(self.home)
        return locked(self.home)

    def file(self):
        """The registry's file: the server re-reads it when it changes."""
        return os.path.join(self.home, REGISTRY_FILE)

    def list_projects(self):
        """{projects: the entries, each with `missing` (no readable tasks.json there now), damaged}."""
        with self._locked():
            registry, damaged = read_registry(self.home)
            if damaged:
                write_registry(self.home, registry)
        return {"projects": [{**e, "missing": is_missing(e)} for e in registry["projects"]], "damaged": damaged}

    def add_project(self, target):
        """Register the project at `target` (a folder in it). {result: update_entry's, project: its entry, damaged}."""
        path, meta = resolve_project(target)
        if not listed(meta):
            raise AtmError(f"{path} opts out of the dashboard (meta.dashboard.listed is false)")
        with self._locked():
            registry, damaged = read_registry(self.home)
            result = update_entry(registry, path, meta)
            write_registry(self.home, registry)
        return {"result": result, "project": find_entry(registry, path), "damaged": damaged}

    def remove_project(self, target):
        """Unregister the project with the ID or path `target`. {removed: its entry, damaged}."""
        if not target:
            raise AtmError("remove needs a project ID or path (see `atm projects`)", 2)
        candidates = {target, os.path.abspath(os.path.expanduser(target))}
        if os.path.isdir(target):
            candidates.add(main_worktree(find_paths(os.path.abspath(target))["root"]))
        with self._locked():
            registry, damaged = read_registry(self.home)
            entry = next((e for e in registry["projects"] if e.get("id") in candidates or e["path"] in candidates), None)
            if entry:
                registry["projects"].remove(entry)
            if entry or damaged:
                write_registry(self.home, registry)
        if entry is None:
            raise AtmError(f"no registered project {target} (see `atm projects`)")
        return {"removed": entry, "damaged": damaged}

    def prune(self):
        """Unregister the projects whose task folder is gone. {removed: their entries, damaged}."""
        with self._locked():
            registry, damaged = read_registry(self.home)
            gone = [e for e in registry["projects"] if is_missing(e)]
            registry["projects"] = [e for e in registry["projects"] if e not in gone]
            if gone or damaged:
                write_registry(self.home, registry)
        return {"removed": gone, "damaged": damaged}

    def stamp(self):
        """The file's size and mtime (None while there is none): it changes when the list does."""
        try:
            st = os.stat(self.file())
        except OSError:
            return None
        return (st.st_size, st.st_mtime_ns)

    def entries(self):
        """The entries as the file has them now, without the lock (the server reads them on every request)."""
        data = read_json(self.file())
        entries = data.get("projects") if isinstance(data, dict) else None
        return [e for e in entries or [] if isinstance(e, dict) and isinstance(e.get("path"), str)]
