# Generated from atm-cli/atm/ by atm-cli/build.py. Don't edit this file: edit the modules and run
# `npm run build:cli` in dashboard/ (the pre-commit hook does it too).
"""atm: the Agentic Task Manager helper that the skills run.

Run through the `atm` launcher, which checks for Python 3.9+. Standard library only: the plugin is used as cloned,
with nothing to install.
"""

import argparse
import base64
import contextlib
import copy
import errno
import hashlib
import json
import os
import random
import re
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from datetime import datetime, timedelta, timezone
from fnmatch import fnmatchcase
from typing import Any, Callable, List, Literal, Optional, Protocol, TypedDict
from urllib.parse import parse_qs, urlsplit


# ==== common/errors.py ====
# atm's errors (docs/layered-architecture.md §4.2, §5): atm-lib raises them by what went wrong, and each front end
# says it its own way: the CLI as a message and an exit code (`code`), the REST API as an HTTP status.

class AtmFailure(Exception):
    """Something atm can't do, with a message for the person or agent who asked. `code` is the CLI's exit code."""

    code = 1

    def __init__(self, message, code=None):
        super().__init__(message)
        if code is not None:
            self.code = code


class NotFound(AtmFailure):
    """No such task, sprint, release, component or project."""


class Invalid(AtmFailure):
    """The request itself is wrong: an unknown option value, a malformed ID (exit code 2, like a usage error)."""

    code = 2


class Conflict(AtmFailure):
    """The data changed since it was read, or the change isn't possible in the task's state."""


class NothingAvailable(AtmFailure):
    """No task to hand out (exit code 3, `atm next`)."""

    code = 3


class Claimed(AtmFailure):
    """Another agent holds the task, or the caller's lease ran out (exit code 4)."""

    code = 4


# ==== common/vocabularies.py ====
# The status kinds and the default vocabularies, generated from common/schema/vocabularies.json by
# atm-cli/build.py: don't edit it, change that file and run `npm run build:cli` in dashboard/ (the pre-commit hook does it too).

KINDS = ('todo', 'active', 'review', 'blocked', 'done', 'skipped')
DEFAULT_STATUSES = [
    {
        "id": "todo",
        "label": "To Do",
        "kind": "todo"
    },
    {
        "id": "in_progress",
        "label": "In Progress",
        "kind": "active"
    },
    {
        "id": "in_review",
        "label": "In Review",
        "kind": "review"
    },
    {
        "id": "blocked",
        "label": "Blocked",
        "kind": "blocked"
    },
    {
        "id": "done",
        "label": "Done",
        "kind": "done"
    },
    {
        "id": "skipped",
        "label": "Skipped",
        "kind": "skipped"
    }
]
DEFAULT_TYPES = [
    {
        "id": "feature",
        "label": "Feature"
    },
    {
        "id": "bugfix",
        "label": "Bugfix"
    },
    {
        "id": "test",
        "label": "Test"
    },
    {
        "id": "infra",
        "label": "Infra"
    },
    {
        "id": "docs",
        "label": "Docs"
    },
    {
        "id": "refactor",
        "label": "Refactor"
    },
    {
        "id": "polish",
        "label": "Polish"
    }
]
DEFAULT_PRIORITIES = [
    {
        "id": "P0",
        "label": "Critical"
    },
    {
        "id": "P1",
        "label": "High"
    },
    {
        "id": "P2",
        "label": "Medium"
    },
    {
        "id": "P3",
        "label": "Low"
    }
]
DEFAULT_PRIORITY = "P2"


# ==== common/core.py ====
# Constants, errors, output, time helpers and atomic writes, shared by every layer. Running git is not here: the
# store runs it for the task files (db/jsongit/gitrun.py), the vcs for the code (vcs/git.py).

# Where the built plugin/scripts/atm.py lives (only the built file is run, never these modules)
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
TASK_DIR = ".agentic-task-manager"
TASKS_FILE = "tasks.json"
DASHBOARD_FILE = "tasks-dashboard.html"


class AtmError(AtmFailure):
    """The errors of the modules from before the layers: atm's errors (common/errors.py) without a kind."""

    def __init__(self, message, code=1):
        super().__init__(message, code)


class AtmExit(Exception):
    """End with an exit code after normal output (not an error)."""

    def __init__(self, code):
        super().__init__(code)
        self.code = code


def out(text=""):
    sys.stdout.write(text + "\n")


def to_json(value):
    return json.dumps(value, indent=2, ensure_ascii=False)


def rel(root, p):
    return os.path.relpath(p, root)


def plugin_version():
    with open(os.path.join(PLUGIN_ROOT, ".claude-plugin", "plugin.json"), encoding="utf-8") as f:
        return json.load(f)["version"]


def parse_version(text):
    parts = []
    for part in (text or "").split("."):
        m = re.match(r"\d+", part)
        parts.append(int(m.group()) if m else 0)
    return parts


def now():
    """The current time; ATM_NOW (ISO 8601 UTC) overrides it, for tests."""
    fixed = os.environ.get("ATM_NOW")
    return parse_iso(fixed) if fixed else datetime.now(timezone.utc)


def now_iso():
    return now().strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def format_duration(seconds):
    seconds = max(0, int(seconds))
    if seconds < 60:
        return f"{seconds}s"
    if seconds < 3600:
        return f"{seconds // 60}m"
    return f"{seconds // 3600}h {seconds % 3600 // 60}m"


def fsync_dir(directory):
    """Make a rename in `directory` durable. Not every platform can open a directory; that's fine to skip."""
    try:
        fd = os.open(directory, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(fd)
    except OSError:
        pass
    finally:
        os.close(fd)


def write_text_atomic(p, text):
    """Replace a file so readers only ever see the complete old or the complete new content."""
    tmp = f"{p}.{os.getpid()}.{os.urandom(4).hex()}.tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, p)
    except BaseException:
        with contextlib.suppress(OSError):
            os.remove(tmp)
        raise
    fsync_dir(os.path.dirname(os.path.abspath(p)))


def write_json_atomic(p, value):
    write_text_atomic(p, to_json(value) + "\n")


def read_json(p):
    """The file's JSON, or None when it can't be read or parsed. Deeply nested JSON (a hostile file) raises
    RecursionError in the parser; that is unreadable too, not a crash (docs/security-review.md F12, F18)."""
    try:
        with open(p, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError, RecursionError):
        return None


def is_zombie(pid):
    """A process that has exited but wasn't reaped yet (common in containers without an init process). Linux only;
    elsewhere an orphaned process is reaped by the system right away."""
    try:
        with open(f"/proc/{pid}/stat", encoding="utf-8") as f:
            stat = f.read()
    except OSError:
        return False
    fields = stat[stat.rfind(")") + 1 :].split()
    return bool(fields) and fields[0] == "Z"


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return not is_zombie(pid)


WHOAMI = "/__atm/whoami"


def whoami(port):
    """Ask the atm server on this port who it is; None if nothing (or not an atm server) answers."""
    import urllib.request

    try:
        with urllib.request.urlopen(f"http://localhost:{port}{WHOAMI}", timeout=1) as response:
            info = json.loads(response.read().decode("utf-8"))
    except (OSError, ValueError):
        return None
    return info if isinstance(info, dict) and info.get("atm") else None


# ==== common/model.py ====
# The rules of task data: the project's vocabularies, looking tasks up, and the status transitions themselves.
# Reading and writing the files is the store's (db/jsongit/files.py).
#
# Every rule is written against a status's kind, never its name (docs/v4/data-model.md §3): a project may name its own
# statuses in meta.statuses, each with one of the six kinds below.

# The kinds and the default vocabularies are common's (common/schema/vocabularies.json), shared with the dashboard
WORKFLOW_TYPES = ("direct", "pull-request")

# command: (kinds a task may be in, kind it moves to)
STATUS_CHANGES = {
    # From review only once the review asked for rework or approved it (SR-3.9)
    "start": (("todo", "review"), "active"),
    "review": (("active",), "review"),
    "done": (("active", "review"), "done"),
    "fail": (("active",), "todo"),
    "release": (("active",), "todo"),
    "reject": (("review",), "todo"),
    # The review's outcome; the task stays in review until an agent takes it back (SR-6.1)
    "approve": (("review",), "review"),
    "request-changes": (("review",), "review"),
    "block": (("todo", "active"), "blocked"),
    "unblock": (("blocked",), "todo"),
    # Undo a done (or skipped) by mistake, or work that turns out unfinished; the reason is kept as a note
    "reopen": (("done", "skipped"), "todo"),
    # A task that turns out not to be needed; reopen undoes it
    "skip": (("todo", "blocked"), "skipped"),
}
WAITING_ON = ("user", "agent")
# Review states that make a task in review work for an agent again (SR-3.9)
REWORK_STATES = ("changes_requested", "approved")
# How a finished task ended (SR-2.8): stored only when it isn't the default, `done` for a done task and `wont_do` for
# a skipped one
RESOLUTIONS = ("done", "wont_do", "duplicate", "obsolete")
SKIP_RESOLUTIONS = ("wont_do", "obsolete", "duplicate")


def _terms(meta, key, defaults, need_kind=False):
    """meta[key] as a list of {id, label[, kind]}, or the defaults when absent. Raises AtmError naming the problem."""
    if key not in meta:
        return defaults
    value = meta[key]
    if not isinstance(value, list) or not value:
        raise AtmError(f"meta.{key} must be a non-empty list")
    seen, terms = set(), []
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str) or not item["id"]:
            raise AtmError(f"meta.{key}: every entry needs an \"id\" (a non-empty string)")
        if item["id"] in seen:
            raise AtmError(f"meta.{key}: \"{item['id']}\" is listed twice")
        seen.add(item["id"])
        term = {"id": item["id"], "label": item.get("label") or item["id"].replace("_", " ").capitalize()}
        if need_kind:
            if item.get("kind") not in KINDS:
                raise AtmError(
                    f"meta.{key}: \"{item['id']}\" has the kind {item.get('kind')!r}; use one of {', '.join(KINDS)}"
                )
            term["kind"] = item["kind"]
        terms.append(term)
    return terms


WORKFLOW_DEFAULTS = {"verify": True, "max_rounds": 3, "commit_template": "feat({id}): {name}"}

# Strings from tasks.json that agents put into shell commands (a base branch, task IDs) are kept to characters a
# shell doesn't treat specially (docs/security-review.md F5): no `;`, `$`, quotes, spaces, or a leading `-`
BRANCH_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,199}")
TASK_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")


def valid_branch(value):
    return isinstance(value, str) and bool(BRANCH_NAME.fullmatch(value)) and ".." not in value


def valid_task_id(value):
    return isinstance(value, str) and bool(TASK_ID.fullmatch(value))


def split_id(task_id):
    """(prefix, digits) of an ID that ends in a number ("T-001" -> ("T-", "001")), else None. A plain scan from the
    end, not a regex: `^(.*?)(\\d+)$` backtracks quadratically on a long run of digits without an end (a hostile ID,
    docs/security-review.md F12)."""
    if not isinstance(task_id, str):
        return None
    end = len(task_id)
    while end and task_id[end - 1].isdigit():
        end -= 1
    return (task_id[:end], task_id[end:]) if end < len(task_id) else None


def check_workflow(workflow):
    """meta.workflow (docs/v4/skills-and-workflows.md S-1.1), when set: raises AtmError naming what's wrong."""
    if workflow is None:
        return
    if not isinstance(workflow, dict) or workflow.get("type") not in WORKFLOW_TYPES:
        raise AtmError(f"meta.workflow.type must be one of {', '.join(WORKFLOW_TYPES)}")
    checks = (
        ("base", valid_branch, "a branch name (letters, digits, `.`, `_`, `/`, `-`; not starting with `-`)"),
        ("verify", lambda v: isinstance(v, bool), "true or false"),
        ("max_rounds", lambda v: isinstance(v, int) and not isinstance(v, bool) and v > 0, "a positive whole number"),
        ("commit_template", lambda v: isinstance(v, str) and v, "a message template like \"feat({id}): {name}\""),
        (
            "trailers",
            lambda v: v == "forbid"
            or (isinstance(v, dict) and isinstance(v.get("require"), list) and all(isinstance(x, str) for x in v["require"])),
            '"forbid" or {"require": ["Co-Authored-By: ..."]}',
        ),
    )
    for key, valid, expected in checks:
        if key in workflow and not valid(workflow[key]):
            raise AtmError(f"meta.workflow.{key} must be {expected}")


SPRINT_STATES = ("planned", "active", "closed")
SPRINT_NAMING = ("people", "time")
REVIEW_MODES = ("blocker", "async", "batch", "none")
MAX_SPRINT_NAME = 48
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")


def check_sprints(meta):
    """meta.sprints (docs/sprints-and-releases-design.md SR-2.1, SR-2.3) as a list of sprints, in order; [] when
    absent. A sprint's ID is its name: unique (compared case-insensitively), at most 48 characters, no space at either
    end. A sprint without a state is planned. Raises AtmError naming the problem."""
    sprints = meta.get("sprints", [])
    if not isinstance(sprints, list):
        raise AtmError("meta.sprints must be a list of sprints")
    seen, active = set(), []
    for sprint in sprints:
        if not isinstance(sprint, dict) or not isinstance(sprint.get("id"), str) or not sprint["id"].strip():
            raise AtmError('meta.sprints: every sprint needs an "id", its name (a non-empty string)')
        name = sprint["id"]
        if name != name.strip() or len(name) > MAX_SPRINT_NAME:
            raise AtmError(f"meta.sprints: {name!r} is not a sprint name (at most {MAX_SPRINT_NAME} characters, no "
                           "space at either end)")
        if name.casefold() in seen:
            raise AtmError(f"meta.sprints: {name!r} is listed twice")
        seen.add(name.casefold())
        state = sprint.get("state", "planned")
        if state not in SPRINT_STATES:
            raise AtmError(f"meta.sprints: {name!r} has the state {state!r}; use one of {', '.join(SPRINT_STATES)}")
        if state == "active":
            active.append(name)
        if "due" in sprint and not (isinstance(sprint["due"], str) and DATE.fullmatch(sprint["due"])):
            raise AtmError(f"meta.sprints: {name!r} has the due date {sprint['due']!r}; use YYYY-MM-DD")
        for key in ("goal", "release", "start", "end"):
            if key in sprint and not isinstance(sprint[key], str):
                raise AtmError(f"meta.sprints: {name!r}'s {key} must be a string")
    if len(active) > 1:
        raise AtmError(f"meta.sprints: only one sprint can be active, but {' and '.join(active)} are")
    return [{**s, "state": s.get("state", "planned")} for s in sprints]


RELEASE_STATES = ("planned", "released", "archived")
# A release ID is any string of these (Q5): agents put it in shell commands, so no spaces or quotes
RELEASE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")


def check_releases(meta):
    """meta.releases (docs/sprints-and-releases-design.md SR-2.2) as a list, in order; [] when absent. A release
    without a state is planned. Raises AtmError naming the problem."""
    releases = meta.get("releases", [])
    if not isinstance(releases, list):
        raise AtmError("meta.releases must be a list of releases")
    seen = set()
    for release in releases:
        if not isinstance(release, dict) or not isinstance(release.get("id"), str) or not RELEASE_ID.fullmatch(
            release["id"]
        ):
            raise AtmError(
                'meta.releases: every release needs an "id" of letters, digits, ".", "-" and "_" (at most 64)'
            )
        if release["id"] in seen:
            raise AtmError(f"meta.releases: {release['id']!r} is listed twice")
        seen.add(release["id"])
        state = release.get("state", "planned")
        if state not in RELEASE_STATES:
            raise AtmError(
                f"meta.releases: {release['id']!r} has the state {state!r}; use one of {', '.join(RELEASE_STATES)}"
            )
        if "target" in release and not (isinstance(release["target"], str) and DATE.fullmatch(release["target"])):
            raise AtmError(f"meta.releases: {release['id']!r} has the target {release['target']!r}; use YYYY-MM-DD")
        for key in ("name", "released_at"):
            if key in release and not isinstance(release[key], str):
                raise AtmError(f"meta.releases: {release['id']!r}'s {key} must be a string")
    return [{**r, "state": r.get("state", "planned")} for r in releases]


def check_components(meta):
    """meta.components (docs/sprints-and-releases-design.md §2, SR-2.7) as a list, in order; [] when absent: each
    {id, label, paths (globs matched against a task's files), review}. Raises AtmError naming the problem."""
    components = meta.get("components", [])
    if not isinstance(components, list):
        raise AtmError("meta.components must be a list of components")
    seen = set()
    for component in components:
        if not isinstance(component, dict) or not isinstance(component.get("id"), str) or not RELEASE_ID.fullmatch(
            component["id"]
        ):
            raise AtmError(
                'meta.components: every component needs an "id" of letters, digits, ".", "-" and "_" (at most 64)'
            )
        name = component["id"]
        if name in seen:
            raise AtmError(f"meta.components: {name!r} is listed twice")
        seen.add(name)
        paths = component.get("paths", [])
        if not isinstance(paths, list) or not all(isinstance(p, str) and p for p in paths):
            raise AtmError(f"meta.components: {name!r}'s paths must be a list of globs (strings)")
        if "review" in component and component["review"] not in REVIEW_MODES:
            raise AtmError(f"meta.components: {name!r} has the review {component['review']!r}; use one of "
                           f"{', '.join(REVIEW_MODES)}")
        if "label" in component and not isinstance(component["label"], str):
            raise AtmError(f"meta.components: {name!r}'s label must be a string")
    return [{**c, "paths": c.get("paths", [])} for c in components]


def find_release(vocab, release_id):
    """The release with this ID, or None."""
    return next((r for r in vocab["releases"] if r["id"] == release_id), None)


def known_release(vocab, release_id):
    """`release_id`, when meta.releases has it; else an AtmError naming the releases."""
    if find_release(vocab, release_id) is None:
        known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm releases add <R>"
        raise AtmError(f"no release {release_id!r}; the releases: {known}", 2)
    return release_id


def check_setting(meta, key, choices, default):
    value = meta.get(key, default)
    if value not in choices:
        raise AtmError(f"meta.{key} must be one of {', '.join(choices)}")
    return value


def workflow_of(data):
    """The project's workflow with its defaults filled in, or None when it has none."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    workflow = meta.get("workflow")
    if workflow is None:
        return None
    check_workflow(workflow)
    return {**WORKFLOW_DEFAULTS, **workflow}


def vocabulary(data):
    """The project's vocabularies from meta, with the defaults for what it doesn't define (DM-2). Raises AtmError,
    naming the problem, when meta defines something that can't work."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    statuses = _terms(meta, "statuses", DEFAULT_STATUSES, need_kind=True)
    for kind in ("todo", "active", "done"):
        if not any(s["kind"] == kind for s in statuses):
            raise AtmError(f"meta.statuses needs at least one status of the kind {kind!r}")
    priorities = _terms(meta, "priorities", DEFAULT_PRIORITIES)
    ids = [p["id"] for p in priorities]
    # Without a default: P2 if the project has it, else the middle priority (as the dashboard does)
    default_priority = meta.get(
        "default_priority", DEFAULT_PRIORITY if DEFAULT_PRIORITY in ids else ids[(len(ids) - 1) // 2]
    )
    if default_priority not in ids:
        raise AtmError(f"meta.default_priority {default_priority!r} is not one of meta.priorities")
    # meta.phases, from before sprints, isn't read: atm migrate turns it into sprints (SR-8.1, SR-8.3)
    check_workflow(meta.get("workflow"))
    return {
        "statuses": statuses,
        "types": _terms(meta, "types", DEFAULT_TYPES),
        "priorities": priorities,
        "default_priority": default_priority,
        "sprints": check_sprints(meta),
        "releases": check_releases(meta),
        "sprint_names": check_setting(meta, "sprint_names", SPRINT_NAMING, "people"),
        "review": check_setting(meta, "review", REVIEW_MODES, "none"),
        "components": check_components(meta),
    }


def find_sprint(vocab, name):
    """The sprint with this name (compared case-insensitively), or None."""
    if not isinstance(name, str):
        return None
    for sprint in vocab["sprints"]:
        if sprint["id"].casefold() == name.casefold():
            return sprint
    return None


def set_sprint(task, sprint, stamp):
    """Put `task` in `sprint` (a sprint ID), or in the backlog (None). Every change ends the open sprint_history entry
    and, for a sprint, starts a new one (SR-2.5). Returns whether anything changed."""
    if task.get("sprint") == sprint:
        return False
    history = task.setdefault("sprint_history", [])
    for entry in history:
        if isinstance(entry, dict) and "to" not in entry:
            entry["to"] = stamp
    if sprint is None:
        task.pop("sprint", None)
    else:
        task["sprint"] = sprint
        history.append({"sprint": sprint, "from": stamp})
    if not history:
        task.pop("sprint_history")
    return True


def effective_release(vocab, task):
    """A task's release: its own, else its sprint's (SR-2.6), or None."""
    if task.get("release"):
        return task["release"]
    sprint = find_sprint(vocab, task.get("sprint"))
    return sprint.get("release") if sprint else None


def effective_review(vocab, task):
    """A task's review mode: its own, else its first component's that has one, else the project's (meta.review),
    else none (SR-2.6)."""
    if task.get("review") in REVIEW_MODES:
        return task["review"]
    components = {c.get("id"): c for c in vocab["components"] if isinstance(c, dict)}
    for component in task.get("components") or []:
        mode = components.get(component, {}).get("review")
        if mode in REVIEW_MODES:
            return mode
    return vocab["review"]


def kind_of(vocab, status):
    """The status's kind. A status the project doesn't list keeps its default kind if it is a default status; any
    other status has none (None)."""
    for s in vocab["statuses"]:
        if s["id"] == status:
            return s["kind"]
    for s in DEFAULT_STATUSES:
        if s["id"] == status:
            return s["kind"]
    return None


def open_in_closed_sprints(vocab, tasks):
    """Unfinished tasks whose sprint is closed (T-330): reopened there, or left by a merge. The loop's default selection
    (the active sprint) never takes them. [{id, name, status, sprint}]"""
    found = []
    for task in tasks:
        sprint = find_sprint(vocab, task.get("sprint")) if isinstance(task, dict) else None
        if sprint and sprint["state"] == "closed" and kind_of(vocab, task.get("status")) not in ("done", "skipped"):
            found.append({"id": task.get("id"), "name": task.get("name", ""), "status": task.get("status"), "sprint": sprint["id"]})
    return found


def move_out_hint(task_id, sprint):
    """The end of a sentence on a task open in a closed sprint (T-330), with what to do: a suggestion, atm moves
    nothing on its own."""
    return f"the closed sprint {sprint!r}, where the loop won't take it: atm move {task_id} --to backlog (or --to active)"


def priority_rank(vocab, priority):
    """0 for the most urgent priority. A missing or unknown priority ranks as the default priority."""
    ids = [p["id"] for p in vocab["priorities"]]
    return ids.index(priority) if priority in ids else ids.index(vocab["default_priority"])


def statuses_of(vocab, kind):
    return [s["id"] for s in vocab["statuses"] if s["kind"] == kind]


def status_for(vocab, kind, chosen=None):
    """The status a task moving to `kind` gets: `chosen` (--status), which must be of that kind, else the first status
    of that kind the project lists."""
    options = statuses_of(vocab, kind)
    if chosen is None:
        return options[0]
    if chosen not in options:
        known = kind_of(vocab, chosen)
        what = f"its kind is {known}" if known else "the project has no such status"
        raise AtmError(f"--status {chosen} is not one of the {kind} statuses ({what}); use {' or '.join(options)}", 2)
    return chosen


# Where the rules look up an archived task (a dependency not in tasks.json, show of an archived task): the store
# that loaded the task data sets it (use_archive), read only when needed; one atm process works on one project
_archive = {"lookup": None}


def use_archive(lookup):
    """Look archived tasks up with `lookup(task_id)` (task or None) from now on; None: there is no archive."""
    _archive["lookup"] = lookup


def archived_task(task_id):
    """The archived task with this ID, if the archive has it."""
    return _archive["lookup"](task_id) if _archive["lookup"] else None


ARCHIVE_DEFAULTS = {"threshold_kb": 1024, "threshold_tasks": 500, "keep_days": 14, "auto": True}


def archive_settings(data):
    """meta.archive, with the defaults for what it doesn't set (DM-2)."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    given = meta.get("archive") if isinstance(meta.get("archive"), dict) else {}
    settings = dict(ARCHIVE_DEFAULTS)
    for key, default in ARCHIVE_DEFAULTS.items():
        value = given.get(key)
        if isinstance(default, bool):
            if isinstance(value, bool):
                settings[key] = value
        elif isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0:
            settings[key] = value
    return settings


def finished_at(task):
    """When a finished task finished: completed_at (done) or skipped_at (skipped), else updated_at for files from
    before those were written (atm migrate fills them in)."""
    for key in ("completed_at", "skipped_at", "updated_at"):
        try:
            return parse_iso(task[key])
        except (KeyError, TypeError, ValueError):
            continue
    return None


def archivable(data, before=None, keep=None):
    """The tasks `atm archive` would move (DM-5.1): done- and skipped-kind, not finished in the last `keep` days
    (default meta.archive.keep_days), not in an open PR; only those finished before `before`, if given."""
    vocab = vocabulary(data)
    recent = now() - timedelta(days=archive_settings(data)["keep_days"] if keep is None else keep)
    result = []
    for t in data["tasks"]:
        if not isinstance(t, dict) or kind_of(vocab, t.get("status")) not in ("done", "skipped"):
            continue
        if isinstance(t.get("pr"), dict) and not t.get("merged_at"):
            continue  # still in an open PR
        finished = finished_at(t)
        if finished is not None and finished > recent:
            continue
        if before is not None and (finished is None or finished >= before):
            continue
        result.append(t)
    return result


def format_size(size):
    return f"{size / 1048576:.1f} MB" if size >= 1048576 else f"{size // 1024} KB"


def archive_hint_for(data, size):
    """The suggestion to archive (CLI-5.1), or None: when the task data (`size` bytes) is over
    meta.archive.threshold_kb, or has more archivable tasks than threshold_tasks, and there is something to move."""
    settings = archive_settings(data)
    movable = len(archivable(data))
    if not movable or (size <= settings["threshold_kb"] * 1024 and movable <= settings["threshold_tasks"]):
        return None
    vocab = vocabulary(data)
    finished = sum(
        1 for t in data["tasks"] if isinstance(t, dict) and kind_of(vocab, t.get("status")) in ("done", "skipped")
    )
    return {
        "bytes": size,
        "finished": finished,
        "archivable": movable,
        "auto": settings["auto"],  # false: the skills only relay the hint (S-3.5)
        "text": f"tasks.json is {format_size(size)} with {finished} finished tasks; `atm archive` would move "
        f"{movable} of them to tasks-archive.json.",
    }


META_KEY = re.compile(r"meta\.(\w+)")


def tolerant_vocabulary(meta):
    """The vocabularies of `meta` as far as they work, and what doesn't: each list or setting `vocabulary` refuses is
    left out (its default used instead), as the dashboard reads them. Returns (vocabulary, problems, the meta without
    what was left out)."""
    usable = dict(meta) if isinstance(meta, dict) else {}
    problems = []
    while True:
        try:
            return vocabulary({"meta": usable}), problems, usable
        except AtmError as e:
            problems.append(str(e))
            key = META_KEY.match(str(e))
            if key and key.group(1) in usable:
                usable.pop(key.group(1))
                if key.group(1) == "priorities":
                    usable.pop("default_priority", None)  # it names one of them
            else:
                usable = {}  # nothing to narrow it to: the defaults


def find_task(data, task_id, archived=False):
    """The task with this ID in tasks.json; with `archived`, also in the archive (for reading only)."""
    for task in data["tasks"]:
        if task.get("id") == task_id:
            return task
    if archived and archived_task(task_id):
        return archived_task(task_id)
    hint = " (it is archived: atm unarchive brings it back)" if not archived and archived_task(task_id) else ""
    raise AtmError(f"no task {task_id}{hint}")


def unfinished_dependencies(data, task):
    """Dependencies that don't have a done-kind status yet, with their status ("missing" for unknown IDs). A
    dependency that is only in the archive is done (DM-3.1)."""
    vocab = vocabulary(data)
    status = {t.get("id"): t.get("status") for t in data["tasks"]}
    deps = task.get("dependencies", [])
    # A list of IDs, as load_tasks checks; a file read tolerantly (load_tasks_tolerant) may hold anything there
    deps = [d for d in deps if isinstance(d, str)] if isinstance(deps, list) else []
    return [
        (dep, status.get(dep, "missing"))
        for dep in deps
        if (dep in status and kind_of(vocab, status[dep]) != "done") or (dep not in status and not archived_task(dep))
    ]


def natural_key(task_id):
    """Sort T-2 before T-10, like the dashboard's localeCompare(..., { numeric: true })."""
    return [int(part) if part.isdigit() else part.lower() for part in re.split(r"(\d+)", str(task_id))]


MAX_NOTE = 500


def check_note(text):
    """Notes stay short; anything longer belongs in a file in the repository."""
    if len(text) > MAX_NOTE:
        raise AtmError(
            f"a note is at most {MAX_NOTE} characters (this one has {len(text)}); "
            "put longer text in a file in the repository and mention it in the note",
            2,
        )
    return text


def add_note(task, kind, text, by, stamp, checked=True):
    """Append a note (DM-4): handoff, outcome, review, rejection, failure, answer or note. `checked=False` for a note
    built from parts already checked one by one (an unblock's questions and answers)."""
    note = {"at": stamp, "kind": kind, "text": check_note(text) if checked else text}
    if by:
        note["by"] = by
    task.setdefault("notes", []).append(note)


def touch(task, actor, stamp):
    """Every change records when and by whom (CLI-3.5)."""
    task["updated_at"] = stamp
    if actor:
        task["updated_by"] = actor


def back_to_todo(task, vocab, actor, stamp, keep_commit=False):
    """The task returns to the first todo-kind status; its attempt's timestamps and actors go (DM-4.4). The commit
    goes too, unless `keep_commit`: an expired lease or a release leaves the work in git, to be finished later."""
    task["status"] = status_for(vocab, "todo")
    touch(task, actor, stamp)
    for field in (
        "started_at",
        "completed_at",
        "skipped_at",
        "started_by",
        "completed_by",
        "agent",
        "review_requested_at",
        "waiting_on",
        "questions",
        "blocked_at",
        "sprint_review",
        "resolution",
        "duplicate_of",
    ):
        task.pop(field, None)
    if not keep_commit:
        task.pop("commit", None)


def add_interval(task, field, start, end, on=None, outcome=None):
    """Append {from, to[, on][, outcome]} to the task's list `field`: `blocks` (with who it waited on) or `reviews`
    (with what the review decided, when it sent the task back to an agent)."""
    interval = {"from": start, "to": end}
    if on:
        interval["on"] = on
    if outcome:
        interval["outcome"] = outcome
    task.setdefault(field, []).append(interval)


def answer_note(questions, answers):
    """One note for an unblocked task: each question with its answer."""
    pairs = []
    for i in range(max(len(questions), len(answers))):
        q = questions[i] if i < len(questions) else None
        a = answers[i] if i < len(answers) else None
        pairs.append(f"{q} → {a}" if q and a else (f"{q} → (no answer)" if q else a))
    return "; ".join(pairs)


def change_status(
    task,
    data,
    command,
    reason=None,
    agent=None,
    status=None,
    note=None,
    commit=None,
    waiting_on=None,
    questions=(),
    answers=(),
    resolution=None,
    duplicate_of=None,
):
    """Apply one status command to `task` (in `data`); raises AtmError if the transition isn't allowed. `status`
    picks the new status when its kind has several; `agent` is who runs it. `start` records them as started_by,
    `done` as completed_by (both kept when it is done, so the dashboard can show who did what); `note` and `reason`
    become notes, `commit` the task's commit. `block` records who it waits on and the open questions; `unblock`
    turns them, with the answers, into an answer note. `done` and `skip` record a `resolution` (SR-2.8), `skip` also
    `duplicate_of` (the caller checks that task exists, in either file)."""
    vocab = vocabulary(data)
    if command in ("done", "skip"):
        resolution = resolution or ("done" if command == "done" else "wont_do")
        allowed_resolutions = RESOLUTIONS if command == "done" else SKIP_RESOLUTIONS
        if resolution not in allowed_resolutions:
            raise AtmError(f"--resolution must be one of {', '.join(allowed_resolutions)}", 2)
        if duplicate_of is not None and resolution != "duplicate":
            raise AtmError("--duplicate-of goes with --resolution duplicate", 2)
        if duplicate_of == task["id"]:
            raise AtmError(f"{task['id']} can't be a duplicate of itself", 2)
    allowed, new_kind = STATUS_CHANGES[command]
    current = task.get("status")
    if kind_of(vocab, current) not in allowed:
        expected = [s for kind in allowed for s in statuses_of(vocab, kind)]
        raise AtmError(f"can't {command} {task['id']}: it is {current}, not {' or '.join(expected)}")
    if command == "done":
        check_review_mode(task, data, agent)
    if command == "start" and kind_of(vocab, current) == "review" and task.get("review_state") not in REWORK_STATES:
        raise AtmError(f"can't start {task['id']}: it waits for its review (atm approve or request-changes decides it)")
    new = status_for(vocab, new_kind, status)
    if command in ("approve", "request-changes") and status is None:
        new = current  # it stays in its review status (a project may have several)
    if command == "start":
        blocking = unfinished_dependencies(data, task)
        if blocking:
            raise AtmError(f"can't start {task['id']}: waiting on " + ", ".join(f"{d} ({s})" for d, s in blocking))
    if note is not None:
        check_note(note)
    if reason is not None:
        check_note(reason)
    for question in questions:
        check_note(question)
    if command == "block":
        if waiting_on not in WAITING_ON:
            raise AtmError(f"--on must be {' or '.join(WAITING_ON)}", 2)
        if not questions and not reason:
            raise AtmError("say what it waits for: --question (repeatable) or --reason", 2)
    for answer in answers:
        check_note(answer)
    asked = list(task.get("questions") or [])
    # Each question was checked when the task was blocked, each answer just now: together they may be longer than one
    # note, and must still be recordable, or the task could never be unblocked
    answered = answer_note(asked, list(answers)) if command == "unblock" and (asked or answers) else None
    stamp = now_iso()
    # A block's or a review's start goes when the task moves on: keep each interval, so waiting time can be measured
    old_kind = kind_of(vocab, current)
    if old_kind == "blocked" and task.get("blocked_at"):
        add_interval(task, "blocks", task["blocked_at"], stamp, on=task.get("waiting_on"))
    elif old_kind == "review" and new_kind != "review" and task.get("review_requested_at"):
        # The round ends when the reviewer decided, with its outcome (SR-3.9)
        outcome = task.get("review_state") if task.get("review_state") in REWORK_STATES else None
        add_interval(task, "reviews", task["review_requested_at"], task.get("reviewed_at") or stamp, outcome=outcome)
    if new_kind != "review":
        task.pop("review_state", None)
        task.pop("reviewed_at", None)
    if new_kind == "todo":
        # Reopening, rejecting or failing a task undoes its work; releasing or unblocking it doesn't
        back_to_todo(task, vocab, agent, stamp, keep_commit=command not in ("reopen", "reject", "fail"))
        if answered:
            add_note(task, "answer", answered, agent, stamp, checked=False)
        if command == "fail":
            add_note(task, "failure", reason or "verification failed", agent, stamp)
        elif command == "reject":
            add_note(task, "rejection", reason, agent, stamp)
        elif command == "reopen":
            add_note(task, "note", f"Reopened: {reason}", agent, stamp)
        elif note:
            add_note(task, "handoff", note, agent, stamp)
        return current
    task["status"] = new
    touch(task, agent, stamp)
    if command == "block":
        # Who started it stays visible while it waits; unblock returns it to to-do and clears the attempt
        task["waiting_on"] = waiting_on
        task["questions"] = list(questions)
        task["blocked_at"] = stamp
        if reason:
            add_note(task, "note", reason, agent, stamp)
    elif command == "review":
        # The work ends here: the review isn't agent work (DM-4.1); done keeps this timestamp
        task["review_requested_at"] = stamp
        task["review_state"] = "waiting"
        if note:
            add_note(task, "review", note, agent, stamp)
    elif command in ("approve", "request-changes"):
        # The task stays in review until an agent takes it back (`atm next` offers it first, SR-3.9)
        task["review_state"] = "approved" if command == "approve" else "changes_requested"
        task["reviewed_at"] = stamp
        if command == "request-changes":
            add_note(task, "rejection", reason, agent, stamp)
        else:
            add_note(task, "note", f"Approved: {note}" if note else "Approved", agent, stamp)
    elif command == "skip":
        for field in ("waiting_on", "questions", "blocked_at"):
            task.pop(field, None)
        task["skipped_at"] = stamp
        if resolution != "wont_do":
            task["resolution"] = resolution  # none means the default, won't do
        if duplicate_of:
            task["duplicate_of"] = duplicate_of
        add_note(task, "note", f"Skipped: {reason}", agent, stamp)
    elif command == "start":
        task["started_at"] = stamp
        task.pop("review_requested_at", None)  # taken back from review: a new round of work
        task.pop("completed_at", None)
        task.pop("completed_by", None)
        task.pop("agent", None)  # the old name of started_by
        if agent:
            task["started_by"] = agent
    elif command == "done":
        task["completed_at"] = stamp
        if resolution != "done":
            task["resolution"] = resolution  # none means the default, done
        if agent:
            task["completed_by"] = agent
        if commit:
            task["commit"] = commit
        # A batch task an agent finishes waits for the end-of-sprint review (SR-4.4): its note is what to check
        batch = old_kind == "active" and is_agent(agent) and effective_review(vocab, task) == "batch"
        if batch:
            task["sprint_review"] = "pending"
        if note:
            add_note(task, "review" if batch else "outcome", note, agent, stamp)
    return current


def is_agent(actor):
    """Whether an actor is an agent (a Claude Code session, "claude:…"); anyone else is a person."""
    return isinstance(actor, str) and actor.startswith("claude:")


def check_review_mode(task, data, agent):
    """`atm done` by review mode (SR-3.8): an agent finishing an active task whose mode is `blocker` or `async` must
    ask for the review instead. A person may always finish a task."""
    vocab = vocabulary(data)
    mode = effective_review(vocab, task)
    if mode in ("blocker", "async") and is_agent(agent) and kind_of(vocab, task.get("status")) == "active":
        raise AtmError(
            f"{task['id']} is reviewed ({mode}): ask for it with atm review {task['id']} --note '<what to check>'", 2
        )


# ==== common/fields.py ====
# A task's fields as every layer reads them: the effective priority, who started it.

def priority_of(vocab, task):
    """The task's priority, or the default for a missing or unknown one."""
    ids = [p["id"] for p in vocab["priorities"]]
    return task.get("priority") if task.get("priority") in ids else vocab["default_priority"]


def started_by(task):
    """started_by, or `agent` in files written before v4."""
    return task.get("started_by") or task.get("agent")


# ==== common/types.py ====
# Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
# `npm run build:cli` in dashboard/ (the pre-commit hook does it too). TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has
# no typing.Required).

StatusEntry = TypedDict(
    "StatusEntry",
    {
        # required
        "id": str,
        "label": str,
        # required; What the status means
        "kind": Literal['todo', 'active', 'review', 'blocked', 'done', 'skipped'],
    },
    total=False,
)

VocabularyTerm = TypedDict(
    "VocabularyTerm",
    {
        # required
        "id": str,
        "label": str,
    },
    total=False,
)

LabelEntry = TypedDict(
    "LabelEntry",
    {
        # required
        "id": str,
        "label": str,
        "description": str,
    },
    total=False,
)

PhaseEntry = TypedDict(
    "PhaseEntry",
    {
        # required
        "id": float,
        "name": str,
    },
    total=False,
)

# How the project pushes (docs/v4/skills-and-workflows.md §1)
Workflow = TypedDict(
    "Workflow",
    {
        # direct or pull-request
        "type": str,
        "base": str,
        "verify": bool,
        "max_rounds": int,
        "commit_template": str,
        # "forbid" or "none", or {"require": [the lines]}: what commit messages end with
        "trailers": object,
    },
    total=False,
)

# When atm suggests archiving, and whether agents may do it (DM-5)
ArchiveSettings = TypedDict(
    "ArchiveSettings",
    {
        "threshold_kb": float,
        "threshold_tasks": float,
        "keep_days": float,
        "auto": bool,
    },
    total=False,
)

Sprint = TypedDict(
    "Sprint",
    {
        # required; The sprint's name
        "id": str,
        "goal": str,
        "state": Literal['planned', 'active', 'closed'],
        "release": str,
        # ISO 8601 UTC
        "start": str,
        # ISO 8601 UTC
        "end": str,
        # YYYY-MM-DD
        "due": str,
        "report": "SprintReport",
    },
    total=False,
)

# What a closed sprint did (SR-2.3)
SprintReport = TypedDict(
    "SprintReport",
    {
        # The tasks done in it
        "done": List[str],
        # The unfinished tasks carried on
        "carried": List[str],
        # Where the carried tasks went
        "to": str,
    },
    total=False,
)

Release = TypedDict(
    "Release",
    {
        # required
        "id": str,
        "name": str,
        "state": Literal['planned', 'released', 'archived'],
        # YYYY-MM-DD
        "target": str,
        # ISO 8601 UTC
        "released_at": str,
    },
    total=False,
)

Component = TypedDict(
    "Component",
    {
        # required
        "id": str,
        "label": str,
        # Globs over its files
        "paths": List[str],
        # Its tasks' default review mode
        "review": str,
    },
    total=False,
)

# A project's meta in tasks.json: its settings, vocabularies, sprints, releases and components (docs/v4/data-model.md §2). Every key is optional and falls back to its default.
Meta = TypedDict(
    "Meta",
    {
        # The project's ID (8 hex characters, set by atm init)
        "id": str,
        # The project's name
        "project": str,
        # One line about the project
        "description": str,
        "created": str,
        # The repository's web address, for commit and PR links
        "repository": str,
        # github, gitlab, bitbucket or gitea, for a host whose name doesn't say
        "repository_type": str,
        # The project's logo, relative to its root
        "logo": str,
        # How long a claim lasts without a heartbeat
        "lease_minutes": float,
        "statuses": List["StatusEntry"],
        "types": List["VocabularyTerm"],
        # Most urgent first
        "priorities": List["VocabularyTerm"],
        # The priority of tasks without one
        "default_priority": str,
        "labels": List["LabelEntry"],
        # Before sprints (atm migrate turns them into sprints)
        "phases": List["PhaseEntry"],
        "workflow": "Workflow",
        "archive": "ArchiveSettings",
        # How many tasks tasks-archive.json holds (DM-5.4)
        "archived": int,
        # The number the next new task gets
        "next_id": int,
        # In order
        "sprints": List["Sprint"],
        # In order
        "releases": List["Release"],
        "components": List["Component"],
        # How a sprint created without a name is named: people or time
        "sprint_names": str,
        # The project's default review mode: blocker, async, batch or none
        "review": str,
    },
    total=False,
)

Note = TypedDict(
    "Note",
    {
        # required; ISO 8601 UTC
        "at": str,
        # Who wrote it
        "by": str,
        # required; handoff, outcome, review, rejection, failure, answer or note
        "kind": str,
        # required; Markdown, at most 500 characters
        "text": str,
    },
    total=False,
)

# The PR that contains the task (atm pr): the task is In PR until merged_at (DM-4.3)
PullRequest = TypedDict(
    "PullRequest",
    {
        # required
        "number": int,
        "url": str,
    },
    total=False,
)

BlockRound = TypedDict(
    "BlockRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        # user or agent
        "on": str,
    },
    total=False,
)

ReviewRound = TypedDict(
    "ReviewRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        "outcome": str,
    },
    total=False,
)

SprintHistoryEntry = TypedDict(
    "SprintHistoryEntry",
    {
        # required
        "sprint": str,
        # required; ISO 8601 UTC
        "from": str,
        # ISO 8601 UTC; absent while it is in the sprint
        "to": str,
    },
    total=False,
)

# Before v4: the last failed attempt (now a failure note; still read)
LastFailure = TypedDict(
    "LastFailure",
    {
        "at": str,
        "reason": str,
    },
    total=False,
)

# One task, as tasks.json and tasks-archive.json keep it (docs/v4/data-model.md §4). Unknown fields are kept as they are.
Task = TypedDict(
    "Task",
    {
        # required; Letters, digits, '.', '_' and '-' (T-001)
        "id": str,
        # required
        "name": str,
        # required; A type ID from the vocabulary (meta.types)
        "type": str,
        # required; A status ID from the vocabulary (meta.statuses); atm acts on its kind
        "status": str,
        # Markdown
        "description": str,
        # Before sprints: the task's phase (atm migrate turns phases into sprints)
        "phase": float,
        # A priority ID from the vocabulary ("P0"…"P3" by default); missing means the default priority
        "priority": str,
        # Free-form tags that group tasks across sprints and types, e.g. ["spike-2", "ux"] (DM-4.6)
        "labels": List[str],
        # Task IDs it waits for
        "dependencies": List[str],
        "requirements": List[str],
        "use_cases": List[str],
        # Files it creates or changes
        "files": List[str],
        # ISO 8601 UTC
        "created_at": str,
        # ISO 8601 UTC
        "updated_at": str,
        # ISO 8601 UTC; set when the task moves to in progress
        "started_at": str,
        # ISO 8601 UTC; set when the task is done
        "completed_at": str,
        # ISO 8601 UTC; set when the task is skipped (atm skip)
        "skipped_at": str,
        # Before v4: the agent that started the task. Now started_by (readers fall back to it)
        "agent": str,
        # Who created it (docs/v4/data-model.md §4.1)
        "created_by": str,
        # Who changed it last
        "updated_by": str,
        # Who started it: the claim's owner
        "started_by": str,
        # Who finished it
        "completed_by": str,
        # The SHA of the task's commit
        "commit": str,
        "pr": "PullRequest",
        # ISO 8601 UTC; when that PR was merged (atm merged)
        "merged_at": str,
        # ISO 8601 UTC; when the task asked for a review (atm review). Its work ended here; review time isn't work
        "review_requested_at": str,
        # A blocked task waits on a decision by the user or an agent (atm block --on)
        "waiting_on": str,
        # The open questions of a blocked task
        "questions": List[str],
        # ISO 8601 UTC; when it was blocked
        "blocked_at": str,
        # Past blocks, oldest first, with who each waited on (T-244)
        "blocks": List["BlockRound"],
        # Past reviews, oldest first
        "reviews": List["ReviewRound"],
        # Handoffs, outcomes, review requests, answers, rejections (docs/v4/data-model.md §4)
        "notes": List["Note"],
        "last_failure": "LastFailure",
        # The sprint it is in (a sprint's name, meta.sprints); absent: the backlog
        "sprint": str,
        # Each sprint it was in, from when to when; the open entry has no `to` (SR-2.5)
        "sprint_history": List["SprintHistoryEntry"],
        # The release it ships in; when absent, its sprint's (SR-2.6)
        "release": str,
        # Component IDs (meta.components)
        "components": List[str],
        # How it is reviewed: blocker, async, batch or none
        "review": str,
        # In the review kind: waiting, changes_requested or approved (SR-6.1)
        "review_state": str,
        # ISO 8601 UTC; when the review was decided
        "reviewed_at": str,
        # A batch task done by an agent: pending until the end-of-sprint review, then reviewed (SR-4.4)
        "sprint_review": str,
        # `user`: the work is done together with the user (SR-6.5)
        "with": str,
        # Related tasks (a link that doesn't block), stored on one side and shown on both (SR-2.8)
        "related": List[str],
        # How a finished task ended, when it isn't the default: wont_do, duplicate, obsolete or done (SR-2.8)
        "resolution": str,
        # The task this one duplicates (with the duplicate resolution)
        "duplicate_of": str,
    },
    total=False,
)

# atm's suggestion to archive (CLI-5.1), present when the working tasks are over meta.archive's thresholds (T-372)
ArchiveHint = TypedDict(
    "ArchiveHint",
    {
        # required; How big the working task data is
        "bytes": int,
        # required; How many working tasks are finished
        "finished": int,
        # required; How many of them `atm archive` would move
        "archivable": int,
        # required; Whether agents may archive on their own (meta.archive.auto)
        "auto": bool,
        # required; The hint as atm says it
        "text": str,
    },
    total=False,
)

# A task as stored, and what atm derives for it
TaskView = TypedDict(
    "TaskView",
    {
        # required
        "task": "Task",
        # required
        "derived": "TaskDerived",
    },
    total=False,
)

# What atm-lib works out for a task, so no front end applies atm's rules itself
TaskDerived = TypedDict(
    "TaskDerived",
    {
        # required; The status's kind: todo, active, review, blocked, done or skipped (absent for an unknown status)
        "kind": str,
        # required; To do, and every dependency done (an archived one counts as done)
        "ready": bool,
        # required; A to-do task's dependencies that aren't done yet
        "waiting_for": List[str],
        # Who a blocked task waits on: user or agent
        "waiting_on": str,
        # required; Its priority, or the project's default
        "priority": str,
        # Its review mode: its own, else its component's, else the project's
        "effective_review": str,
        # Its release: its own, else its sprint's
        "effective_release": str,
        # A done task's stage: done, in_pr or merged (DM-4.3)
        "stage": str,
        # required; From the archive
        "archived": bool,
        # What atm finds wrong with the task (T-377); it isn't ready then
        "problems": List[str],
    },
    total=False,
)

DependencyItem = TypedDict(
    "DependencyItem",
    {
        # required
        "id": str,
        # required; Its status (", archived" added for an archived task), or missing
        "status": str,
        # required
        "name": str,
    },
    total=False,
)

# atm deps: what a task waits for, and what waits for it
DepsResult = TypedDict(
    "DepsResult",
    {
        # required
        "id": str,
        # required
        "waits_for": List["DependencyItem"],
        # required
        "waited_on_by": List["DependencyItem"],
    },
    total=False,
)

# GET /api/v1/projects/<p>/tasks: the tasks the query's filters keep, each with its derived values (docs/layered-architecture.md §6.2)
TaskViewList = TypedDict(
    "TaskViewList",
    {
        # required; ISO 8601 UTC
        "generated_at": str,
        # required
        "meta": "Meta",
        # required; How big the project's working task data is (tasks.json), for the archive notice
        "bytes": int,
        # required
        "tasks": List["TaskView"],
        # What atm finds wrong with the task file as a whole: entries left out, vocabularies that fell back to their defaults (T-377)
        "problems": List[str],
        "archive_hint": "ArchiveHint",
    },
    total=False,
)


# ==== common/components.py ====
# The rules of components (docs/sprints-and-releases-design.md §2, SR-2.7): the parts of a project (meta.components,
# each with globs over its files), the components a task's files suggest, and the IDs a task may name. Listing,
# defining and changing them is atm-lib's (lib/components.py).

def matches(component, path):
    """Whether `path` (a task's file, relative to the project) is in the component: one of its globs matches. `*`
    and `**` both match across folders (`dashboard/**` is everything under dashboard/)."""
    return any(fnmatchcase(path, glob) for glob in component.get("paths") or [])


def suggest_components(vocab, files):
    """The components the files are in, in the project's order."""
    return [c["id"] for c in vocab["components"] if any(matches(c, f) for f in files if isinstance(f, str))]


def component_ids(vocab, values):
    """The component IDs a task may have: in a project with components, only theirs; otherwise any ID of letters,
    digits, '.', '-' and '_' (agents put them in shell commands). Raises AtmError naming the first bad one."""
    known = [c["id"] for c in vocab["components"]]
    for value in values:
        if not isinstance(value, str) or not RELEASE_ID.fullmatch(value):
            raise AtmError(f"component {value!r}: use letters, digits, '.', '-' and '_' (at most 64)", 2)
        if known and value not in known:
            raise AtmError(f"no component {value!r}; the components: {', '.join(known)}", 2)
    return list(values)


# ==== common/sprint_names.py ====
# Names for sprints created without one (docs/sprints-and-releases-design.md SR-2.9), by meta.sprint_names: a
# historical person's name not yet used in the project (`people`), or the time it was created (`time`).
#
# The names are plain ASCII letters, spaces, hyphens and dots, without apostrophes: agents pass sprint names in shell
# commands, inside single quotes.

PEOPLE = (
    # Scientists and mathematicians
    "Ada Lovelace", "Alan Turing", "Albert Einstein", "Alfred Wegener", "Alexander Fleming", "Alexander von Humboldt",
    "Alhazen", "Andre-Marie Ampere", "Antoine Lavoisier", "Archimedes", "Barbara McClintock",
    "Benjamin Banneker", "Blaise Pascal", "Carl Friedrich Gauss", "Carl Linnaeus", "Caroline Herschel",
    "Charles Babbage", "Charles Darwin", "Chien-Shiung Wu", "Claude Shannon", "Dmitri Mendeleev", "Dorothy Hodgkin",
    "Edmond Halley", "Edwin Hubble", "Emmy Noether", "Enrico Fermi", "Ernest Rutherford", "Euclid", "Evariste Galois",
    "Florence Nightingale", "Galileo Galilei", "George Washington Carver", "Gertrude Elion", "Grace Hopper",
    "Gregor Mendel", "Hedy Lamarr", "Henrietta Leavitt", "Hypatia", "Ibn Sina", "Isaac Newton", "James Clerk Maxwell",
    "Jagadish Chandra Bose", "Johannes Kepler", "John von Neumann", "Jonas Salk", "Katherine Johnson",
    "Leonhard Euler", "Lise Meitner", "Louis Pasteur", "Mae Jemison", "Marie Curie", "Mary Anning",
    "Mary Somerville", "Max Planck", "Michael Faraday", "Muhammad al-Khwarizmi", "Niels Bohr", "Nicolaus Copernicus",
    "Nikola Tesla", "Omar Khayyam", "Pierre Curie", "Rachel Carson", "Richard Feynman", "Robert Hooke",
    "Rosalind Franklin", "Santiago Ramon y Cajal", "Sophie Germain", "Srinivasa Ramanujan", "Stephen Hawking",
    "Subrahmanyan Chandrasekhar", "Tu Youyou", "Tycho Brahe", "Vera Rubin", "Werner Heisenberg", "Zhang Heng",
    "Alessandro Volta", "Anders Celsius", "Christiaan Huygens", "Daniel Bernoulli", "Erwin Schrodinger",
    "Fibonacci", "Gottfried Leibniz", "Henri Poincare", "Hermann von Helmholtz", "Jean-Baptiste Lamarck",
    "Joseph Fourier", "Kurt Godel", "Lord Kelvin", "Maria Goeppert Mayer", "Paul Dirac", "Pierre-Simon Laplace",
    "Ronald Ross", "Seki Takakazu", "Wilhelm Rontgen", "William Harvey", "Annie Jump Cannon", "Cecilia Payne",
    "Edward Jenner", "Hans Christian Orsted", "Joseph Lister", "Lev Landau", "Linus Pauling",
    "Ludwig Boltzmann", "Robert Boyle", "Satyendra Nath Bose", "Wangari Maathai",
    # Engineers and inventors
    "Alexander Graham Bell", "Charles Kettering", "Edith Clarke", "Frank Whittle", "Granville Woods", "Gustave Eiffel",
    "Isambard Brunel", "James Watt", "Johannes Gutenberg", "Karl Benz", "Lillian Gilbreth", "Margaret Hamilton",
    "Nikolaus Otto", "Orville Wright", "Rudolf Diesel", "Samuel Morse", "Thomas Edison", "Wilbur Wright",
    "Yuri Gagarin", "Valentina Tereshkova", "Sally Ride", "Robert Goddard", "Konstantin Tsiolkovsky",
    # Explorers and travellers
    "Amelia Earhart", "Abel Tasman", "Bartolomeu Dias", "Ernest Shackleton", "Ferdinand Magellan", "Fridtjof Nansen",
    "Gertrude Bell", "Ibn Battuta", "Isabella Bird", "James Cook", "Leif Erikson", "Matthew Henson", "Marco Polo",
    "Mary Kingsley", "Roald Amundsen", "Sacagawea", "Tenzing Norgay", "Edmund Hillary", "Zheng He", "Xuanzang",
    "Vasco da Gama", "Robert Falcon Scott", "Nellie Bly", "Freya Stark", "Jacques Cousteau", "Thor Heyerdahl",
    # Writers and poets
    "Agatha Christie", "Anton Chekhov", "Charles Dickens", "Charlotte Bronte", "Emily Bronte", "Emily Dickinson",
    "Franz Kafka", "Fyodor Dostoevsky", "Gabriel Garcia Marquez", "George Eliot", "Hans Christian Andersen",
    "Homer", "Italo Calvino", "Jane Austen", "Jorge Luis Borges", "Jules Verne", "Leo Tolstoy", "Mark Twain",
    "Mary Shelley", "Miguel de Cervantes", "Murasaki Shikibu", "Naguib Mahfouz", "Nazim Hikmet", "Oscar Wilde",
    "Pablo Neruda", "Rabindranath Tagore", "Rumi", "Sappho", "Toni Morrison", "Victor Hugo", "Virginia Woolf",
    "Walt Whitman", "William Shakespeare", "Yunus Emre", "Chinua Achebe", "Dante Alighieri", "Edgar Allan Poe",
    "Geoffrey Chaucer", "Hafez", "Herman Melville", "James Joyce", "Johann Wolfgang von Goethe", "Langston Hughes",
    "Li Bai", "Du Fu", "Matsuo Basho", "Maya Angelou", "Rainer Maria Rilke", "Ursula K. Le Guin", "Wislawa Szymborska",
    "Alexandre Dumas", "Arthur Conan Doyle", "Astrid Lindgren", "Bertolt Brecht", "Gustave Flaubert", "H. G. Wells",
    "Honore de Balzac", "Ivan Turgenev", "Lewis Carroll", "Machado de Assis", "Marcel Proust", "Nikolai Gogol",
    "Orhan Veli", "Robert Louis Stevenson", "Sei Shonagon", "Selma Lagerlof", "Sylvia Plath", "Zora Neale Hurston",
    # Philosophers and thinkers
    "Aristotle", "Baruch Spinoza", "Confucius", "Hannah Arendt", "Ibn Khaldun", "Immanuel Kant",
    "John Locke", "Laozi", "Mary Wollstonecraft", "Michel de Montaigne", "Plato", "Rene Descartes", "Simone de Beauvoir",
    "Socrates", "Thales", "Voltaire", "Zeno", "Adam Smith", "David Hume", "Epicurus", "Mencius", "Pythagoras",
    # Artists and architects
    "Artemisia Gentileschi", "Claude Monet", "Diego Velazquez", "Frida Kahlo", "Georgia OKeeffe", "Hokusai",
    "Johannes Vermeer", "Leonardo da Vinci", "Michelangelo", "Mimar Sinan", "Pablo Picasso", "Paul Cezanne",
    "Rembrandt", "Sandro Botticelli", "Vincent van Gogh", "Wassily Kandinsky", "Zaha Hadid", "Antoni Gaudi",
    "Filippo Brunelleschi", "Frank Lloyd Wright", "Gustav Klimt", "Henri Matisse", "Hilma af Klint", "Paul Klee",
    "Raphael", "Rosa Bonheur", "Titian", "Utagawa Hiroshige", "Berthe Morisot", "Edgar Degas", "Le Corbusier",
    # Composers and musicians
    "Antonio Vivaldi", "Clara Schumann", "Claude Debussy", "Duke Ellington", "Fanny Mendelssohn", "Frederic Chopin",
    "George Gershwin", "Hildegard of Bingen", "Igor Stravinsky", "Johann Sebastian Bach", "Ludwig van Beethoven",
    "Maurice Ravel", "Nina Simone", "Pyotr Tchaikovsky", "Scott Joplin", "Wolfgang Amadeus Mozart", "Ella Fitzgerald",
    "Louis Armstrong", "Bela Bartok", "Edvard Grieg", "Franz Schubert", "Giuseppe Verdi", "Jean Sibelius",
    "Joseph Haydn", "Sergei Rachmaninoff", "Antonin Dvorak", "Billie Holiday", "Miles Davis", "Umm Kulthum",
)

MONTHS = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")


def time_name(when):
    """`May 12-10:38pm`: when the sprint was created, in local time."""
    hour = when.hour % 12 or 12
    return f"{MONTHS[when.month - 1]} {when.day}-{hour}:{when.minute:02d}{'am' if when.hour < 12 else 'pm'}"


def generate_sprint_name(naming, taken):
    """A new sprint name by `naming` (meta.sprint_names), not one of `taken` (compared case-insensitively). The time
    is atm's clock in the local time zone (ATM_NOW and TZ in tests); ATM_SEED fixes the random pick, for tests."""
    used = {name.casefold() for name in taken}
    if naming == "time":
        base = time_name(now().astimezone())
        name, n = base, 1
        while name.casefold() in used:
            n += 1
            name = f"{base}-{n}"
        return name
    free, round_ = [p for p in PEOPLE if p.casefold() not in used], 1
    while not free:
        # Every name taken (a project with hundreds of sprints): the same names again, numbered
        round_ += 1
        free = [f"{p} {round_}" for p in PEOPLE if f"{p} {round_}".casefold() not in used]
    seed = os.environ.get("ATM_SEED")
    return (random.Random(seed) if seed else random.Random()).choice(free)


# ==== db/interface.py ====
# The store: how atm-lib reads and writes a project's data, whatever keeps it (docs/layered-architecture.md §3).
#
# It grows with the migration, one slice at a time (§9): each slice adds the operations it needs. Tasks are the
# dicts of the task file's format (docs/v4/data-model.md §4); a store may hand out its own objects, so atm-lib never
# changes what a read returns.

class Store(Protocol):
    def get_vocabularies(self) -> dict:
        """The project's statuses, types, priorities and the rest of its vocabularies, with the defaults filled in."""
        ...

    def list_tasks(self) -> List[dict]:
        """The working tasks (not the archived ones), in the project's order."""
        ...

    def get_task(self, task_id: str) -> Optional[dict]:
        """A working task, or None."""
        ...

    def get_archived_task(self, task_id: str) -> Optional[dict]:
        """An archived task, or None."""
        ...

    def list_archived(self) -> List[dict]:
        """The archived tasks."""
        ...

    def read_claims(self) -> dict:
        """The claims and what goes with them: `claims` (by task), `completed`, `waiting` and `resolved` (tasks done,
        waiting or decided in another working copy) and `hints` (when each hint was last shown)."""
        ...

    def sizes(self) -> dict:
        """How big the project's data is, in bytes: `tasks` (the working tasks) and `archive`."""
        ...

    def files_for_commit(self) -> List[str]:
        """The files of the store that travel with a code commit (relative to the repository): tasks.json for the
        JSON-in-git store, none for a store that syncs elsewhere (docs/layered-architecture.md §2)."""
        ...

    def problems(self) -> dict:
        """What a tolerant read found wrong with the data (T-377): {"file": [...], "tasks": {id: [...]}}, in `atm
        check`'s words; empty for a strict read, which refuses such data instead."""
        ...

    def working_copy(self) -> Optional[str]:
        """The working copy this store reads, for a store that has several (git worktrees); None otherwise."""
        ...

    def raw(self) -> dict:
        """The data as stored, unchecked, for `atm check` to report on: {"document" (None when it isn't task data),
        "archive" (the archived tasks), "archive_error" (why the archive can't be read, or None)}."""
        ...

    def change(self, fn: Callable[[Any], Any]) -> Any:
        """Run `fn(work)` (a db.working.Working: the tasks and claims to change) under the store's isolation, and write
        what it changed; nothing when it raises. Returns what `fn` returns, then raises what it deferred. The
        transitional form of `apply` (docs/layered-architecture.md §3.3): `work.changes()` is the list of writes."""
        ...

    def document(self) -> dict:
        """The whole task document (`meta` and `tasks`), for the code from before the layers that still takes it.
        Transitional: it goes when the last of that code has moved (docs/layered-architecture.md §9, step 14)."""
        ...


# ==== vcs/interface.py ====
# The code's version control, as atm-lib uses it (docs/layered-architecture.md §2). Git's other role, carrying the
# task files, belongs to the JSON-in-git store, not here. vcs/git.py implements it with git.

class Vcs(Protocol):
    def in_repository(self) -> bool:
        """Whether the project's folder is under version control at all."""
        ...

    def paths(self, files: List[str], cwd: str) -> List[str]:
        """The files to commit, relative to the repository, from names relative to `cwd`; refuses a name that would
        commit more than one file (the whole tree, a directory, a pattern) or a file outside the repository."""
        ...

    def commit(self, files: List[str], message: str) -> str:
        """Commit exactly these files; the new commit's SHA."""
        ...

    def resolve(self, rev: str) -> Optional[str]:
        """A revision's SHA, or None if there is no such commit."""
        ...

    def in_history(self, sha: str) -> bool:
        """Whether the commit is reachable from the current one."""
        ...

    def message(self, sha: str) -> str:
        """A commit's message."""
        ...

    def default_base(self) -> Optional[str]:
        """The remote's default branch."""
        ...

    def remote_url(self) -> Optional[str]:
        """The remote's address."""
        ...

    def branch(self) -> Optional[str]:
        """The checked-out branch."""
        ...


# ==== db/jsongit/gitrun.py ====
# Git as the carrier of the task files (docs/layered-architecture.md §2): finding the repository and its common
# directory, staging the task folder's files, the merge driver's config. Git as the code's version control is vcs/git.py.

def git(args, cwd):
    """Run git; return stdout stripped, or None if git fails or isn't installed."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def is_tracked(root, p):
    return git(["ls-files", "--error-unmatch", "--", rel(root, p)], root) is not None


def in_git(root):
    return git(["rev-parse", "--git-dir"], root) is not None


# ==== db/jsongit/files.py ====
# The JSON-in-git store's files (docs/v4/data-model.md §1): reading tasks.json strictly (every command) or tolerantly
# (showing a malformed file, T-377), and the archive, tasks-archive.json, which the rules look archived tasks up in.

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
    """The rules' lookup of archived tasks in the archive of `paths`, read on the first lookup."""
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
            raise AtmError(f"{paths['tasks']}: task #{i + 1} isn't a task (an object with an \"id\"); run atm check")
        if not valid_task_id(task["id"]):
            raise AtmError(
                f"{paths['tasks']}: the task ID {task['id']!r} has characters atm doesn't allow (letters, digits, '.', "
                "'_' and '-' only): fix it in the file, then run atm check"
            )
        deps = task.get("dependencies", [])
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            raise AtmError(f"{paths['tasks']}: {task['id']}'s dependencies aren't a list of IDs; run atm check")
    use_archive(archive_of(paths))
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
    use_archive(archive_of(paths))
    return data, problems


def archive_hint(paths, data):
    """The suggestion to archive (CLI-5.1) for the task file of `paths`, or None (common/model.py archive_hint_for)."""
    return archive_hint_for(data, os.path.getsize(paths["tasks"]) if os.path.isfile(paths["tasks"]) else 0)


# ==== paths.py ====
# Where a project's task files are: the project root, the task folder, legacy locations.

LEGACY_DIRS = ["docs", "."]
# The files atm reads and writes in the task folder; a repository can commit any of them as a symlink
FOLDER_FILES = (TASKS_FILE, "tasks-archive.json", ".gitignore", ".gitattributes")


def check_task_folder(root, task_dir):
    """Refuse a task folder atm would read or write outside the project through (docs/security-review.md F7): a
    cloned repository can commit `.agentic-task-manager` or its files as symlinks to anything the user can write,
    such as ~/.zshrc."""
    if os.path.islink(task_dir):
        raise AtmError(f"{task_dir} is a symlink; atm works only on a real folder inside the project")
    if not os.path.isdir(task_dir):
        return
    real_root = os.path.realpath(root)
    if os.path.commonpath([os.path.realpath(task_dir), real_root]) != real_root:
        raise AtmError(f"{task_dir} leads outside the project ({os.path.realpath(task_dir)})")
    for name in FOLDER_FILES:
        if os.path.islink(os.path.join(task_dir, name)):
            raise AtmError(f"{os.path.join(task_dir, name)} is a symlink; atm reads and writes only regular files there")


def project_root(cwd):
    """The git top level; outside git, the nearest directory with a task folder or docs/tasks.json, else cwd."""
    top = git(["rev-parse", "--show-toplevel"], cwd)
    if top:
        return top
    d = cwd
    while True:
        if os.path.isdir(os.path.join(d, TASK_DIR)) or os.path.isfile(os.path.join(d, "docs", TASKS_FILE)):
            return d
        parent = os.path.dirname(d)
        if parent == d:
            return cwd
        d = parent


def find_paths(cwd):
    root = project_root(cwd)
    task_dir = os.path.join(root, TASK_DIR)
    check_task_folder(root, task_dir)
    legacy = None
    for d in LEGACY_DIRS:
        candidate = os.path.normpath(os.path.join(root, d, TASKS_FILE))
        if os.path.isfile(candidate):
            dashboard = os.path.join(os.path.dirname(candidate), DASHBOARD_FILE)
            legacy = {"tasks": candidate, "dashboard": dashboard if os.path.isfile(dashboard) else None}
            break
    tasks = os.path.join(task_dir, TASKS_FILE)
    return {
        "root": root,
        "dir": task_dir,
        "tasks": tasks,
        "tasks_exists": os.path.isfile(tasks),
        "dashboard": os.path.join(task_dir, DASHBOARD_FILE),
        "legacy": legacy,
    }


def cmd_paths(args):
    paths = find_paths(os.getcwd())
    if args.json:
        out(to_json(paths))
        return
    for key in ["root", "dir", "tasks", "dashboard"]:
        out(f"{key}: {paths[key]}")
    if paths["legacy"]:
        out(f"legacy: {paths['legacy']['tasks']}")


# ==== lock.py ====
# The lock directory.
#
# A lock is a directory: mkdir succeeds for exactly one process, on every OS and file system. The holder writes
# owner.json inside it and removes the directory when done. A crashed holder leaves the directory behind, so a waiter
# breaks the lock when its owner is provably gone (same host, pid not running) or it is older than LOCK_STALE. See
# docs/parallel-agents-design.md, section 4.

LOCK_DIR = ".lock"
LOCK_OWNER = "owner.json"
LOCK_TIMEOUT = 10.0  # seconds a command waits for the lock (ATM_LOCK_TIMEOUT overrides)
LOCK_STALE = 60.0  # a lock older than this is abandoned: commands hold it for milliseconds
LOCK_POLL = 0.05


def lock_timeout():
    try:
        return float(os.environ.get("ATM_LOCK_TIMEOUT", LOCK_TIMEOUT))
    except ValueError:
        return LOCK_TIMEOUT


def lock_owner(lock):
    return read_json(os.path.join(lock, LOCK_OWNER))


def lock_age(lock, owner):
    """Seconds since the lock was taken: from owner.json, or the directory itself if owner.json isn't written yet."""
    try:
        since = parse_iso(owner["acquired_at"]).timestamp() if owner else os.stat(lock).st_mtime
    except (OSError, KeyError, TypeError, ValueError):
        return 0.0
    return time.time() - since


def lock_is_stale(lock, owner):
    if owner and owner.get("host") == socket.gethostname() and isinstance(owner.get("pid"), int):
        if not pid_alive(owner["pid"]):
            return True
    return lock_age(lock, owner) > LOCK_STALE


def break_lock(lock, owner):
    """Move a stale lock aside. Returns True if this process broke it.

    Rename first, then check the owner inside the renamed directory (docs/security-review.md F13): checking first and
    renaming after leaves a gap in which another waiter breaks the same stale lock and takes a fresh one, and this
    process would then move the fresh lock aside, leaving two holders. If what was moved isn't the stale lock judged
    here, it goes back."""
    aside = f"{lock}.stale.{os.getpid()}.{os.urandom(4).hex()}"
    try:
        os.rename(lock, aside)
    except OSError:
        return False
    if lock_owner(aside) != owner:
        try:
            os.rename(aside, lock)  # someone else's live lock: put it back
        except OSError:
            pass  # a newer lock is in place already; the moved one was broken by another waiter too
        return False
    shutil.rmtree(aside, ignore_errors=True)
    return True


def describe_owner(owner):
    if not owner:
        return "another process"
    return f"pid {owner.get('pid')} on {owner.get('host')} since {owner.get('acquired_at')}"


@contextlib.contextmanager
def locked(directory, timeout=None):
    """Hold the lock for `directory` (creating it if needed) for the duration of the block. `timeout` (seconds) replaces
    the usual wait; 0 tries once."""
    os.makedirs(directory, exist_ok=True)
    lock = os.path.join(directory, LOCK_DIR)
    deadline = time.time() + (lock_timeout() if timeout is None else timeout)
    while True:
        try:
            os.mkdir(lock)
            break
        except FileExistsError:
            owner = lock_owner(lock)
            if lock_is_stale(lock, owner) and break_lock(lock, owner):
                continue
            if time.time() >= deadline:
                raise AtmError(f"timed out waiting for the lock {lock} (held by {describe_owner(owner)})")
            time.sleep(LOCK_POLL)
    # SIGTERM would otherwise end the process without running the cleanup below
    previous = signal.signal(signal.SIGTERM, lambda signum, frame: sys.exit(128 + signum))
    try:
        write_json_atomic(
            os.path.join(lock, LOCK_OWNER), {"host": socket.gethostname(), "pid": os.getpid(), "acquired_at": now_iso()}
        )
        yield
    finally:
        shutil.rmtree(lock, ignore_errors=True)
        signal.signal(signal.SIGTERM, previous)


def cmd_lock_test(args):
    """Test hook: hold the task folder's lock for `seconds`, logging to .lock-test.log, so tests can check exclusion."""
    seconds = args.seconds
    task_dir = find_paths(os.getcwd())["dir"]
    log = os.path.join(task_dir, ".lock-test.log")
    with locked(task_dir):
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"acquire {os.getpid()}\n")
        time.sleep(seconds)
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"release {os.getpid()}\n")


# ==== registry.py ====
# The user's project registry ($ATM_HOME/projects.json): the projects the dashboard server shows.
#
# One file per user, guarded by $ATM_HOME/.lock (not by any project's lock, and never taken while holding one), written
# atomically. A linked git worktree registers its repository's main worktree. Format: docs/multi-project-api.md, 5.1.

REGISTRY_FILE = "projects.json"
REGISTRY_VERSION = 1
SEEN_EVERY = 24 * 3600  # seconds between seen_at updates, so the registry isn't rewritten on every command


def atm_home():
    """The user's atm folder: ATM_HOME, else ~/.agentic-task-manager."""
    return os.path.abspath(os.path.expanduser(os.environ.get("ATM_HOME") or "~/.agentic-task-manager"))


def ensure_home(home):
    """Create the atm folder readable by its owner only: it lists the user's projects and the server's pid and port
    (docs/security-review.md F18). A folder that exists already keeps its permissions."""
    os.makedirs(home, mode=0o700, exist_ok=True)


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


def main_worktree(root):
    """The main worktree's folder for a project root (itself unless it is a linked git worktree)."""
    common = git(["rev-parse", "--git-common-dir"], root)
    if common:
        common = os.path.normpath(os.path.join(root, common))
        if os.path.basename(common) == ".git":
            return os.path.dirname(common)
    return root


def head_branch(git_dir):
    """The branch checked out per a git directory's HEAD file; a detached HEAD gives its short SHA, no file None."""
    try:
        with open(os.path.join(git_dir, "HEAD"), encoding="utf-8") as f:
            head = f.readline().strip()
    except OSError:
        return None
    if head.startswith("ref: "):
        ref = head[5:]
        return ref[len("refs/heads/") :] if ref.startswith("refs/heads/") else ref
    return head[:7] or None


def worktrees(root):
    """The git worktrees of the project whose main worktree is `root`, main first, as `git worktree list` has them:
    [{path, branch, main}]. Read from the git directory, without running git, so the server can ask on every poll.
    Linked worktrees whose folder is gone are left out; outside git (or a bare repository) the list is empty."""
    git_dir = os.path.join(root, ".git")
    if not os.path.isdir(git_dir):
        return []
    found = [{"path": root, "branch": head_branch(git_dir), "main": True}]
    linked = os.path.join(git_dir, "worktrees")
    try:
        names = sorted(os.listdir(linked))
    except OSError:
        names = []
    for name in names:
        admin = os.path.join(linked, name)
        try:
            with open(os.path.join(admin, "gitdir"), encoding="utf-8") as f:
                path = os.path.dirname(f.readline().strip())  # the worktree's .git file
        except OSError:
            continue
        if path and os.path.isdir(path):
            found.append({"path": path, "branch": head_branch(admin), "main": False})
    return found


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


def describe_entry(entry):
    return f"{entry['id']}  {entry['name']}  {entry['path']}"


def damaged_notice(damaged):
    return f"projects.json was unreadable: moved it to {damaged} and started an empty registry." if damaged else None


def projects_list(home, as_json):
    ensure_home(home)
    with locked(home):
        registry, damaged = read_registry(home)
        if damaged:
            write_registry(home, registry)
    entries = [{**e, "missing": is_missing(e)} for e in registry["projects"]]
    if as_json:
        out(to_json({"projects": entries, "damaged": damaged}))
        return
    if damaged:
        out(damaged_notice(damaged))
    if not entries:
        out("No projects registered. `atm projects add [PATH]` adds one; atm adds a project when it runs there.")
    for e in entries:
        out(describe_entry(e) + ("  (not found)" if e["missing"] else ""))


def projects_add(home, target, as_json):
    path, meta = resolve_project(target)
    if not listed(meta):
        raise AtmError(f"{path} opts out of the dashboard (meta.dashboard.listed is false)")
    ensure_home(home)
    with locked(home):
        registry, damaged = read_registry(home)
        result = update_entry(registry, path, meta)
        write_registry(home, registry)
    entry = find_entry(registry, path)
    if as_json:
        out(to_json({"result": result, "project": entry, "damaged": damaged}))
        return
    if damaged:
        out(damaged_notice(damaged))
    out(f"{'Added' if result == 'added' else 'Already registered'}: {describe_entry(entry)}")


def projects_remove(home, target, as_json):
    if not target:
        raise AtmError("remove needs a project ID or path (see `atm projects`)", 2)
    candidates = {target, os.path.abspath(os.path.expanduser(target))}
    if os.path.isdir(target):
        candidates.add(main_worktree(find_paths(os.path.abspath(target))["root"]))
    ensure_home(home)
    with locked(home):
        registry, damaged = read_registry(home)
        entry = next((e for e in registry["projects"] if e.get("id") in candidates or e["path"] in candidates), None)
        if entry:
            registry["projects"].remove(entry)
        if entry or damaged:
            write_registry(home, registry)
    if entry is None:
        raise AtmError(f"no registered project {target} (see `atm projects`)")
    if as_json:
        out(to_json({"removed": entry, "damaged": damaged}))
        return
    out(f"Removed: {describe_entry(entry)}")


def projects_prune(home, as_json):
    ensure_home(home)
    with locked(home):
        registry, damaged = read_registry(home)
        gone = [e for e in registry["projects"] if is_missing(e)]
        registry["projects"] = [e for e in registry["projects"] if e not in gone]
        if gone or damaged:
            write_registry(home, registry)
    if as_json:
        out(to_json({"removed": gone, "damaged": damaged}))
        return
    if damaged:
        out(damaged_notice(damaged))
    if not gone:
        out("Nothing to prune: every registered project is there.")
    for e in gone:
        out(f"Removed: {describe_entry(e)}")


def cmd_projects(args):
    home = atm_home()
    if args.action == "add":
        projects_add(home, args.target, args.json)
    elif args.action == "remove":
        projects_remove(home, args.target, args.json)
    elif args.action == "prune":
        projects_prune(home, args.json)
    else:
        if args.target:
            raise AtmError(f"unexpected argument '{args.target}'", 2)
        projects_list(home, args.json)


# ==== repository.py ====
# `atm repository`: the project's repository web address in meta.repository (e.g. https://github.com/o/r), which the
# dashboard uses to link commits and pull requests. Git remotes are turned into that address.

SCP_REMOTE = re.compile(r"^(?:[\w.-]+@)?([\w.-]+):(?!//)(.+)$")  # git@github.com:o/r.git
URL = re.compile(r"^(https?|ssh|git)://(?:[^@/]+@)?([^/:]+)(?::\d+)?(/.*)?$")


def repository_url(text):
    """The web address of a repository: an http(s) URL as given, or a git remote turned into one
    (git@github.com:o/r.git, ssh://git@github.com/o/r.git, https://github.com/o/r.git -> https://github.com/o/r).
    Credentials, a trailing .git and a trailing slash go. None if it isn't one."""
    text = (text or "").strip()
    m = URL.match(text)
    if m:
        scheme, host, path = m.group(1), m.group(2), (m.group(3) or "")
        web = "https" if scheme in ("ssh", "git") else scheme
    else:
        m = SCP_REMOTE.match(text)
        if not m or "/" in m.group(1):
            return None
        web, host, path = "https", m.group(1), "/" + m.group(2)
    path = re.sub(r"\.git/?$", "", path.rstrip("/")).rstrip("/")
    if not host or "." not in host and host != "localhost" or not path.strip("/"):
        return None
    return f"{web}://{host}{path}"


def origin_repository(root):
    """The web address of the clone's origin remote, or None."""
    return repository_url(git(["remote", "get-url", "origin"], root) or "")


def ensure_repository(paths):
    """Fill meta.repository from origin when tasks.json has none. Returns the address it set, or None."""
    if not os.path.isfile(paths["tasks"]):
        return None
    url = origin_repository(paths["root"])
    if not url:
        return None
    with locked(paths["dir"]):
        data = load_tasks(paths)
        meta = data.setdefault("meta", {})
        if meta.get("repository"):
            return None
        meta["repository"] = url
        write_json_atomic(paths["tasks"], data)
    return url


def move_pr_links(tasks, old, new):
    """Point the tasks' recorded PR links (pr.url) on the `old` address at the `new` one, after the repository moved.
    Only the link changes: no timestamps or actors, so the history stays as it was. Returns how many moved."""
    moved = 0
    for task in tasks:
        pr = task.get("pr") if isinstance(task, dict) else None
        link = pr.get("url") if isinstance(pr, dict) else None
        if isinstance(link, str) and (link == old or link.startswith(old + "/")):
            pr["url"] = new + link[len(old) :]
            moved += 1
    return moved


def cmd_repository(args):
    paths = find_paths(os.getcwd())
    if args.action is None:
        if args.url is not None or args.old is not None:
            raise AtmError(f"unexpected argument '{args.url or args.old}' (atm repository set URL)", 2)
        current = load_tasks(paths).get("meta", {}).get("repository")
        if args.json:
            out(to_json({"repository": current or None}))
        elif current:
            out(current)
        else:
            out("No repository address yet. Set it with `atm repository set <url>` (a git remote works too).")
        return
    url = repository_url(args.url)
    if not url:
        raise AtmError(f"{args.url!r} is not a repository address: give an http(s) URL or a git remote", 2)
    old = None
    if args.old is not None:
        old = repository_url(args.old)
        if not old:
            raise AtmError(f"--from {args.old!r} is not a repository address: give an http(s) URL or a git remote", 2)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        meta = data.setdefault("meta", {})
        old = old or repository_url(meta.get("repository") or "")
        meta["repository"] = url
        moved = 0
        if old and old != url:
            # The repository moved: its PR links too, in tasks.json and the archive (T-273)
            moved = move_pr_links(data.get("tasks") or [], old, url)
            archive = load_archive(paths) if os.path.isfile(archive_path(paths)) else None
            archived = move_pr_links(archive["tasks"], old, url) if archive else 0
            if archived:
                write_json_atomic(archive_path(paths), archive)
            moved += archived
        write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json({"repository": url, "moved_links": moved}))
    else:
        links = f" ({moved} PR link{'' if moved == 1 else 's'} moved from {old})" if moved else ""
        out(f"Repository address set: {url}{links}")


# ==== logo.py ====
# `atm logo`: the project's logo for the dashboard, in meta.logo (a path relative to the project root), or else one
# found in the usual places. atm serve serves it at /p/<id>/logo (docs/multi-project-api.md).

LOGO_TYPES = {
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".webp": "image/webp",
    ".ico": "image/x-icon",
}
MAX_LOGO_BYTES = 512 * 1024
# Where a logo usually is, first match wins: logo.svg/logo.png in the root, then .github/logo.*, public/favicon.* and
# assets/logo.*, each in the order of LOGO_TYPES
LOGO_CANDIDATES = ["logo.svg", "logo.png"] + [
    f"{base}{ext}" for base in (".github/logo", "public/favicon", "assets/logo") for ext in LOGO_TYPES
]


def logo_file(root, relative):
    """(absolute path, content type) of the image at `relative` in the project at `root`. AtmError (exit code 2) if
    it isn't a supported image type, is outside the project (symbolic links followed), is missing or too large."""
    if not isinstance(relative, str) or not relative.strip():
        raise AtmError("no logo path given", 2)
    content_type = LOGO_TYPES.get(os.path.splitext(relative)[1].lower())
    if not content_type:
        raise AtmError(f"{relative}: a logo is an SVG, PNG, WebP or ICO image", 2)
    real_root = os.path.realpath(root)
    full = os.path.realpath(os.path.join(real_root, relative))
    if os.path.commonpath([real_root, full]) != real_root:
        raise AtmError(f"{relative}: the logo must be inside the project folder", 2)
    if not os.path.isfile(full):
        raise AtmError(f"{relative}: no such file", 2)
    if os.path.getsize(full) > MAX_LOGO_BYTES:
        raise AtmError(f"{relative}: the logo is larger than {MAX_LOGO_BYTES // 1024} KB", 2)
    return full, content_type


def find_logo(root, meta):
    """(path relative to root, absolute path, content type, how it was found: "set" or "found") of the project's
    logo, or None. meta.logo wins when it is set (a logo it names that is unusable means no logo); otherwise the first
    usable LOGO_CANDIDATES file."""
    if meta.get("logo"):
        try:
            return (meta["logo"], *logo_file(root, meta["logo"]), "set")
        except AtmError:
            return None
    for candidate in LOGO_CANDIDATES:
        try:
            return (candidate, *logo_file(root, candidate), "found")
        except AtmError:
            continue
    return None


def cmd_logo(args):
    paths = find_paths(os.getcwd())
    root = paths["root"]
    if args.action is None:
        if args.path is not None:
            raise AtmError(f"unexpected argument '{args.path}' (atm logo set PATH)", 2)
        meta = load_tasks(paths).get("meta", {})
        logo = find_logo(root, meta)
        if args.json:
            found = {"logo": logo[0], "source": logo[3]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": meta.get("logo")}))
        elif logo:
            out(f"{logo[0]}" + (" (found; set one with `atm logo set <path>`)" if logo[3] == "found" else ""))
        elif meta.get("logo"):
            out(f"meta.logo is {meta['logo']}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if args.action == "unset":
        with locked(paths["dir"]):
            data = load_tasks(paths)
            removed = data.get("meta", {}).pop("logo", None)
            if removed is not None:
                write_json_atomic(paths["tasks"], data)
        out(to_json({"logo": None}) if args.json else "Logo unset." if removed else "No logo was set.")
        return
    if args.path is None:
        raise AtmError("atm logo set needs a path", 2)
    # The path is relative to where atm runs, like every path atm takes; it is kept relative to the project root
    relative = os.path.relpath(os.path.abspath(args.path), os.path.realpath(root)).replace(os.sep, "/")
    logo_file(root, relative)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        data.setdefault("meta", {})["logo"] = relative
        write_json_atomic(paths["tasks"], data)
    out(to_json({"logo": relative}) if args.json else f"Logo set: {relative}")


# ==== merge.py ====
# Merging tasks.json, and `atm init`.
#
# Registered as a git merge driver (`tasks.json merge=atm`), so branches that changed tasks.json merge task by task
# instead of line by line. Design: docs/parallel-agents-design.md, option E and layer L3.

# What `atm` means is not in this file: git config can't be committed, so each clone registers the driver in its own
# .git/config (merge.atm.driver), which the atm skills do on every run. A clone without it merges tasks.json normally.
GITATTRIBUTES_LINES = (
    "# 'atm' is defined in each clone's git config (merge.atm.driver), registered by the atm plugin on every run.\n"
    "# Without it, git merges tasks.json line by line as usual.\n"
    "tasks.json merge=atm\n"
)
# Fields that move with the status: they come from the side whose progress wins
PROGRESS_FIELDS = (
    "status",
    "updated_at",
    "started_at",
    "completed_at",
    "skipped_at",
    "last_failure",
    "agent",
    "started_by",
    "completed_by",
    "updated_by",
    "commit",
    "review_requested_at",
    "review_state",
    "reviewed_at",
    "sprint_review",
    "waiting_on",
    "questions",
    "blocked_at",
)
# How far along a status's kind is (DM-3.3); a status without a kind ranks like todo
KIND_RANK = {"todo": 0, "active": 1, "blocked": 1, "review": 1, "done": 2, "skipped": 2}
MISSING = object()


def merge_vocabulary(*versions):
    """The vocabulary to rank statuses by: this branch's, else the other's, else the defaults. A merge never fails
    over a vocabulary; the next atm command reports it."""
    for data in versions:
        try:
            return vocabulary(data)
        except AtmError:
            continue
    return vocabulary({})


def merge_value(base, ours, theirs):
    """Three-way merge of one value: (value, conflict). On conflict, ours is kept."""
    if ours == theirs:
        return ours, False
    if ours == base:
        return theirs, False
    if theirs == base:
        return ours, False
    return ours, True


def progress_winner(ours, theirs, vocab):
    """Both sides moved a task along: the one further along wins, then the newer update, then ours."""
    rank = lambda task: KIND_RANK.get(kind_of(vocab, task.get("status")), 0)
    rank_o, rank_t = rank(ours), rank(theirs)
    if rank_o != rank_t:
        return ours if rank_o > rank_t else theirs
    return theirs if str(theirs.get("updated_at", "")) > str(ours.get("updated_at", "")) else ours


def merge_notes(ours, theirs):
    """Notes only grow: both sides' notes, each once, oldest first."""
    merged = list(ours) + [n for n in theirs if n not in ours]
    return sorted(merged, key=lambda n: str(n.get("at", "")) if isinstance(n, dict) else "")


def merge_intervals(ours, theirs):
    """Past blocks and reviews only grow: both sides', one per start, oldest first."""
    starts = {i.get("from") for i in ours if isinstance(i, dict)}
    merged = list(ours) + [i for i in theirs if isinstance(i, dict) and i.get("from") not in starts]
    return sorted(merged, key=lambda i: str(i.get("from", "")) if isinstance(i, dict) else "")


def merge_history(ours, theirs):
    """A task's sprint_history only grows: one entry per start (`from`); an entry ended on either side (`to`) is kept
    ended, so a sprint change on one branch closes the entry the other branch still has open."""
    by_start = {}
    for entry in list(ours) + list(theirs):
        if not isinstance(entry, dict):
            continue
        known = by_start.get(entry.get("from"))
        if known is None or ("to" in entry and "to" not in known):
            by_start[entry.get("from")] = entry
    return sorted(by_start.values(), key=lambda e: str(e.get("from", "")))


# meta lists merged item by item, by ID, like tasks (docs/sprints-and-releases-design.md SR-2.4), with how far along
# each state is: changed on both sides, the further state wins
META_LISTS = {
    "sprints": {"planned": 0, "active": 1, "closed": 2},
    "releases": {"planned": 0, "released": 1, "archived": 2},
    "components": None,
}
# Sprints are named, and two names that differ only in case are the same sprint (SR-2.3)
item_key = lambda name, item: (
    item.get("id").casefold() if name == "sprints" and isinstance(item.get("id"), str) else item.get("id")
)


def merge_items(name, base, ours, theirs):
    """Merge three versions of a meta list of {id, ...} items. Returns (items, conflict descriptions)."""
    rank = META_LISTS[name]
    keyed = lambda items: {item_key(name, i): i for i in items if isinstance(i, dict)} if isinstance(items, list) else {}
    b, o, t = keyed(base), keyed(ours), keyed(theirs)
    items, conflicts = [], []
    for key, item in o.items():
        if key not in b:
            other = t.get(key)
            if other is not None and other != item:
                conflicts.append(f"meta.{name}: {item.get('id')!r} was added on both branches, differently")
            items.append(item)
        elif key not in t:
            if item != b[key]:
                conflicts.append(f"meta.{name}: {item.get('id')!r} changed on this branch, deleted on the other")
                items.append(item)
        else:
            merged, fields = merge_item(b[key], item, t[key], rank)
            items.append(merged)
            if fields:
                conflicts.append(f"meta.{name}: both branches changed {item.get('id')!r} ({', '.join(fields)})")
    for key, item in t.items():
        if key not in o and (key not in b or item != b[key]):
            if key in b:
                conflicts.append(f"meta.{name}: {item.get('id')!r} deleted on this branch, changed on the other")
            items.append(item)
    return items, conflicts


def merge_item(base, ours, theirs, rank):
    """One sprint, release or component changed on both sides: field by field; where both changed a field and both
    changed the state, the side further along wins it (a sprint closed on one branch stays closed with its report);
    otherwise ours, as a conflict. Returns (item, names of conflicting fields)."""
    winner = None
    if rank and ours.get("state") != base.get("state") and theirs.get("state") != base.get("state"):
        ours_rank, theirs_rank = rank.get(ours.get("state"), 0), rank.get(theirs.get("state"), 0)
        if ours_rank != theirs_rank:
            winner = ours if ours_rank > theirs_rank else theirs
    merged, conflicts = {}, []
    for key in list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]:
        value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
        if conflict and winner is not None:
            value, conflict = winner.get(key, MISSING), False
        if conflict:
            conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def merge_set(base, ours, theirs):
    """A list merged as a set (`related`, SR-2.8): what either side added stays, what either side removed goes, in
    ours' order, then theirs' additions. MISSING when nothing is left."""
    as_list = lambda v: v if isinstance(v, list) else []
    base, ours, theirs = as_list(base), as_list(ours), as_list(theirs)
    removed = (set(base) - set(ours)) | (set(base) - set(theirs))
    merged = [v for v in ours if v not in removed]
    merged += [v for v in theirs if v not in removed and v not in merged]
    return merged or MISSING


def merge_task(base, ours, theirs, vocab):
    """Merge one task changed on both sides. Returns (task, names of conflicting fields)."""
    changed = lambda side: any(side.get(f, MISSING) != base.get(f, MISSING) for f in PROGRESS_FIELDS)
    if not changed(theirs):
        progress = ours
    elif not changed(ours):
        progress = theirs
    else:
        progress = progress_winner(ours, theirs, vocab)
    keys = list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]
    merged, conflicts = {}, []
    for key in keys:
        if key in PROGRESS_FIELDS:
            value = progress.get(key, MISSING)
        elif key == "notes":
            value = merge_notes(ours.get(key, []), theirs.get(key, []))
        elif key in ("blocks", "reviews"):
            value = merge_intervals(ours.get(key, []), theirs.get(key, []))
        elif key == "sprint_history":
            value = merge_history(ours.get(key, []), theirs.get(key, []))
        elif key == "related":
            value = merge_set(base.get(key, []), ours.get(key, []), theirs.get(key, []))
        else:
            value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
            if conflict and key == "phase" and ("sprint" in ours or "sprint" in theirs):
                # One side has been migrated to sprints (SR-8.5): its sprint stands for the phase
                value, conflict = MISSING, False
            if conflict:
                conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def next_free_id(task_id, used):
    parts = split_id(task_id)
    if not parts:
        n = 2
        while f"{task_id}-{n}" in used:
            n += 1
        return f"{task_id}-{n}"
    prefix, digits = parts
    numbers = [int(x[len(prefix) :]) for x in used if x.startswith(prefix) and x[len(prefix) :].isdigit()]
    return prefix + str(max(numbers) + 1).zfill(len(digits))


def merge_task_files(base, ours, theirs):
    """Merge three versions of tasks.json. Returns (merged data, list of conflict descriptions)."""
    by_id = lambda data: {t.get("id"): t for t in data.get("tasks", [])}
    vocab = merge_vocabulary(ours, theirs)
    base_tasks, our_tasks, their_tasks = by_id(base), by_id(ours), by_id(theirs)
    used = set(base_tasks) | set(our_tasks) | set(their_tasks)
    conflicts, merged_tasks, renamed, theirs_new = [], [], {}, []

    for task in ours.get("tasks", []):
        task_id = task.get("id")
        b, t = base_tasks.get(task_id), their_tasks.get(task_id)
        if b is None:
            merged_tasks.append(task)  # added here (if added there too, theirs is renumbered below)
        elif t is None:
            if task != b:
                conflicts.append(f"{task_id}: changed on this branch, deleted on the other")
                merged_tasks.append(task)
        else:
            merged, fields = merge_task(b, task, t, vocab)
            merged_tasks.append(merged)
            if fields:
                conflicts.append(f"{task_id}: both branches changed {', '.join(fields)}")

    for task in theirs.get("tasks", []):
        task_id = task.get("id")
        if task_id in base_tasks:
            if task_id not in our_tasks and task != base_tasks[task_id]:
                conflicts.append(f"{task_id}: deleted on this branch, changed on the other")
                merged_tasks.append(task)
            continue
        if task_id in our_tasks:
            if our_tasks[task_id] == task:
                continue  # the same task added on both sides
            new_id = next_free_id(task_id, used)
            used.add(new_id)
            renamed[task_id] = new_id
            task = {**task, "id": new_id}
        theirs_new.append(task)
    for task in theirs_new:
        if renamed and task.get("dependencies"):
            task = {**task, "dependencies": [renamed.get(d, d) for d in task["dependencies"]]}
        merged_tasks.append(task)

    result = {}
    for key in list(ours) + [k for k in theirs if k not in ours]:
        if key == "tasks":
            result[key] = merged_tasks
        elif key == "meta" and isinstance(ours.get(key), dict) and isinstance(theirs.get(key), dict):
            b, o, t = base.get(key) or {}, ours[key], theirs[key]
            meta = {}
            for k in list(o) + [k for k in t if k not in o]:
                if k in META_LISTS:
                    value, item_conflicts = merge_items(k, b.get(k, []), o.get(k, []), t.get(k, []))
                    conflicts.extend(item_conflicts)
                else:
                    value, _ = merge_value(b.get(k, MISSING), o.get(k, MISSING), t.get(k, MISSING))
                if value is not MISSING:
                    meta[k] = value
            if "sprints" in meta:
                meta.pop("phases", None)  # a project migrated on either side has no phases left (SR-8.5)
            result[key] = meta
        else:
            result[key] = ours.get(key, theirs.get(key))
    return result, conflicts


def cmd_merge_driver(args):
    base_path, ours_path, theirs_path = args.base, args.ours, args.theirs
    versions = [read_json(p) for p in (base_path, ours_path, theirs_path)]
    if versions[0] is None and os.path.exists(base_path) and os.path.getsize(base_path) == 0:
        versions[0] = {"tasks": []}  # the file was added on both branches
    if not all(isinstance(v, dict) and isinstance(v.get("tasks", []), list) for v in versions):
        raise AtmError("tasks.json merge: one side is not valid JSON; resolve the conflict by hand")
    merged, conflicts = merge_task_files(*versions)
    write_json_atomic(ours_path, merged)
    if args.json:
        out(to_json({"conflicts": conflicts}))
    if conflicts:
        sys.stderr.write("atm: tasks.json has conflicting edits (this branch's version was kept):\n")
        for conflict in conflicts:
            sys.stderr.write(f"  {conflict}\n")
        raise AtmExit(1)


def launcher_command():
    launcher = os.path.join(SCRIPT_DIR, "atm").replace("'", "'\\''")
    return f"'{launcher}' merge-driver %O %A %B"


def ensure_merge_driver(paths):
    """In git: `tasks.json merge=atm` in the task folder's .gitattributes (staged when added), and the driver
    registered in the clone's config, pointing at this plugin's launcher. Returns True if .gitattributes changed."""
    root = paths["root"]
    if not in_git(root):
        return False
    for key, value in (("merge.atm.name", "atm: merge tasks.json task by task"), ("merge.atm.driver", launcher_command())):
        if git(["config", "--get", key], root) != value:
            git(["config", key, value], root)
    attributes = os.path.join(paths["dir"], ".gitattributes")
    try:
        with open(attributes, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    if "tasks.json merge=atm" in text.splitlines():
        return False
    os.makedirs(paths["dir"], exist_ok=True)
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if not text or text.endswith("\n") else "\n") + GITATTRIBUTES_LINES)
    git(["add", "--", rel(root, attributes)], root)
    return True


ARCHIVE_ATTRIBUTES_LINE = "tasks-archive.json merge=atm\n"


def ensure_archive_merge(paths):
    """In git: the archive merges task by task too. Added when a project first archives, so projects that never do
    see no change. Returns True if .gitattributes changed (staged)."""
    root = paths["root"]
    if not in_git(root):
        return False
    ensure_merge_driver(paths)  # the driver itself, and the tasks.json line
    attributes = os.path.join(paths["dir"], ".gitattributes")
    with open(attributes, encoding="utf-8") as f:
        text = f.read()
    if ARCHIVE_ATTRIBUTES_LINE.strip() in text.splitlines():
        return False
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if text.endswith("\n") else "\n") + ARCHIVE_ATTRIBUTES_LINE)
    git(["add", "--", rel(root, attributes)], root)
    return True


def ensure_project_id(paths):
    """Give tasks.json a meta.id if it has none: the dashboard keeps each project's saved view under it (projects
    served from the same port share browser storage). Returns the new id, or None if there was one already."""
    if not os.path.isfile(paths["tasks"]):
        return None
    with locked(paths["dir"]):
        data = load_tasks(paths)
        meta = data.get("meta")
        if not isinstance(meta, dict):
            meta = data["meta"] = {}
        if isinstance(meta.get("id"), str) and meta["id"]:
            return None
        meta["id"] = os.urandom(4).hex()
        write_json_atomic(paths["tasks"], data)
    return meta["id"]


def apply_meta(paths, meta_file):
    """`init --meta-file F`: merge F's keys into tasks.json's meta (project, description, phases, vocabularies),
    creating tasks.json if there is none, so /atm:create-tasks never writes the file itself. The vocabularies are
    checked before anything is written; meta.id is kept. Returns (created, keys)."""
    new = read_json(meta_file)
    if not isinstance(new, dict):
        raise AtmError(f"{meta_file} must hold a JSON object: the meta keys to set", 2)
    new.pop("id", None)
    with locked(paths["dir"]) if os.path.isdir(paths["dir"]) else contextlib.nullcontext():
        created = not os.path.isfile(paths["tasks"])
        data = {"meta": {"created": now().strftime("%Y-%m-%d")}, "tasks": []} if created else load_tasks(paths)
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        data["meta"].update(new)
        vocabulary(data)  # raises, naming the problem, before anything is written
        os.makedirs(paths["dir"], exist_ok=True)
        write_json_atomic(paths["tasks"], data)
    return created, sorted(new)


def cmd_init(args):
    paths = find_paths(os.getcwd())
    if args.meta_file:
        created, keys = apply_meta(paths, args.meta_file)
        paths = find_paths(os.getcwd())
        if not args.json:
            out(("Created tasks.json; meta: " if created else "Updated tasks.json meta: ") + (", ".join(keys) or "-"))
    project_id = ensure_project_id(paths)
    repository = ensure_repository(paths)
    register(paths)  # the dashboard lists every project atm runs in
    git_repo = in_git(paths["root"])
    changed = ensure_merge_driver(paths) if git_repo else False
    if args.json:
        out(
            to_json(
                {"project_id": project_id, "repository": repository, "git": git_repo, "gitattributes_added": changed}
            )
        )
        return
    if project_id:
        out(f"Gave this project an id ({project_id}) in tasks.json meta, for the dashboard's saved view.")
    if repository:
        out(f"Recorded the repository address ({repository}), for commit and PR links on the dashboard.")
    if not git_repo:
        out("Not in a git repository: there is no merge driver to set up.")
        return
    out(
        "Registered the tasks.json merge driver (git config merge.atm)"
        + (" and added .agentic-task-manager/.gitattributes (staged)." if changed else "; .gitattributes already set.")
    )


# ==== claims.py ====
# Claims: which agent works on which task, until when (the lease); and `next`.
#
# Claims live in the git common directory, so all worktrees of a clone see them; tasks.json itself is per worktree.
# Lock order: the task folder, then the claims store, so two commands can never wait on each other.

CLAIMS_STORE = "claims.json"  # in <git common dir>/atm/, shared by all worktrees of a clone
CLAIMS_STORE_NO_GIT = "claims-store.json"  # in the task folder, outside git
CLAIMS_SNAPSHOT = "claims.json"  # in the task folder (gitignored): what the dashboard reads
DEFAULT_LEASE_MINUTES = 30


def session_identity(session_id, cwd):
    """A Claude Code session's identity: "claude:" and 8 characters of its ID; in a linked git worktree, also
    "@<worktree folder>". Subagents share their parent's session ID, so agents spawned in separate worktrees still get
    identities of their own. The main worktree keeps the plain form, so its claims and history still match."""
    identity = "claude:" + str(session_id)[:8]
    root = project_root(cwd)
    if main_worktree(root) != root:
        identity += "@" + os.path.basename(root)
    return identity


def agent_identity(agent=None):
    """Who runs the command (the actor, DM-4.5): --agent, else ATM_AGENT, else this Claude Code session (per linked
    worktree, see session_identity), else the git user's name (a person in a terminal), else user@host."""
    if agent:
        return agent
    if os.environ.get("ATM_AGENT"):
        return os.environ["ATM_AGENT"]
    if os.environ.get("CLAUDE_CODE_SESSION_ID"):
        return session_identity(os.environ["CLAUDE_CODE_SESSION_ID"], os.getcwd())
    name = git(["config", "user.name"], project_root(os.getcwd()))
    if name:
        return name
    user = os.environ.get("USER") or os.environ.get("USERNAME") or "user"
    return f"{user}@{socket.gethostname()}"


def claims_dir(paths):
    common = git(["rev-parse", "--git-common-dir"], paths["root"])
    if common:
        return os.path.join(os.path.normpath(os.path.join(paths["root"], common)), "atm")
    return paths["dir"]


def claims_store_path(paths):
    directory = claims_dir(paths)
    return os.path.join(directory, CLAIMS_STORE if directory != paths["dir"] else CLAIMS_STORE_NO_GIT)


@contextlib.contextmanager
def locked_with_claims(paths):
    """Hold the task folder's lock and, if it is elsewhere, the claims store's lock (always in this order)."""
    with contextlib.ExitStack() as stack:
        stack.enter_context(locked(paths["dir"]))
        if claims_dir(paths) != paths["dir"]:
            stack.enter_context(locked(claims_dir(paths)))
        yield


def read_store(paths):
    """The shared store: live `claims`, and `completed` tasks (done in some worktree; its tasks.json may be the only
    one that says so until the branches merge, so `next` must not hand those tasks out again elsewhere); `waiting`
    tasks (blocked or sent for review in some worktree, likewise); `resolved` ones (answered, passed, rejected or
    skipped in another worktree than the one they wait in, to be applied there); and `hints`, when each hint was last
    shown (so `done` repeats one at most once a day)."""
    data = read_json(claims_store_path(paths))
    data = data if isinstance(data, dict) else {}

    def section(key):
        return data.get(key) if isinstance(data.get(key), dict) else {}

    return {
        "claims": data.get("claims") or {},
        "completed": data.get("completed") or {},
        "waiting": section("waiting"),
        "resolved": section("resolved"),
        "hints": section("hints"),
    }


def apply_resolutions(paths, data, store):
    """Apply what was decided in another worktree about a task waiting in this one (store["resolved"]): the answer,
    the passed or rejected review, the skip, as if given here, so this worktree's copy stops waiting too. A task changed
    here in the meantime is left as it is. Returns the IDs applied."""
    applied = []
    for task_id, record in list(store["resolved"].items()):
        if not isinstance(record, dict) or record.get("worktree") != paths["root"]:
            continue
        store["resolved"].pop(task_id)
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task is None:
            continue
        try:
            change_status(
                task,
                data,
                record.get("command"),
                agent=record.get("by"),
                reason=record.get("reason"),
                answers=record.get("answers") or [],
            )
            applied.append(task_id)
        except (AtmError, KeyError):
            pass
    return applied


def write_store(paths, store):
    os.makedirs(claims_dir(paths), exist_ok=True)
    write_json_atomic(claims_store_path(paths), store)
    write_claims_snapshot(paths, store["claims"])


def write_claims_snapshot(paths, claims):
    """The gitignored copy the dashboard polls: live claims, as a list."""
    if os.path.isdir(paths["dir"]):
        snapshot = {"generated_at": now_iso(), "claims": sorted(claims.values(), key=lambda c: c["task"])}
        write_json_atomic(os.path.join(paths["dir"], CLAIMS_SNAPSHOT), snapshot)


def lease_minutes(data):
    try:
        minutes = float(data.get("meta", {}).get("lease_minutes", DEFAULT_LEASE_MINUTES))
    except (TypeError, ValueError, AttributeError):
        return DEFAULT_LEASE_MINUTES
    return minutes if minutes > 0 else DEFAULT_LEASE_MINUTES


def lease_until(data):
    return (now() + timedelta(minutes=lease_minutes(data))).strftime("%Y-%m-%dT%H:%M:%SZ")


def expire_claims(paths, data, claims):
    """Drop claims whose lease ran out; their tasks in this worktree go back to todo. Returns the expired claims."""
    current = now()
    vocab = vocabulary(data)
    expired = []
    for task_id, claim in list(claims.items()):
        try:
            alive = parse_iso(claim["lease_expires_at"]) > current
        except (KeyError, TypeError, ValueError):
            alive = False
        if alive:
            continue
        expired.append(claims.pop(task_id))
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task and kind_of(vocab, task.get("status")) == "active":
            stamp = now_iso()
            back_to_todo(task, vocab, None, stamp, keep_commit=True)
            add_note(task, "failure", f"lease expired ({claim.get('agent')})", None, stamp)
    return expired


def check_holder(claims, task_id, agent, command):
    claim = claims.get(task_id)
    if claim and claim.get("agent") != agent:
        raise AtmError(
            f"can't {command} {task_id}: it is claimed by {claim.get('agent')} until {claim.get('lease_expires_at')}",
            4,
        )


def own_lease_expired(expired, data, task_id, agent, then):
    """When `agent`'s own claim on `task_id` is among the claims that just expired and the task went back to todo: an
    error that says so and how to go on (`then`, the command to run after `atm start`). Exit code 4, like a claim
    conflict. Another agent's expired claim, or none: None."""
    claim = next((c for c in expired if c.get("task") == task_id and c.get("agent") == agent), None)
    task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
    if not claim or not task or kind_of(vocabulary(data), task.get("status")) != "todo":
        return None
    until = str(claim.get("lease_expires_at", ""))
    at = f"{until[11:16]} UTC" if len(until) >= 16 else until
    minutes = lease_minutes(data)
    idle = f"{minutes:g} min"
    kept = " Your commit is kept." if task.get("commit") else ""
    next_steps = f"Run `atm start {task_id}`" + (f", then `{then}` again." if then else " to claim it again.")
    return AtmError(
        f"{task_id}: your lease expired at {at} (no heartbeat for {idle}), so it went back to todo.{kept} {next_steps}",
        4,
    )


def report_expired(expired):
    for claim in expired:
        sys.stderr.write(f"atm: lease of {claim['task']} by {claim.get('agent')} expired; it is available again\n")


def ready_in_order(data, store, candidates=None, rank=None):
    """The to-do tasks the loop could take, in the order it takes them: every dependency done, not claimed, not
    completed or waiting in another worktree; by `rank` (the selection's own order, SR-3.2), then the most urgent
    priority, then lowest ID."""
    vocab = vocabulary(data)
    if candidates is None:
        candidates = [t for t in data["tasks"] if kind_of(vocab, t.get("status")) == "todo"]
    taken = set(store["claims"]) | set(store["completed"]) | set(store["waiting"])
    ready = [t for t in candidates if not unfinished_dependencies(data, t) and t.get("id") not in taken]
    rank = rank or (lambda t: 0)
    order = lambda t: (rank(t), priority_rank(vocab, t.get("priority")), natural_key(t.get("id")))
    return sorted(ready, key=order)


class Selection:
    """What the loop works on (docs/sprints-and-releases-design.md §3.2): the active sprint (the default in a project
    with sprints), named sprints, a list of tasks, all tasks, or the backlog; `task_type` narrows it."""

    def __init__(self, sprints=(), tasks=(), all_tasks=False, backlog=False, task_type=None):
        self.sprints, self.tasks, self.all, self.backlog = list(sprints or []), list(tasks or []), all_tasks, backlog
        self.task_type = task_type


def select(data, sel):
    """The tasks `sel` covers, in any status, with how to rank them and a word for them. Returns (tasks, rank, scope,
    reason): reason is set when the selection can't be made (no active sprint, a closed sprint)."""
    vocab = vocabulary(data)
    sprints = vocab["sprints"]
    in_sprint = lambda names: (lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names)
    rank, scope, reason = None, "the tasks", None
    if sel.tasks:
        order = {task_id: i for i, task_id in enumerate(sel.tasks)}
        missing = [i for i in sel.tasks if i not in {t.get("id") for t in data["tasks"]}]
        if missing:
            raise AtmError(f"--tasks: no task {', '.join(missing)}", 2)
        keep, rank, scope = (lambda t: t.get("id") in order), (lambda t: order[t.get("id")]), "the listed tasks"
    elif sel.all:
        # The active sprint, then the planned ones in order, then the backlog (closed sprints' leftovers before it)
        place = {
            s["id"]: 0 if s["state"] == "active" else 1 + i if s["state"] == "planned" else len(sprints) + 1
            for i, s in enumerate(sprints)
        }
        keep = lambda t: True
        rank = lambda t: place.get((find_sprint(vocab, t.get("sprint")) or {}).get("id"), len(sprints) + 2)
        scope = "all tasks"
    elif sel.backlog:
        keep, scope = (lambda t: not find_sprint(vocab, t.get("sprint"))), "the backlog"
    elif sel.sprints:
        named = []
        for name in sel.sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise AtmError(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in sprints) or 'none'}", 2)
            if sprint["state"] == "closed":
                raise AtmError(f"the sprint {sprint['id']!r} is closed", 2)
            named.append(sprint["id"])
        keep, scope = in_sprint(named), "the sprint " + " and ".join(f"“{n}”" for n in named)
    elif sprints:
        active = [s["id"] for s in sprints if s["state"] == "active"]
        if not active:
            return [], None, "the active sprint", "no active sprint: plan one with /atm:plan-sprint, or run with --all"
        keep, scope = in_sprint(active), f"the sprint “{active[0]}”"
    else:
        keep = lambda t: True  # a project without sprints: every task, as before sprints
    tasks = [
        t
        for t in data["tasks"]
        if keep(t)
        and (sel.task_type is None or t.get("type") == sel.task_type)
    ]
    return tasks, rank, scope, reason


def offered_to(data, task, agent):
    """Whether a task back from review (rework or approved) may go to `agent`: always to the agent that worked on it;
    to others once one lease length has passed since the review ended (SR-3.9, Q11)."""
    if not agent or task.get("started_by") == agent:
        return True
    try:
        reviewed = parse_iso(task.get("reviewed_at"))
    except (TypeError, ValueError):
        return True
    return now() >= reviewed + timedelta(minutes=lease_minutes(data))


def loop_order(data, store, sel, agent=None, with_user=False):
    """Every task the loop could take now, in the order it takes them: `atm next` hands out the first, and atm serve's
    summary lists them as `next`, which the dashboard's Next up shows (T-321), so the two can't disagree. Returns
    (ready, selected, candidates, together, scope, problem); `problem` says why the selection itself is refused."""
    vocab = vocabulary(data)
    selected, rank, scope, problem = select(data, sel)
    if problem:
        return [], selected, [], [], scope, problem
    # Tasks back from review come first (SR-3.9): the agent that worked on them gets them back for a lease length
    rework = [
        t
        for t in selected
        if kind_of(vocab, t.get("status")) == "review"
        and t.get("review_state") in REWORK_STATES
        and offered_to(data, t, agent)
    ]
    first = {t.get("id") for t in rework}
    candidates = rework + [t for t in selected if kind_of(vocab, t.get("status")) == "todo"]
    # Work done together with the user waits for them, unless they're watching (SR-3.5)
    together = [t for t in candidates if t.get("with") == "user"]
    if not with_user:
        candidates = [t for t in candidates if t.get("with") != "user"]
    in_order = rank or (lambda t: 0)
    ready = ready_in_order(data, store, candidates, lambda t: (t.get("id") not in first, in_order(t)))
    return ready, selected, candidates, together, scope, None


def next_task(data, store, task_type=None, sel=None, agent=None, with_user=False):
    """The loop's rule (same as the dashboard's "Next up"): todo (or back from review), every dependency done, not
    claimed and not completed in another worktree, in the selection (the default one when `sel` is None); in its
    order, then the most urgent priority, then lowest ID. Returns (task, reason) where reason explains an empty
    result: the selection is finished, the rest of it waits, or it is empty (SR-3.6)."""
    vocab = vocabulary(data)
    sel = sel or Selection(task_type=task_type)
    ready, selected, candidates, together, scope, problem = loop_order(data, store, sel, agent, with_user)
    if problem:
        return None, problem
    if ready:
        return ready[0], None
    if together and not with_user and not candidates:
        ids = ", ".join(t.get("id") for t in together)
        return None, f"only work done together with the user is left in {scope}: {ids} (--with-user takes it)"
    if not candidates:
        if not selected:
            if not vocab["sprints"]:
                return None, "nothing left to do"
            return None, f"{scope} {'has' if scope.startswith('the sprint') else 'have'} no tasks"
        kinds = [kind_of(vocab, t.get("status")) for t in selected]
        open_kinds = {k: kinds.count(k) for k in ("active", "review", "blocked") if k in kinds}
        if open_kinds:
            words = {"active": "in progress", "review": "in review", "blocked": "blocked"}
            what = ", ".join(f"{n} {words[k]}" for k, n in open_kinds.items())
            return None, f"nothing ready in {scope}: the rest is waiting ({what})"
        if vocab["sprints"]:
            return None, f"{scope} is finished: {kinds.count('done')} done" + (
                f", {kinds.count('skipped')} skipped" if kinds.count("skipped") else ""
            )
        return None, "nothing left to do"
    claimed = [t["id"] for t in candidates if t.get("id") in store["claims"]]
    done_elsewhere = [t["id"] for t in candidates if t.get("id") in store["completed"]]
    waiting_elsewhere = [t["id"] for t in candidates if t.get("id") in store["waiting"]]
    reason = f"{len(candidates)} todo task(s) waiting on dependencies"
    if claimed:
        reason += f" or claimed by other agents ({', '.join(claimed)})"
    if done_elsewhere:
        reason += f"; done in another worktree, not merged yet: {', '.join(done_elsewhere)}"
    if waiting_elsewhere:
        reason += f"; blocked or in review in another worktree: {', '.join(waiting_elsewhere)}"
    return None, reason


# ==== db/working.py ====
# A store's working copy for one change (docs/layered-architecture.md §3.3): what atm-lib reads and changes inside a
# store's `change(fn)`, and the record of what it changed.
#
# A store runs `fn(work)` under its own isolation (the JSON-in-git store: its lock), writes what changed and only that,
# and raises what `fn` deferred after writing. `changes()` lists each task and claim `fn` changed, before and after: a
# store that keeps entities apart (a database) can turn the list into conditional writes, without atm-lib's rules
# changing. Until the last of the code from before the layers has moved, the copy is the whole task document.

def _stamp(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False)


class Working:
    def __init__(self, document, claims_store, here, size, before, archive=None):
        """`here` and `size` may be callables (worked out when asked); `before()` gives the document and the claims
        store as they were, from wherever the store keeps them (the JSON-in-git store: its files, not written until
        the change ends), so recording what changed costs nothing unless `changes()` is asked for. `archive()` loads
        the archive document, only when a change asks for it."""
        self.document = document
        self.claims_store = claims_store
        self._here = here
        self._size = size
        self._before = before
        self._load_archive = archive
        self._archive = None
        self._before_archive = None
        self.archive_first = False
        self._deferred = None
        self._before_store = _stamp(claims_store)

    def archive(self):
        """The archive document ({"meta", "tasks"}), to read and change in place; loaded on first use."""
        if self._archive is None:
            self._archive = self._load_archive() if self._load_archive else {"meta": {}, "tasks": []}
            self._before_archive = _stamp(self._archive)
        return self._archive

    def moving_into_archive(self):
        """This change moves tasks from the working tasks into the archive: a store that writes the two apart writes
        the archive first, so a crash between leaves a task in both, never in neither (DM-5.2). Without it (a
        restore), the working tasks go first."""
        self.archive_first = True

    def archive_changed(self):
        """Whether the archive was asked for and changed: the store writes it then, and only then."""
        return self._archive is not None and _stamp(self._archive) != self._before_archive

    @staticmethod
    def _entities(document, claims_store):
        tasks = {t.get("id"): t for t in document["tasks"] if isinstance(t, dict)}
        return {"task": tasks, "claim": dict(claims_store.get("claims") or {})}

    def tasks(self):
        """The working tasks, to read and change in place."""
        return self.document["tasks"]

    def claims(self):
        """The claims store: `claims`, `completed`, `waiting`, `resolved`, `hints`, to read and change in place."""
        return self.claims_store

    def here(self):
        """The working copy this change runs in: `worktree` (its folder, None for a store without them) and `branch`."""
        if callable(self._here):
            self._here = self._here()
        return self._here

    def size(self):
        """How big the task data is, in bytes, as it was read for this change (the archive hint)."""
        if callable(self._size):
            self._size = self._size()
        return self._size

    def defer(self, error):
        """Raise `error` once the change is written: a lease that expired is written as expired, then reported."""
        self._deferred = error

    def raise_deferred(self):
        if self._deferred is not None:
            raise self._deferred

    def changes(self):
        """What changed: [{"entity": "task" | "claim", "id", "before", "after"}], `before` or `after` None for an entity
        created or removed. The rest of the claims store (completed, waiting, resolved, hints) is in `store_changed`."""
        before = self._entities(*self._before())
        after = self._entities(self.document, self.claims_store)
        found = []
        for entity in ("task", "claim"):
            old, new = before[entity], after[entity]
            for key in list(old) + [k for k in new if k not in old]:
                if old.get(key) != new.get(key):
                    found.append({"entity": entity, "id": key, "before": old.get(key), "after": new.get(key)})
        return found

    def store_changed(self):
        return _stamp(self.claims_store) != self._before_store


# ==== db/jsongit/store.py ====
# The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
# has. For now it wraps the code from before the layers (`load_tasks`, the archive, the claims store); that code moves
# in here as the migration's slices do (§9).

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


# ==== db/memory.py ====
# The in-memory store: a project's data in plain lists, for atm-lib's tests. It also shows that atm-lib depends on
# the store's interface only: the contract tests (tests/lib/) run the same checks on it and on the JSON-in-git store.

def empty_claims():
    return {"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}}


class MemoryStore:
    def __init__(self, tasks=None, archive=None, meta=None, claims=None, problems=None):
        """`problems`: what a tolerant read would have found ({"file": [...], "tasks": {id: [...]}}), for the tests of
        what atm-lib makes of them; the memory holds data, not a file to read."""
        self.meta = dict(meta or {})
        self.tasks = list(tasks or [])
        self.archive = list(archive or [])
        self.claims = {**empty_claims(), **(claims or {})}
        self._problems = {"file": [], "tasks": {}, **(problems or {})}

    def problems(self):
        return self._problems

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

    def files_for_commit(self):
        return []  # nothing of the memory travels with code commits

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
        work.raise_deferred()
        return result


# ==== vcs/git.py ====
# Git as the code's version control (docs/layered-architecture.md §2): committing exactly the files named, the
# history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's.

# Pathspecs that stage more than the files named: another agent's work can be in the same tree
WHOLE_TREE = {".", "-A", "--all", "-u", "--update", ":/", "*"}
# git reads a path as a pattern (`*.env` is every .env file, `src/[id]` matches `src/i`); with this, a path names
# exactly one file (docs/security-review.md F6)
LITERAL = ["--literal-pathspecs"]


def run_git(args, root):
    """Run git; return stdout stripped, or raise with git's own message."""
    result = subprocess.run(["git"] + args, cwd=root, capture_output=True, text=True)
    if result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        command = next((a for a in args if not a.startswith("-")), "")
        raise AtmFailure(f"git {command} failed: {message}")
    return result.stdout.strip()


def ask_git(args, root):  # not `git`: the store has its own (db/jsongit/gitrun.py), and names are unique
    """Run git; return stdout stripped, or None if git fails or isn't installed (a question, not a step)."""
    try:
        result = subprocess.run(["git"] + args, cwd=root, capture_output=True, text=True)
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


class GitVcs:
    def __init__(self, root):
        self.root = root

    def in_repository(self):
        return ask_git(["rev-parse", "--git-dir"], self.root) is not None

    def branch(self):
        """The main worktree's checked-out branch (a detached HEAD's short SHA), read from .git/HEAD without running
        git; None outside git."""
        try:
            with open(os.path.join(self.root, ".git", "HEAD"), encoding="utf-8") as f:
                head = f.readline().strip()
        except OSError:
            return None
        if head.startswith("ref: "):
            ref = head[5:]
            return ref[len("refs/heads/") :] if ref.startswith("refs/heads/") else ref
        return head[:7] or None

    def user_name(self):
        """Who git says the user is (`user.name`), for naming the actor; None when it isn't set."""
        return ask_git(["config", "user.name"], self.root)

    def default_base(self):
        """The remote's default branch (origin/HEAD), else main."""
        ref = ask_git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], self.root)
        return ref.split("/", 1)[1] if ref and "/" in ref else "main"

    def remote_url(self):
        """The origin remote's address, as git has it (atm repository turns it into a web address)."""
        return ask_git(["remote", "get-url", "origin"], self.root)

    def paths(self, files, cwd):
        """The files to commit, relative to the repository, from names relative to `cwd`; refuses anything that
        would stage more than the files named, and a missing file git doesn't track."""
        picked = []
        for name in files:
            if name in WHOLE_TREE or name.startswith(":"):
                raise Invalid(f"atm commit stages only the files you name; {name!r} would stage the whole tree")
            full = os.path.realpath(os.path.join(cwd, name))
            relative = os.path.relpath(full, self.root)
            if relative == "." or relative.startswith(".." + os.sep) or relative == "..":
                raise Invalid(f"{name} is outside the repository")
            if os.path.isdir(full):
                raise Invalid(f"{name} is a directory: name the files in it")
            # A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
            if (
                not os.path.exists(full)
                and ask_git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is None
                and ask_git(["cat-file", "-e", f"HEAD:{relative}"], self.root) is None
            ):
                raise NotFound(f"{name}: no such file, and git doesn't track it")
            if relative not in picked:
                picked.append(relative)
        return picked

    def commit(self, files, message):
        """Commit exactly `files` (relative to the repository); whatever else is staged stays staged and out of this
        commit. Returns the new commit's SHA."""
        # A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
        to_add = [
            f
            for f in files
            if ask_git(LITERAL + ["ls-files", "--error-unmatch", "--", f], self.root) is not None
            or os.path.lexists(os.path.join(self.root, f))
        ]
        if to_add:
            run_git(LITERAL + ["add", "-A", "--"] + to_add, self.root)
        run_git(LITERAL + ["commit", "-q", "-m", message, "--"] + list(files), self.root)
        return run_git(["rev-parse", "HEAD"], self.root)

    def resolve(self, rev):
        """A revision's SHA, or None if there is no such commit."""
        return ask_git(["rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"], self.root)

    def in_history(self, sha):
        """Whether the commit is an ancestor of HEAD (on the current branch)."""
        return ask_git(["merge-base", "--is-ancestor", sha, "HEAD"], self.root) is not None

    def message(self, sha):
        return run_git(["log", "-1", "--format=%B", sha], self.root)


# ==== lib/project.py ====
# A project as atm-lib sees it: where its data is kept, its code's version control, and who acts.

class Project:
    def __init__(self, store, vcs=None, actor=None):
        self.store = store
        self.vcs = vcs
        self.actor = actor


# ==== lib/tasks.py ====
# atm-lib: reading tasks (docs/layered-architecture.md §5.1).

def view_task(task, archived, tasks, archived_lookup, vocab):
    """A task with what is derived from the rest of the project: its effective priority, its dependencies' statuses,
    the tasks related to it either way (`related` is stored on one side, SR-2.8) and those that duplicate it.

    `tasks` are the working tasks, `archived_lookup(id)` an archived task or None, `vocab` the vocabularies."""
    by_id = {t.get("id"): t for t in tasks}
    dependencies = []
    for d in task.get("dependencies") or []:
        if d in by_id:
            dependencies.append({"id": d, "status": by_id[d].get("status"), "archived": False})
        elif archived_lookup(d):
            dependencies.append({"id": d, "status": archived_lookup(d).get("status"), "archived": True})
        else:
            dependencies.append({"id": d, "status": None, "archived": False})
    own = [r for r in task.get("related") or [] if isinstance(r, str)]
    others = [
        t.get("id")
        for t in tasks
        if t is not task and task.get("id") in (t.get("related") or []) and t.get("id") not in own
    ]
    return {
        "task": task,
        "archived": archived,
        "priority": priority_of(vocab, task),
        "started_by": started_by(task),
        "dependencies": dependencies,
        "related": own + others,
        "duplicated_by": [t.get("id") for t in tasks if t.get("duplicate_of") == task.get("id")],
    }


def show_tasks(project, ids):
    """`atm show`: the tasks in full, archived ones too, in the order asked for."""
    store = project.store
    tasks = store.list_tasks()
    vocab = store.get_vocabularies()
    views = []
    for task_id in ids:
        task, archived = store.get_task(task_id), False
        if task is None:
            task, archived = store.get_archived_task(task_id), True
        if task is None:
            raise NotFound(f"no task {task_id}")
        views.append(view_task(task, archived, tasks, store.get_archived_task, vocab))
    return views


# ==== lib/query.py ====
# atm-lib: the read commands (docs/layered-architecture.md §5.1): list, deps, status, the project's summary, the
# backlog. Agents read tasks through these, never tasks.json itself (DM-1.1). They never write: the store reads the
# task files and the claims without the lock (both are replaced atomically).
#
# Much of it still takes the whole task document (`store.document()`) for the rules from before the layers it calls
# (`unfinished_dependencies`, `loop_order`, ...); those move into atm-lib with their own slices (§9).

class Filters:
    """What a read keeps (`atm list`'s and `atm status`'s options): any of `sprint` (a list of names), `backlog`,
    `release`, `component`, `status` (a list), `type`, `priority`, `label` (a list: all of them), `agent`, `ready`,
    `blocked`, `in_pr`, `with_user`, `archived`; what isn't given keeps everything. The CLI passes its parsed
    arguments, which have these names."""

    def __init__(self, **given):
        self.__dict__.update(given)

    def __getattr__(self, name):
        return None


def summary(vocab, task):
    """The fields `list` shows, as a dict for --json."""
    fields = {
        "id": task.get("id"),
        "status": task.get("status"),
        "kind": kind_of(vocab, task.get("status")),
        "sprint": task.get("sprint"),
        "priority": priority_of(vocab, task),
        "labels": task.get("labels", []),
        "name": task.get("name", ""),
    }
    if isinstance(task.get("pr"), dict):
        fields["pr"] = task["pr"]
        fields["merged_at"] = task.get("merged_at")
    return fields


def scope_filter(vocab, args):
    """A test for the tasks `--sprint` (repeatable), `--backlog`, `--release` and `--component` keep (SR-3.2); a
    task's release is its effective one (its own, else its sprint's)."""
    names = []
    for name in getattr(args, "sprint", None) or []:
        sprint = find_sprint(vocab, name)
        if sprint is None:
            raise NotFound(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in vocab['sprints']) or 'none'}", 2)
        names.append(sprint["id"])
    if getattr(args, "release", None):
        known_release(vocab, args.release)

    def keep(task):
        sprint = (find_sprint(vocab, task.get("sprint")) or {}).get("id")
        if names and sprint not in names:
            return False
        if getattr(args, "backlog", False) and sprint:
            return False
        if getattr(args, "release", None) and effective_release(vocab, task) != args.release:
            return False
        return not getattr(args, "component", None) or args.component in (task.get("components") or [])

    return keep


def in_open_pr(task):
    """In PR: a recorded PR, not merged yet (DM-4.3)."""
    return isinstance(task.get("pr"), dict) and not task.get("merged_at")


def ready_tasks(data):
    """To-do tasks whose dependencies are all done, in the loop's order."""
    vocab = vocabulary(data)
    ready = [
        t
        for t in data["tasks"]
        if kind_of(vocab, t.get("status")) == "todo" and not unfinished_dependencies(data, t)
    ]
    return sorted(ready, key=lambda t: (priority_rank(vocab, t.get("priority")), natural_key(t.get("id"))))


def list_matching(project, filters):
    """`atm list`: the tasks the filters keep, in the project's order (the archive's with `archived`)."""
    store = project.store
    data = store.document()
    vocab = vocabulary(data)
    args = filters
    ready = {t.get("id") for t in ready_tasks(data)}
    in_scope = scope_filter(vocab, args)
    tasks = []
    for t in store.list_archived() if args.archived else data["tasks"]:
        kind = kind_of(vocab, t.get("status"))
        if not in_scope(t):
            continue
        if args.status and t.get("status") not in args.status:
            continue
        if args.type and t.get("type") != args.type:
            continue
        if args.priority and priority_of(vocab, t) != args.priority:
            continue
        if args.label and not set(args.label) <= set(t.get("labels") or []):
            continue
        if args.agent and started_by(t) != args.agent:
            continue
        if args.ready and t.get("id") not in ready:
            continue
        if args.blocked and not (kind == "blocked" or (kind == "todo" and t.get("id") not in ready)):
            continue
        if args.in_pr and not in_open_pr(t):
            continue
        if args.with_user and t.get("with") != "user":
            continue
        tasks.append(t)
    return {"vocab": vocab, "tasks": tasks}


def task_stage(vocab, task):
    """A done task's stage (DM-4.3): `done`, `in_pr` (a recorded PR, not merged) or `merged`; None for other kinds."""
    if kind_of(vocab, task.get("status")) != "done":
        return None
    if task.get("merged_at"):
        return "merged"
    return "in_pr" if isinstance(task.get("pr"), dict) else "done"


def task_views(project, filters):
    """The tasks the filters keep, each with what atm derives for it (the REST API's tasks, docs/layered-architecture.md
    §6.2): its kind, whether it is ready, the dependencies it waits for, who a blocked task waits on, its effective
    priority, review mode and release, its stage, and whether it is archived. The dashboard takes these instead of
    working them out itself."""
    found = list_matching(project, filters)
    data, vocab = project.store.document(), found["vocab"]
    ready = {t.get("id") for t in ready_tasks(data)}
    # A task a tolerant read found wrong (T-377) says so; what its broken fields would decide isn't decided
    troubled = {} if filters.archived else project.store.problems()["tasks"]
    views = []
    for t in found["tasks"]:
        kind = kind_of(vocab, t.get("status"))
        problems = troubled.get(t.get("id"))
        values = {
            "kind": kind,
            "ready": t.get("id") in ready and not problems,
            "waiting_for": [d for d, _ in unfinished_dependencies(data, t)] if kind == "todo" else [],
            "problems": problems,
            "waiting_on": t.get("waiting_on") if kind == "blocked" else None,
            "priority": priority_of(vocab, t),
            "effective_review": effective_review(vocab, t),
            "effective_release": effective_release(vocab, t),
            "stage": task_stage(vocab, t),
            "archived": bool(filters.archived),
        }
        # What doesn't apply is left out, not null (api.schema.json's optional fields)
        views.append({"task": t, "derived": {k: v for k, v in values.items() if v is not None}})
    return views


def read_problems(project):
    """What a tolerant read found wrong with the project's task data as a whole (T-377): entries left out, the
    vocabularies that fell back to their defaults. Empty for a strict read, which refuses such data instead."""
    return list(project.store.problems()["file"])


def search_text(task):
    """What a task is found by: its ID, name, description, type, labels and notes (T-258)."""
    strings = [task.get(f) for f in ("id", "name", "description", "type")]
    strings += task.get("labels") if isinstance(task.get("labels"), list) else []
    strings += [n.get("text") for n in task.get("notes") or [] if isinstance(n, dict)]
    return " ".join(s for s in strings if isinstance(s, str)).lower()


def search_tasks(project, query, also=""):
    """The IDs of the working tasks (not the archive) in which every word of `query` appears, case-insensitively, in
    the task's text or in `also` (the project's name, so it finds all of its tasks), in the project's order."""
    terms = query.lower().split()
    if not terms:
        return []
    extra = also.lower()
    return [
        t["id"]
        for t in project.store.list_tasks()
        if isinstance(t.get("id"), str) and all(term in f"{extra} {search_text(t)}" for term in terms)
    ]


def list_claims(project):
    """The project's claims, from every working copy, by task."""
    claims = project.store.read_claims()["claims"]
    listed = [c for c in claims.values() if isinstance(c, dict) and isinstance(c.get("task"), str)]
    return sorted(listed, key=lambda c: c["task"])


def task_dependencies(project, task_id):
    """`atm deps`: what a task waits for, and what waits for it, archived tasks included (T-358)."""
    store = project.store
    task = store.get_task(task_id) or store.get_archived_task(task_id)
    if task is None:
        raise NotFound(f"no task {task_id}")
    by_id = {t.get("id"): t for t in store.list_tasks()}

    def dependency(d):
        found = by_id.get(d) or store.get_archived_task(d)
        if not found:
            return {"id": d, "status": "missing", "name": ""}
        return {"id": d, "status": found.get("status") + (", archived" if d not in by_id else ""), "name": found.get("name", "")}

    waits_for = [dependency(d) for d in task.get("dependencies") or []]
    waited_on_by = [
        {"id": t.get("id"), "status": t.get("status"), "name": t.get("name", "")}
        for t in store.list_tasks()
        if task_id in (t.get("dependencies") or [])
    ]
    # Archived tasks that depended on it are history too, marked as the waits-for side marks them (T-358); a task in
    # both files is the tasks.json one
    waited_on_by += [
        {"id": t.get("id"), "status": f"{t.get('status')}, archived", "name": t.get("name", "")}
        for t in store.list_archived()
        if t.get("id") not in by_id and isinstance(t.get("dependencies"), list) and task_id in t["dependencies"]
    ]
    return {"id": task_id, "waits_for": waits_for, "waited_on_by": waited_on_by}


ACTIVITY_FIELDS = ("updated_at", "started_at", "completed_at", "review_requested_at", "blocked_at", "skipped_at")
SUMMARY_DAYS = 14


def parse_time(text):
    try:
        return parse_iso(text) if isinstance(text, str) else None
    except ValueError:
        return None


RECENT_SPRINTS = 4
NEXT_TASKS = 2
NAME_CAP = 80  # task names in the summary, so /api/projects stays small with many projects


def capped(name):
    name = name if isinstance(name, str) else ""
    return name if len(name) <= NAME_CAP else name[: NAME_CAP - 1] + "…"


def sprint_numbers(sprint, own_kinds):
    """A sprint as the overview shows it: its name, state, due date and release, and done of its tasks but the skipped."""
    done, total = own_kinds.count("done"), len(own_kinds) - own_kinds.count("skipped")
    return {
        "id": sprint["id"],
        "state": sprint["state"],
        "due": sprint.get("due"),
        "release": sprint.get("release"),
        "done": done,
        "total": total,
        "percent": round(100 * done / total) if total else 100,
    }


def recent_sprints(vocab, tasks, kinds):
    """Up to four sprints with tasks for the overview's rings (SR-7.7), in the project's order: the active one and the
    planned ones first, then the most recently closed (by their end) to fill up. None for a project without sprints."""
    if not vocab["sprints"]:
        return None
    own = {s["id"]: [] for s in vocab["sprints"]}
    for t, k in zip(tasks, kinds):
        sprint = find_sprint(vocab, t.get("sprint"))
        if sprint:
            own[sprint["id"]].append(k)
    with_tasks = [s for s in vocab["sprints"] if own[s["id"]]]
    chosen = [s["id"] for s in with_tasks if s["state"] != "closed"][:RECENT_SPRINTS]
    closed = sorted((s for s in with_tasks if s["state"] == "closed"), key=lambda s: s.get("end") or "", reverse=True)
    chosen += [s["id"] for s in closed[: RECENT_SPRINTS - len(chosen)]]
    return [sprint_numbers(s, own[s["id"]]) for s in with_tasks if s["id"] in chosen]


def active_sprint_numbers(vocab, tasks, kinds):
    """The active sprint for the overview's cards (SR-7.7), with its progress; None when none is active."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    own = [k for t, k in zip(tasks, kinds) if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
    return {**sprint_numbers(active, own), "goal": active.get("goal")}


DESCRIPTION_CAP = 200


def description(meta):
    """meta.description (an optional line about the project, set with atm init --meta-file), capped at 200 characters
    for the overview, where it is the project name's tooltip (T-209); None without one."""
    text = meta.get("description") if isinstance(meta, dict) else None
    if not isinstance(text, str) or not text.strip():
        return None
    text = text.strip()
    return text if len(text) <= DESCRIPTION_CAP else text[: DESCRIPTION_CAP - 1] + "…"


def git_summary(project, tasks):
    """The project's git line for its card (T-206): the main worktree's branch (read from .git/HEAD, no git process),
    and the latest PR recorded on its tasks (`atm pr`; the highest number), `merged` once its tasks have merged_at.
    No forge call and no CI state."""
    branch = project.vcs.branch() if project.vcs else None
    prs = {}
    for t in tasks:
        pr = t.get("pr")
        if isinstance(pr, dict) and isinstance(pr.get("number"), int):
            entry = prs.setdefault(pr["number"], {"number": pr["number"], "url": None, "state": "open"})
            # The dashboard links to it: only a web address (docs/security-review.md F9)
            if isinstance(pr.get("url"), str) and re.match(r"https?://\S+\Z", pr["url"], re.IGNORECASE):
                entry["url"] = pr["url"]
            if t.get("merged_at"):
                entry["state"] = "merged"
    return {"branch": branch, "pr": prs[max(prs)] if prs else None}


BOARD_CAP = 30  # cards per column of the All projects board (docs/all-projects-kanban-proposal.md, 3.1)
BOARD_READY_CAP = 10  # to-do tasks per project, so one project's new batch doesn't fill the To Do column
BOARD_DONE_HOURS = 24


def board_card(vocab, task, **extra):
    """What a project's Kanban card shows, nothing more (T-257)."""
    labels = task.get("labels")
    card = {
        "id": task.get("id"),
        "name": capped(task.get("name")),
        "type": task.get("type"),
        "sprint": task.get("sprint") if isinstance(task.get("sprint"), str) else None,
        "priority": priority_of(vocab, task),
        "labels": [l for l in labels if isinstance(l, str)] if isinstance(labels, list) else [],
    }
    card.update(extra)
    return card


def board_column(cards, cap=BOARD_CAP):
    """A column: its first `cap` cards, and how many there are in all (for "+ N more")."""
    return {"count": len(cards), "cards": cards[:cap]}


def todo_order(data, tasks, kinds, loop_ready, vocab):
    """Every to-do task, for the board's To Do column (T-349): first the ready ones, those the loop takes in its order
    before the rest (other sprints', the backlog's), then the ready ones done with the user, then those waiting on
    dependencies (each by priority, then ID). Each with why it isn't plain ready: done with the user, or the
    dependencies it waits for."""
    rank = {t.get("id"): i for i, t in enumerate(loop_ready)}

    def entry(t):
        waits = [dep for dep, _ in unfinished_dependencies(data, t)]
        key = (
            bool(waits),
            t.get("with") == "user",
            t.get("id") not in rank,
            rank.get(t.get("id"), 0),
            priority_rank(vocab, t.get("priority")),
            natural_key(t.get("id")),
        )
        return key, t, waits

    entries = sorted((entry(t) for t, k in zip(tasks, kinds) if k == "todo"), key=lambda e: e[0])
    return [(t, waits) for _, t, waits in entries]


def board(data, store, tasks, kinds, todo, vocab, current, live, recent_done=True):
    """The project's cards for the All projects board (T-257), one rule per column: every to-do task (`todo`, as
    todo_order gives it, T-349); everything in progress (longest running first), in review (longest waiting first)
    and blocked (on the user first, then longest waiting); what was finished, newest first: in the last 24 hours, or
    all of it without `recent_done` (a sprint's board shows all its tasks, T-349)."""
    of_kind = lambda kind: [t for t, k in zip(tasks, kinds) if k == kind]
    claimed_by = {c.get("task"): c.get("agent") for c in live}
    since = lambda t, field: t.get(field) or t.get("updated_at")
    active = sorted(of_kind("active"), key=lambda t: str(t.get("started_at") or ""))
    review = sorted(of_kind("review"), key=lambda t: str(since(t, "review_requested_at") or ""))
    blocked = sorted(
        of_kind("blocked"),
        key=lambda t: (t.get("waiting_on") != "user", str(since(t, "blocked_at") or "")),
    )
    cutoff = current.timestamp() - BOARD_DONE_HOURS * 3600
    finished_at = lambda t: parse_time(t.get("completed_at"))
    done = [
        t for t in of_kind("done") if not recent_done or (finished_at(t) and finished_at(t).timestamp() > cutoff)
    ]
    done.sort(key=lambda t: str(t.get("completed_at")), reverse=True)
    return {
        "todo": board_column(
            [
                board_card(
                    vocab,
                    t,
                    **({"with_user": True} if t.get("with") == "user" else {}),
                    **({"waiting_for": waits} if waits else {}),
                )
                for t, waits in todo
            ],
            BOARD_READY_CAP,
        ),
        "active": board_column(
            [
                board_card(vocab, t, by=claimed_by.get(t.get("id")) or started_by(t), since=since(t, "started_at"))
                for t in active
            ]
        ),
        "review": board_column([board_card(vocab, t, since=since(t, "review_requested_at")) for t in review]),
        "blocked": board_column(
            [
                board_card(
                    vocab,
                    t,
                    since=since(t, "blocked_at"),
                    waiting_on=t.get("waiting_on"),
                    questions=len(t.get("questions") or []),
                )
                for t in blocked
            ]
        ),
        "done": board_column([board_card(vocab, t, completed_at=t.get("completed_at")) for t in done]),
    }


def work_span(vocab, task, current):
    """A task's work as (start, end) timestamps, as the dashboard counts it (work.ts): from started_at until it asked
    for a review or was done, or until now while active; None for a task that holds none."""
    start = parse_time(task.get("started_at"))
    if not start:
        return None
    kind = kind_of(vocab, task.get("status"))
    if kind == "active":
        end = max(current, start)
    elif kind in ("done", "review"):
        end = (
            parse_time(task.get("review_requested_at"))
            or parse_time(task.get("completed_at") if kind == "done" else task.get("updated_at"))
            or start
        )
    else:
        return None
    return (start.timestamp(), max(end, start).timestamp())


def agent_minutes(vocab, tasks, current):
    """Agent time (docs/project-stats-proposal.md, T-255): the work of tasks an agent started, or nobody recorded
    (before v4), each agent's spans merged so an agent on two tasks counts once; parallel agents add up. As the
    dashboard's Project tab counts it (projectstats.ts)."""
    spans = {}
    for i, t in enumerate(tasks):
        starter = started_by(t)
        if starter and not str(starter).startswith("claude:"):
            continue
        span = work_span(vocab, t, current)
        if span:
            spans.setdefault(starter or i, []).append(span)  # unattributed spans are each their own
    total = 0.0
    for agent_spans in spans.values():
        end = None
        for s, e in sorted(agent_spans):
            if end is None or s > end:
                total += e - s
                end = e
            elif e > end:
                total += e - end
                end = e
    return round(total / 60)


def project_stats(store, data, vocab, tasks, current):
    """The overview card's line (T-255): when the project started (its first task's created_at, else meta.created),
    how many tasks it has, and the agent time, archived tasks included; an unreadable archive is left out. (A task
    folder with a symlinked file never gets here: find_paths refuses it, docs/security-review.md F8.)"""
    try:
        archived = store.list_archived()
    except AtmFailure:
        archived = []
    ids = {t.get("id") for t in tasks}
    every = tasks + [t for t in archived if t.get("id") not in ids]
    created = [parse_time(t.get("created_at")) for t in every]
    created = [c for c in created if c] or [parse_time((data.get("meta") or {}).get("created"))]
    started = min((c for c in created if c), default=None)
    return {
        "started": started.strftime("%Y-%m-%dT%H:%M:%SZ") if started else None,
        "tasks": len(every),
        "agent_minutes": agent_minutes(vocab, every, current),
    }


def project_summary(project):
    """A project's numbers for the dashboard's overview and project selector (docs/multi-project-api.md, 3.2), by
    status kind as atm counts them everywhere else: done / (total - skipped), the active and recent sprints, live
    claims, what needs the user, the latest activity and the completions of the last 14 days."""
    data = project.store.document()
    vocab = vocabulary(data)
    tasks = data["tasks"]
    kinds = [kind_of(vocab, t.get("status")) for t in tasks]
    done, skipped = kinds.count("done"), kinds.count("skipped")
    counted = len(tasks) - skipped
    percent = round(100 * done / counted) if counted else (100 if skipped else 0)

    current = now()
    store = project.store.read_claims()
    live = sorted(
        (
            c
            for c in store["claims"].values()
            if isinstance(c, dict) and (parse_time(c.get("lease_expires_at")) or current) > current
        ),
        key=lambda c: str(c.get("claimed_at") or ""),
    )
    agents_working = len(live)
    by_id = {t.get("id"): t for t in tasks}
    # What the cards show by name (T-205): who works on what now, what the loop takes next, the last finished task
    working = [
        {
            "task": c.get("task"),
            "name": capped((by_id.get(c.get("task")) or {}).get("name")),
            "agent": c.get("agent"),
            "branch": c.get("branch"),
        }
        for c in live
    ]
    # What the loop takes next with its default selection (the active sprint's tasks in a project with sprints, SR-5.1),
    # by the very rule `atm next` follows: the dashboard's Next up shows the first (T-321)
    in_loop_order = loop_order(data, store, Selection())[0]
    next_up = [{"id": t.get("id"), "name": capped(t.get("name"))} for t in in_loop_order[:NEXT_TASKS]]
    # The board's To Do column: every to-do task, the loop's first (T-349)
    todo = todo_order(data, tasks, kinds, in_loop_order, vocab)
    finished = [(t.get("completed_at"), t) for t, k in zip(tasks, kinds) if k == "done" and parse_time(t.get("completed_at"))]
    last_task = max(finished, key=lambda f: f[0])[1] if finished else None
    last_done = (
        {"id": last_task.get("id"), "name": capped(last_task.get("name")), "completed_at": last_task.get("completed_at")}
        if last_task
        else None
    )

    times = [parse_time(t.get(f)) for t in tasks for f in ACTIVITY_FIELDS]
    times += [parse_time(n.get("at")) for t in tasks for n in t.get("notes") or [] if isinstance(n, dict)]
    times = [t for t in times if t]
    last = max(times) if times else None

    completions = [0] * SUMMARY_DAYS
    today = current.date()
    for t, k in zip(tasks, kinds):
        finished = parse_time(t.get("completed_at")) if k == "done" else None
        if finished:
            age = (today - finished.date()).days
            if 0 <= age < SUMMARY_DAYS:
                completions[SUMMARY_DAYS - 1 - age] += 1

    # The tasks that need the user, as the dashboard's needsYouReason decides it: in review and waiting for the
    # reviewer, blocked on the user, or to do together with the user (SR-6.6) now: in the active sprint (any, without
    # one) and ready, its dependencies done (T-347)
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)

    def together_now(t):
        in_scope = active is None or (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
        return in_scope and not unfinished_dependencies(data, t)

    def needs_you(t, k):
        if k == "review":
            return None if t.get("review_state") in REWORK_STATES else "review"
        if k == "blocked" and t.get("waiting_on") == "user":
            return "blocked"
        return "together" if k == "todo" and t.get("with") == "user" and together_now(t) else None

    needs_you_tasks = [
        {"id": t.get("id"), "name": capped(t.get("name")), "reason": needs_you(t, k)}
        for t, k in zip(tasks, kinds)
        if needs_you(t, k)
    ]

    return {
        "total": len(tasks),
        "done": done,
        "skipped": skipped,
        "percent": percent,
        "active_sprint": active_sprint_numbers(vocab, tasks, kinds),
        "recent_sprints": recent_sprints(vocab, tasks, kinds),
        "agents_working": agents_working,
        "working": working,
        "next": next_up,
        "last_done": last_done,
        "needs_you": {
            "review": sum(1 for item in needs_you_tasks if item["reason"] == "review"),
            "blocked_on_user": sum(1 for item in needs_you_tasks if item["reason"] == "blocked"),
            "together": sum(1 for item in needs_you_tasks if item["reason"] == "together"),
        },
        "needs_you_tasks": needs_you_tasks,
        "git": git_summary(project, tasks),
        "description": description(data.get("meta")),
        "last_activity": last.strftime("%Y-%m-%dT%H:%M:%SZ") if last else None,
        "completions_14d": completions,
        "board": board(data, store, tasks, kinds, todo, vocab, current, live),
        "board_active_sprint": active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live),
        "stats": project_stats(project.store, data, vocab, tasks, current),
    }


def active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live):
    """The All projects board's default view (T-340): the same columns, of the active sprint's tasks only, no more, no
    less (T-349: every one done too, not the last 24 hours'), with their own counts; None without an active sprint (the
    board says so)."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    in_sprint = lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
    pairs = [(t, k) for t, k in zip(tasks, kinds) if in_sprint(t)]
    return board(
        data,
        store,
        [t for t, _ in pairs],
        [k for _, k in pairs],
        [(t, waits) for t, waits in todo if in_sprint(t)],
        vocab,
        current,
        live,
        recent_done=False,
    )


def status_report(project, scope):
    """`atm status`: the overview, in the scope's tasks (`Filters` with sprint, backlog, release, component): the
    active sprint, counts, what is in progress, in review, blocked or waiting, the next task, the files' size and the
    hints. `report` is what --json prints; `has_sprints` says whether the project has any."""
    data = project.store.document()
    vocab = vocabulary(data)
    store = project.store.read_claims()
    sizes = project.store.sizes()
    args = scope
    in_scope = scope_filter(vocab, args)
    tasks = [t for t in data["tasks"] if in_scope(t)]
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    sprint = None
    if active:
        in_active = [t for t in data["tasks"] if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
        sprint = {
            **{k: active[k] for k in ("id", "goal", "due", "release", "start") if active.get(k)},
            "done": sum(1 for t in in_active if kind_of(vocab, t.get("status")) == "done"),
            "tasks": sum(1 for t in in_active if kind_of(vocab, t.get("status")) != "skipped"),
            # Batch tasks waiting for the end-of-sprint review: they wait on the user (SR-4.4)
            "review_pending": [t.get("id") for t in in_active if t.get("sprint_review") == "pending"],
        }

    def counts(key):
        result = {}
        for t in tasks:
            for value in key(t):
                result[str(value)] = result.get(str(value), 0) + 1
        return result

    status_order = [s["id"] for s in vocab["statuses"]]
    by_status = counts(lambda t: [t.get("status")])
    by_status = {s: by_status[s] for s in status_order if s in by_status} | {
        s: n for s, n in by_status.items() if s not in status_order
    }
    in_progress = [
        {
            "id": t.get("id"),
            "name": t.get("name", ""),
            "by": (store["claims"].get(t.get("id")) or {}).get("agent") or started_by(t),
        }
        for t in tasks
        if kind_of(vocab, t.get("status")) == "active"
    ]
    waiting = [
        {"id": t.get("id"), "name": t.get("name", ""), "on": [d for d, _ in unfinished_dependencies(data, t)]}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "todo" and unfinished_dependencies(data, t)
    ]
    blocked = [
        {"id": t.get("id"), "name": t.get("name", ""), "waiting_on": t.get("waiting_on"), "questions": t.get("questions", [])}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "blocked"
    ]
    in_review = [
        {"id": t.get("id"), "name": t.get("name", ""), "since": t.get("review_requested_at")}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "review"
    ]
    # Blocked or in review in another worktree (T-204): this worktree's copy doesn't say so until the branches merge
    task_names = {t.get("id"): t.get("name", "") for t in tasks}
    listed = {i["id"] for i in blocked + in_review}
    for record in store["waiting"].values():
        here = project.store.working_copy()
        if not isinstance(record, dict) or record.get("worktree") == here or record.get("task") in listed:
            continue
        item = {"id": record.get("task"), "name": task_names.get(record.get("task"), ""), "branch": record.get("branch")}
        if kind_of(vocab, record.get("status")) == "review":
            in_review.append({**item, "since": record.get("at")})
        else:
            blocked.append({**item, "waiting_on": record.get("waiting_on"), "questions": record.get("questions") or []})
    nxt, reason = next_task(data, store)
    largest = sorted(((len(json.dumps(t, ensure_ascii=False)), t.get("id")) for t in tasks), reverse=True)
    result = {
        "sprint": sprint,
        "tasks": len(tasks),
        "by_status": by_status,
        "by_sprint": counts(lambda t: [(find_sprint(vocab, t.get("sprint")) or {}).get("id") or "backlog"]),
        "by_type": counts(lambda t: [t.get("type")]),
        "by_label": counts(lambda t: t.get("labels") or []),
        "in_progress": in_progress,
        "in_review": in_review,
        "blocked": blocked,
        "waiting_on_dependencies": waiting,
        "next": {"id": nxt.get("id"), "name": nxt.get("name", "")} if nxt else None,
        "next_reason": reason,
        "file_bytes": sizes["tasks"],
        "largest_tasks": [{"id": task_id, "bytes": size} for size, task_id in largest[:3]],
        "archived": len(project.store.list_archived()),
        "archive_bytes": sizes["archive"],
    }
    # Open tasks left in closed sprints (T-330), whatever the scope: the loop's default selection never takes them
    stranded = open_in_closed_sprints(vocab, data["tasks"])
    if stranded:
        result["open_in_closed_sprints"] = stranded
    hint = archive_hint_for(data, sizes["tasks"])
    if hint:
        result["suggest"] = {"archive": hint}
    return {"report": result, "has_sprints": bool(vocab["sprints"])}


# ==== lib/status.py ====
# atm-lib: the status changes (start, review, done, reject, approve, request-changes, block, unblock, reopen, skip,
# fail, release) and notes (docs/layered-architecture.md §5.1).
#
# Each runs as one change of the store (`store.change`, §3.3): the task's status, its claim, and the records shared by
# a clone's worktrees (completed, waiting, resolved) change together or not at all. The rules themselves are still
# those of the modules from before the layers (`change_status`, the lease and resolution helpers); they move into
# atm-lib with their own slices (§9).

# The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing
OWN_LEASE_COMMANDS = ("done", "release", "fail", "review", "block")

# Decisions that may be made in any worktree about a task waiting in another (T-204): the command, and the kinds the
# waiting task must have there
REMOTE_DECISIONS = {"unblock": ("blocked",), "skip": ("blocked",), "done": ("review",), "reject": ("review",)}


def claim_record(work, data, task_id, agent):
    """A new claim on the task, from this working copy (its worktree and branch, T-202)."""
    here = work.here()
    return {
        "task": task_id,
        "agent": agent,
        "branch": here["branch"],
        "worktree": here["worktree"],
        "claimed_at": now_iso(),
        "lease_expires_at": lease_until(data),
    }


def completion_record(work, task_id, agent):
    """Done in this working copy: so `next` doesn't hand the task out again in another before the branches merge."""
    here = work.here()
    return {"task": task_id, "agent": agent, "branch": here["branch"], "worktree": here["worktree"], "completed_at": now_iso()}


def waiting_entry(work, task):
    """The shared record of a task blocked or sent for review in this working copy (T-204)."""
    here = work.here()
    return {
        "task": task["id"],
        "status": task.get("status"),
        "waiting_on": task.get("waiting_on"),
        "questions": list(task.get("questions") or []),
        "worktree": here["worktree"],
        "branch": here["branch"],
        "at": now_iso(),
    }


def adopt_waiting(task, data, store, here, command):
    """When `task` waits in another worktree (blocked or in review there, per the shared store) but not in this one,
    and `command` decides it: give this copy the waiting state, so the command applies here as it would there. Returns
    the worktree it waits in (where the decision is applied too, by apply_resolutions), or None."""
    record = store["waiting"].get(task["id"])
    if command not in REMOTE_DECISIONS or not isinstance(record, dict) or record.get("worktree") == here:
        return None
    vocab = vocabulary(data)
    if kind_of(vocab, task.get("status")) in REMOTE_DECISIONS[command]:
        return None  # it waits here too: decided here as usual
    if kind_of(vocab, record.get("status")) not in REMOTE_DECISIONS[command]:
        return None
    task["status"] = record["status"]
    if record.get("waiting_on"):
        task["waiting_on"] = record["waiting_on"]
    if record.get("questions"):
        task["questions"] = list(record["questions"])
    return record.get("worktree")


def newly_ready(data, task_id):
    """The to-do tasks waiting on `task_id` whose dependencies are now all done."""
    ready = {t.get("id") for t in ready_tasks(data)}
    return [t.get("id") for t in data["tasks"] if task_id in (t.get("dependencies") or []) and t.get("id") in ready]


def change_task_status(project, command, task_id, **options):
    """One status command on a task, as `project.actor`. `options` are the command's: status, note, reason, commit,
    on, questions, answers, resolution, duplicate_of. Returns the task after the change and what goes with it:
    `previous` (its status before), `claim` (its claim now), `unblocked` (after done), `hint` (the archive hint, at
    most once a day), `stranded` (reopened in a closed sprint, T-330) and `expired` (the claims whose lease ran out)."""
    agent = project.actor
    status = options.get("status")
    answers = list(options.get("answers") or [])

    def change(work):
        data, store = work.document, work.claims()
        claims = store["claims"]
        here = work.here()["worktree"]
        expired = expire_claims(None, data, claims)
        # apply_resolutions knows a working copy by its folder, as atm-cli's paths give it
        apply_resolutions({"root": here}, data, store)
        task = find_task(data, task_id)
        if command in OWN_LEASE_COMMANDS:
            own = own_lease_expired(expired, data, task_id, agent, f"atm {command} {task_id}")
            if own:
                work.defer(own)  # the expiry is written first, so `atm show` agrees with the error
                return {"task": task, "expired": expired}
        check_holder(claims, task_id, agent, command)
        duplicate_of = options.get("duplicate_of")
        if duplicate_of and not any(t.get("id") == duplicate_of for t in data["tasks"]) and not archived_task(duplicate_of):
            raise Invalid(f"{task_id} can't be a duplicate of {duplicate_of}, which doesn't exist")
        waits_in = adopt_waiting(task, data, store, here, command)
        previous = change_status(
            task,
            data,
            command,
            reason=options.get("reason"),
            agent=agent,
            status=status,
            note=options.get("note"),
            commit=options.get("commit"),
            waiting_on=options.get("on"),
            questions=list(options.get("questions") or []),
            answers=answers,
            resolution=options.get("resolution"),
            duplicate_of=duplicate_of,
        )
        if command == "start":
            claims[task_id] = claim_record(work, data, task_id, agent)
            store["completed"].pop(task_id, None)  # starting a task again on purpose
        elif command == "reopen":
            claims.pop(task_id, None)
            store["completed"].pop(task_id, None)  # not done any more, in any worktree
        else:
            claims.pop(task_id, None)
        if command == "done":
            store["completed"][task_id] = completion_record(work, task_id, agent)
        # Blocked or in review: recorded for every worktree (T-204); any other change ends the wait
        if command in ("block", "review"):
            store["waiting"][task_id] = waiting_entry(work, task)
        else:
            store["waiting"].pop(task_id, None)
            resolution = store["resolved"].pop(task_id, None)
            if waits_in:
                store["resolved"][task_id] = {
                    "task": task_id,
                    "command": command,
                    "answers": answers,
                    "reason": options.get("reason"),
                    "by": agent,
                    "at": now_iso(),
                    "worktree": waits_in,
                }
            elif resolution and resolution.get("worktree") != here:
                store["resolved"][task_id] = resolution  # still to be applied where it waits
        unblocked = newly_ready(data, task_id) if command == "done" else []
        # The archive hint after done, at most once a day per clone (CLI-5.2), so a loop doesn't repeat it
        hint = archive_hint_for(data, work.size()) if command == "done" else None
        today = now_iso()[:10]
        if hint and store["hints"].get("archive") == today:
            hint = None
        if hint:
            store["hints"]["archive"] = today
        # Reopened in a closed sprint, the task is open where the loop's default selection never looks (T-330)
        stranded = None
        if command == "reopen":
            sprint = find_sprint(vocabulary(data), task.get("sprint"))
            if sprint and sprint["state"] == "closed":
                stranded = f"{task['id']} is now open in {move_out_hint(task['id'], sprint['id'])}"
        return {
            "task": task,
            "previous": previous,
            "claim": claims.get(task_id),
            "unblocked": unblocked,
            "hint": hint,
            "stranded": stranded,
            "expired": expired,
            "document": data,
        }

    return project.store.change(change)


def add_task_note(project, task_id, text):
    """A note on a task, whatever its status (DM-4), at most 500 characters. Returns the task."""

    def change(work):
        task = find_task(work.document, task_id)
        stamp = now_iso()
        add_note(task, "note", text, project.actor, stamp)
        touch(task, project.actor, stamp)
        return task

    return project.store.change(change)


# ==== lib/claims.py ====
# atm-lib: claims and the loop's next task (docs/layered-architecture.md §5.1): `atm next` (with `--claim`, atomic),
# heartbeats, the live claims. Each runs as one change of the store (§3.3); the loop's rule itself (`next_task`,
# `Selection`) is still claims.py's, from before the layers, and moves with a later slice.

# heartbeat --auto renews a claim once its last renewal is this old (CLI-6.2)
AUTO_RENEW_AFTER = timedelta(minutes=10)


def selection(sprints=None, tasks=None, all_tasks=False, backlog=False, task_type=None):
    """What the loop works on: the active sprint by default, named sprints, a list of tasks, all, or the backlog."""
    return Selection(sprints=sprints, tasks=tasks, all_tasks=all_tasks, backlog=backlog, task_type=task_type)


def next_task_for(project, sel, with_user=False, claim=False, status=None):
    """The task the loop takes next for `project.actor` (the dashboard's Next up is the same rule); with `claim`,
    started and claimed in the same change, so no other agent can take it. Returns {task, claim, reason, expired}:
    `reason` says why there is none."""
    agent = project.actor

    def change(work):
        data, store = work.document, work.claims()
        expired = expire_claims(None, data, store["claims"])
        apply_resolutions({"root": work.here()["worktree"]}, data, store)
        task, reason = next_task(data, store, sel=sel, agent=agent, with_user=with_user)
        new = None
        if task and claim:
            change_status(task, data, "start", agent=agent, status=status)
            new = store["claims"][task["id"]] = claim_record(work, data, task["id"], agent)
        return {"task": task, "claim": new, "reason": reason, "expired": expired}

    return project.store.change(change)


def renew_lease(project, task_id, expire=True):
    """Extend the actor's lease on a task (`atm heartbeat`; `atm run` with `expire` false: it renews a live claim and
    lets nothing else expire). Returns {claim, minutes, expired}; raises Claimed when the task has no live claim (its
    lease expired) or another agent holds it."""
    agent = project.actor

    def change(work):
        data, claims = work.document, work.claims()["claims"]
        if not expire:
            claim = claims.get(task_id)
            try:
                alive = claim is not None and parse_iso(claim["lease_expires_at"]) > now()
            except (KeyError, TypeError, ValueError):
                alive = False
            if not alive:
                raise Claimed(f"{task_id} has no live claim")
            check_holder(claims, task_id, agent, "renew the claim on")
            claim["lease_expires_at"] = lease_until(data)
            claim["renewed_at"] = now_iso()
            return {"claim": claim, "minutes": lease_minutes(data), "expired": []}
        expired = expire_claims(None, data, claims)
        claim = claims.get(task_id)
        if not claim:
            own = own_lease_expired(expired, data, task_id, agent, None)
            gone = any(c.get("task") == task_id for c in expired)
            work.defer(own or Claimed(f"{task_id} has no live claim" + (" (its lease expired)" if gone else "")))
            return None
        check_holder(claims, task_id, agent, "heartbeat")
        claim["lease_expires_at"] = lease_until(data)
        claim["renewed_at"] = now_iso()
        return {"claim": claim, "minutes": lease_minutes(data), "expired": expired}

    return project.store.change(change)


def renewed_at(claim):
    try:
        return parse_iso(claim.get("renewed_at") or claim["claimed_at"])
    except (KeyError, TypeError, ValueError):
        return None


def renew_due_claims(project):
    """`heartbeat --auto`: renew the actor's live claims whose last renewal is ten minutes old. Most calls only read
    the claims (no change, nothing written). Returns the claims renewed."""
    agent, current = project.actor, now()

    def due(claims):
        return [
            c
            for c in claims.values()
            if isinstance(c, dict)
            and c.get("agent") == agent
            and parse_iso(c["lease_expires_at"]) > current
            and (renewed_at(c) is None or current - renewed_at(c) >= AUTO_RENEW_AFTER)
        ]

    if not due(project.store.read_claims()["claims"]):
        return []

    def change(work):
        renewed = due(work.claims()["claims"])
        for claim in renewed:
            claim["lease_expires_at"] = lease_until(work.document)
            claim["renewed_at"] = now_iso()
        return renewed

    return project.store.change(change)


def live_claims(project):
    """The live claims, by task, with the expired ones dropped (their tasks back to todo). Returns {claims, expired}."""

    def change(work):
        claims = work.claims()["claims"]
        expired = expire_claims(None, work.document, claims)
        return {"claims": sorted(claims.values(), key=lambda c: c["task"]), "expired": expired}

    return project.store.change(change)


# ==== lib/edit.py ====
# atm-lib: adding and changing tasks (`atm add`, one task or many; `atm edit`) and checking the task data (`atm
# check`), docs/layered-architecture.md §5.1. IDs come from meta.next_id inside the store's change, so two agents adding
# at once never get the same one (docs/v4/data-model.md DM-2.3). Status and timestamps are not edited here: they change
# only through the status commands.

LABEL = re.compile(r"^[a-z0-9_-]{1,32}$")
DEFAULT_ID = ("T-", 3)


def id_format(data):
    """The project's ID pattern: the prefix and digit count of its last numbered task ID (T-001: "T-", 3)."""
    for task in reversed(data["tasks"]):
        parts = split_id(task.get("id"))
        if parts:
            return parts[0], len(parts[1])
    return DEFAULT_ID


def highest_number(data, prefix):
    numbers = [int(p[1]) for p in (split_id(t.get("id")) for t in data["tasks"]) if p and p[0] == prefix]
    return max(numbers, default=0)


def next_number(data):
    """meta.next_id, but never behind the highest ID in the file (tasks added by hand, or by an older atm)."""
    prefix, _ = id_format(data)
    stored = data.get("meta", {}).get("next_id") if isinstance(data.get("meta"), dict) else None
    floor = highest_number(data, prefix) + 1
    return max(stored, floor) if isinstance(stored, int) and not isinstance(stored, bool) else floor


def allocate_ids(data, count):
    """`count` new IDs in the project's pattern; advances meta.next_id."""
    prefix, width = id_format(data)
    first = next_number(data)
    ids = [prefix + str(n).zfill(width) for n in range(first, first + count)]
    if not isinstance(data.get("meta"), dict):
        data["meta"] = {}
    data["meta"]["next_id"] = first + count
    return ids


def check_labels(labels):
    for label in labels:
        if not isinstance(label, str) or not LABEL.match(label):
            raise AtmError(
                f"label {label!r}: use lowercase letters, digits, - and _ (at most 32 characters)", 2
            )
    return labels


def open_sprint(vocab, name):
    """The ID of the sprint called `name`, which a task can be put in: it must exist and not be closed (SR-2.5)."""
    sprint = find_sprint(vocab, name)
    if sprint is None:
        known = ", ".join(s["id"] for s in vocab["sprints"] if s["state"] != "closed") or "none yet: atm sprints add"
        raise AtmError(f"no sprint {name!r}; the open sprints: {known}", 2)
    if sprint["state"] == "closed":
        raise AtmError(f"the sprint {sprint['id']!r} is closed; a task can only go into a planned or active sprint", 2)
    return sprint["id"]


def check_with(value):
    """`with`: who the work is done together with; `user` for now (SR-6.5)."""
    if value != "user":
        raise AtmError(f"with {value!r}: use user (work done together with the user)", 2)
    return value


def check_review(value):
    if value not in REVIEW_MODES:
        raise AtmError(f"review {value!r}: use one of {', '.join(REVIEW_MODES)}", 2)
    return value


def check_related(task_id, related, known, archived=archived_task):
    """`archived(id)`: the archived task with that ID, or None (the store's lookup, from atm-lib)."""
    for other in related:
        if other == task_id:
            raise AtmError(f"{task_id} can't be related to itself", 2)
        if other not in known and not archived(other):
            raise AtmError(f"{task_id} can't be related to {other}, which doesn't exist", 2)
    return related


def new_task(vocab, fields, actor, stamp):
    """A complete new task from the given fields, checked against the vocabularies."""
    # Tasks go into sprints (or the backlog): phases became sprints (SR-5.5, SR-8.3); a "phase" given is ignored
    for key in ("name", "type"):
        if fields.get(key) in (None, ""):
            raise AtmError(f"a new task needs a {key}", 2)
    types = [t["id"] for t in vocab["types"]]
    if fields["type"] not in types:
        raise AtmError(f"unknown type {fields['type']!r}; use one of {', '.join(types)}", 2)
    priorities = [p["id"] for p in vocab["priorities"]]
    if fields.get("priority") is not None and fields["priority"] not in priorities:
        raise AtmError(f"unknown priority {fields['priority']!r}; use one of {', '.join(priorities)}", 2)
    task = {"id": None, "name": str(fields["name"]), "type": fields["type"]}
    if fields.get("priority") is not None:
        task["priority"] = fields["priority"]
    labels = check_labels(list(fields.get("labels") or []))
    if labels:
        task["labels"] = labels
    task.update(
        {
            "description": str(fields.get("description") or ""),
            "requirements": list(fields.get("requirements") or []),
            "use_cases": list(fields.get("use_cases") or []),
            "status": status_for(vocab, "todo"),
            "dependencies": list(fields.get("dependencies") or []),
            "files": list(fields.get("files") or []),
            "created_at": stamp,
            "updated_at": stamp,
        }
    )
    if actor:
        task["created_by"] = actor
        task["updated_by"] = actor
    if fields.get("sprint"):
        set_sprint(task, open_sprint(vocab, fields["sprint"]), stamp)
    if fields.get("release"):
        task["release"] = known_release(vocab, fields["release"])
    # "auto" (--components auto): the components its files are in (SR-2.7)
    requested = fields.get("components") or []
    components = suggest_components(vocab, task["files"]) if requested == "auto" else component_ids(vocab, requested)
    if components:
        task["components"] = components
    if fields.get("review"):
        task["review"] = check_review(fields["review"])
    if fields.get("with"):
        task["with"] = check_with(fields["with"])
    if fields.get("related"):
        task["related"] = list(fields["related"])  # checked once the new IDs are known (add_tasks)
    return task


def find_cycle(tasks):
    """A dependency cycle among `tasks` (as a list of IDs), or None."""
    deps = {t["id"]: t.get("dependencies") or [] for t in tasks}
    done = set()
    # Depth-first with an explicit stack, not recursion: a chain of thousands of tasks (or a hostile file) can't hit
    # Python's recursion limit (docs/security-review.md F12). `path` is the current chain; `on_path` its positions.
    for start in deps:
        if start in done:
            continue
        path, on_path, stack = [start], {start: 0}, [iter(deps[start])]
        while stack:
            dep = next(stack[-1], None)
            if dep is None:
                node = path.pop()
                del on_path[node]
                done.add(node)
                stack.pop()
            elif dep in on_path:
                return path[on_path[dep] :] + [dep]
            elif dep in deps and dep not in done:
                on_path[dep] = len(path)
                path.append(dep)
                stack.append(iter(deps[dep]))
    return None


def add_tasks(data, entries, actor, archived=archived_task):
    """Add `entries` (dicts of task fields; an "id" there is a placeholder other entries' dependencies may use) to
    `data`. Returns the new tasks. Raises AtmError, before changing anything, if one is invalid. `archived(id)` looks
    up an archived task (a dependency on one is fine)."""
    vocab = vocabulary(data)
    stamp = now_iso()
    tasks = [new_task(vocab, e, actor, stamp) for e in entries]
    placeholders = [e.get("id") for e in entries]
    for p in placeholders:
        if p is not None and placeholders.count(p) > 1:
            raise AtmError(f"placeholder ID {p!r} is used twice", 2)
    ids = allocate_ids(data, len(tasks))
    rename = {p: real for p, real in zip(placeholders, ids) if p is not None}
    existing = {t.get("id") for t in data["tasks"]}
    for task, real in zip(tasks, ids):
        task["id"] = real
        task["dependencies"] = [rename.get(d, d) for d in task["dependencies"]]
        # An archived task is done: depending on it is fine, as atm check says (T-356)
        missing = [d for d in task["dependencies"] if d not in existing and d not in ids and not archived(d)]
        if missing:
            raise AtmError(f"{real} ({task['name']}) depends on {', '.join(missing)}, which doesn't exist", 2)
        if task.get("related"):
            related = [rename.get(r, r) for r in task["related"]]
            task["related"] = check_related(real, related, existing | set(ids), archived)
    cycle = find_cycle(data["tasks"] + tasks)
    if cycle:
        raise AtmError("these dependencies would make a cycle: " + " -> ".join(cycle), 2)
    data["tasks"].extend(tasks)
    return tasks


def archived_in(work):
    """A lookup of the archived tasks of a change: the archive is read only when something asks for it."""
    index = {}

    def lookup(task_id):
        if not index:
            index.update({t.get("id"): t for t in work.archive()["tasks"] if isinstance(t, dict)} or {None: None})
        return index.get(task_id)

    return lookup


def add_new_tasks(project, entries):
    """`atm add`: the tasks `entries` describe (dicts of task fields; an "id" there is a placeholder the others'
    dependencies may use), with the next IDs, all or none, as `project.actor`. Returns {tasks, vocab}."""

    def change(work):
        tasks = add_tasks(work.document, entries, project.actor, archived_in(work))
        return {"tasks": tasks, "vocab": vocabulary(work.document)}

    return project.store.change(change)


class Changes:
    """What `atm edit` changes (its options' names: name, description, sprint, backlog, type, priority, release, review,
    with_, components; add_/remove_ file, dep, label, component, related); what isn't given changes nothing. The CLI
    passes its parsed arguments, which have these names."""

    def __init__(self, **given):
        self.__dict__.update(given)

    def __getattr__(self, name):
        return [] if name.startswith(("add_", "remove_")) else None


def edit_existing_task(project, task_id, changes):
    """`atm edit`: change a task's definition (never its status), as `project.actor`. Returns {task, changed, vocab}:
    `changed` the names of the fields that changed (nothing is written when none did)."""

    def change(work):
        task = find_task(work.document, task_id)
        changed = edit_task(work.document, task, changes, archived_in(work))
        if changed:
            touch(task, project.actor, now_iso())
        return {"task": task, "changed": changed, "vocab": vocabulary(work.document)}

    return project.store.change(change)


def edit_task(data, task, args, archived=archived_task):
    """Apply atm edit's options to `task`; returns the names of the fields that changed. Raises AtmError (nothing
    changed yet in the file) when a value is invalid. A description from a file is the caller's to read."""
    vocab = vocabulary(data)
    changed = []

    def set_field(key, value):
        if task.get(key) != value:
            task[key] = value
            changed.append(key)

    def edit_list(key, add, remove):
        current = list(task.get(key) or [])
        updated = [v for v in current if v not in remove] + [v for v in add if v not in current]
        if updated != current:
            task[key] = updated
            changed.append(key)

    if args.name is not None:
        set_field("name", args.name)
    if args.description is not None:
        set_field("description", args.description)
    if args.sprint is not None or args.backlog:
        if set_sprint(task, None if args.backlog else open_sprint(vocab, args.sprint), now_iso()):
            changed.append("sprint")
    if args.type is not None:
        types = [t["id"] for t in vocab["types"]]
        if args.type not in types:
            raise AtmError(f"unknown type {args.type!r}; use one of {', '.join(types)}", 2)
        set_field("type", args.type)
    if args.priority is not None:
        priorities = [p["id"] for p in vocab["priorities"]]
        if args.priority not in priorities:
            raise AtmError(f"unknown priority {args.priority!r}; use one of {', '.join(priorities)}", 2)
        set_field("priority", args.priority)
    edit_list("files", args.add_file, args.remove_file)
    edit_list("labels", check_labels(args.add_label), args.remove_label)
    for key, value, check in (
        ("release", args.release, lambda v: known_release(vocab, v)),
        ("review", args.review, check_review),
        ("with", args.with_, check_with),
    ):
        if value == "":
            if key in task:
                del task[key]
                changed.append(key)
        elif value is not None:
            set_field(key, check(value))
    if args.components == "auto":
        edit_list("components", suggest_components(vocab, task.get("files") or []), [])
    edit_list("components", component_ids(vocab, args.add_component), args.remove_component)
    known = {t.get("id") for t in data["tasks"]}
    edit_list("related", check_related(task["id"], args.add_related, known, archived), args.remove_related)
    for key in ("components", "related"):
        if key in task and not task[key]:
            del task[key]
    ids = {t.get("id") for t in data["tasks"]}
    missing = [d for d in args.add_dep if d not in ids and not archived(d)]
    if missing:
        raise AtmError(f"{task['id']} can't depend on {', '.join(missing)}, which doesn't exist", 2)
    if task["id"] in args.add_dep:
        raise AtmError(f"{task['id']} can't depend on itself", 2)
    edit_list("dependencies", args.add_dep, args.remove_dep)
    cycle = find_cycle(data["tasks"])
    if cycle:
        raise AtmError("these dependencies would make a cycle: " + " -> ".join(cycle), 2)
    return changed


def check_resolution(vocab, task, known_ids, errors):
    """A task's resolution, duplicate_of and related (SR-2.8): a resolution only on a done or skipped task, one of
    RESOLUTIONS; duplicate_of only with the duplicate resolution, naming another task that exists (in either file)."""
    task_id = task["id"]
    resolution = task.get("resolution")
    if "resolution" in task:
        if resolution not in RESOLUTIONS:
            errors.append(f"{task_id}: the resolution {resolution!r} is not one of {', '.join(RESOLUTIONS)}")
        elif kind_of(vocab, task.get("status")) not in ("done", "skipped"):
            errors.append(f"{task_id}: has a resolution ({resolution}) but isn't done or skipped")
    if "duplicate_of" in task:
        other = task["duplicate_of"]
        if resolution != "duplicate":
            errors.append(f"{task_id}: duplicate_of needs the resolution duplicate")
        if other == task_id or other not in known_ids:
            errors.append(f"{task_id}: duplicate_of {other!r} isn't another task")
    related = task.get("related", [])
    if not (isinstance(related, list) and all(isinstance(r, str) for r in related)):
        errors.append(f"{task_id}: 'related' must be a list of task IDs")
    else:
        for other in related:
            if other == task_id or other not in known_ids:
                errors.append(f"{task_id}: related to {other}, which isn't another task")


def check_data(data, root, files=False, archived=()):
    """Problems in a task file: (errors, warnings), each a list of messages. `archived` are the archive's tasks:
    dependencies on them are fine, and an ID in both files is an error. With `files`, also a warning for each file a
    finished task lists that doesn't exist (often just history: files get renamed and deleted later)."""
    errors, warnings = [], []
    archived_ids = {t.get("id") for t in archived if isinstance(t, dict)}
    try:
        vocab = vocabulary(data)
    except AtmError as e:
        errors.append(str(e))
        vocab = vocabulary({})
    tasks = data.get("tasks", [])
    ids = [t.get("id") for t in tasks if isinstance(t, dict)]
    for task_id in sorted({i for i in ids if ids.count(i) > 1}, key=str):
        errors.append(f"{task_id}: the ID is used by {ids.count(task_id)} tasks")
    for task_id in sorted(set(ids) & archived_ids, key=str):
        errors.append(f"{task_id}: in both tasks.json and the archive (atm archive keeps the tasks.json copy)")
    known_ids = set(ids) | archived_ids
    statuses = {s["id"] for s in vocab["statuses"]}
    types = {t["id"] for t in vocab["types"]}
    priorities = {p["id"] for p in vocab["priorities"]}
    for i, task in enumerate(tasks):
        if not isinstance(task, dict) or not isinstance(task.get("id"), str) or not task["id"]:
            errors.append(f"task #{i + 1}: needs an \"id\" (a non-empty string)")
            continue
        task_id = task["id"]
        if not valid_task_id(task_id):
            errors.append(f"{task_id!r}: an ID is letters, digits, '.', '_' and '-', at most 64, not starting with '-'")
        for key, kind in (("name", str), ("type", str), ("status", str)):
            if not isinstance(task.get(key), kind):
                errors.append(f"{task_id}: needs a {key!r} (a string)")
        if isinstance(task.get("status"), str) and kind_of(vocab, task["status"]) is None:
            errors.append(f"{task_id}: the status {task['status']!r} is not one of the project's statuses")
        if isinstance(task.get("type"), str) and task["type"] not in types:
            warnings.append(f"{task_id}: the type {task['type']!r} is not one of the project's types")
        if "priority" in task and task["priority"] not in priorities:
            warnings.append(f"{task_id}: the priority {task['priority']!r} is not one of the project's priorities")
        for key in ("dependencies", "files", "labels", "requirements", "use_cases"):
            if key in task and not (isinstance(task[key], list) and all(isinstance(v, str) for v in task[key])):
                errors.append(f"{task_id}: {key!r} must be a list of strings")
        if "sprint" in task and find_sprint(vocab, task["sprint"]) is None:
            errors.append(f"{task_id}: the sprint {task['sprint']!r} is not one of meta.sprints")
        if "release" in task and find_release(vocab, task["release"]) is None:
            errors.append(f"{task_id}: the release {task['release']!r} is not one of meta.releases")
        if "sprint_history" in task and not (
            isinstance(task["sprint_history"], list)
            and all(isinstance(e, dict) and isinstance(e.get("sprint"), str) for e in task["sprint_history"])
        ):
            errors.append(f'{task_id}: "sprint_history" must be a list of {{"sprint", "from", "to"}}')
        task_files = [f for f in task.get("files") or [] if isinstance(f, str)]
        if vocab["components"] and task_files and not suggest_components(vocab, task_files):
            warnings.append(f"{task_id}: its files match no component (meta.components' paths)")
        check_resolution(vocab, task, known_ids, errors)
        for dep in task.get("dependencies") or []:
            if isinstance(dep, str) and dep not in known_ids:
                errors.append(f"{task_id}: depends on {dep}, which doesn't exist")
        for label in task.get("labels") or []:
            if isinstance(label, str) and not LABEL.match(label):
                errors.append(f"{task_id}: the label {label!r} isn't lowercase letters, digits, - and _")
        # Tasks list the files they will create, so only a finished task's missing files are worth a word
        if files and kind_of(vocab, task.get("status")) == "done":
            for f in task.get("files") or []:
                if isinstance(f, str) and not os.path.exists(os.path.join(root, f)):
                    warnings.append(f"{task_id}: the file {f} doesn't exist")
    for t in open_in_closed_sprints(vocab, [t for t in tasks if isinstance(t, dict) and isinstance(t.get("id"), str)]):
        warnings.append(f"{t['id']}: {t['status']} in {move_out_hint(t['id'], t['sprint'])}")
    for sprint in vocab["sprints"]:
        if sprint.get("release") and find_release(vocab, sprint["release"]) is None:
            errors.append(f"the sprint {sprint['id']!r}: the release {sprint['release']!r} is not one of meta.releases")
    well_formed = [t for t in tasks if isinstance(t, dict) and isinstance(t.get("dependencies", []), list)]
    cycle = find_cycle([{"id": t.get("id"), "dependencies": t.get("dependencies") or []} for t in well_formed])
    if cycle:
        errors.append("dependency cycle: " + " -> ".join(str(c) for c in cycle))
    return errors, warnings


def check_project(project, files=False):
    """`atm check`: what is wrong with the task data as stored. Returns {errors, warnings, count}: `count` the tasks,
    None when the data isn't task data at all. With `files`, a finished task's files that don't exist are warned about
    too (relative to the working copy)."""
    raw = project.store.raw()
    data = raw["document"]
    if data is None:
        return {"errors": ["tasks.json is not a task file (expected an object with a 'tasks' list)"], "warnings": [], "count": None}
    errors_before = [raw["archive_error"]] if raw["archive_error"] else []
    errors, warnings = check_data(data, project.store.working_copy(), files=files, archived=raw["archive"])
    return {"errors": errors_before + errors, "warnings": warnings, "count": len(data["tasks"])}


# ==== lib/components.py ====
# atm-lib: the project's components (docs/sprints-and-releases-design.md §2, SR-2.7; docs/layered-architecture.md
# §5.1): list them, define one, change one; moving a label onto a component, or giving it to the tasks its globs match
# (T-324), changes the archive's tasks too, in the same store change. The rules (which files a component has, the IDs a
# task may name) are common's (common/components.py).

def _tasks(*documents):
    return [t for data in documents for t in data["tasks"] if isinstance(t, dict)]


def list_components(project):
    """meta.components in order, each with how many tasks have it, archived ones too."""
    store = project.store
    tasks = [t for t in store.list_tasks() if isinstance(t, dict)] + store.list_archived()
    return [
        {**c, "tasks": sum(1 for t in tasks if c["id"] in (t.get("components") or []))}
        for c in store.get_vocabularies()["components"]
    ]


def _label_entry(entry):
    """A meta.labels entry's ID: the entry itself when it is a string, else its "id"."""
    return entry if isinstance(entry, str) else entry.get("id") if isinstance(entry, dict) else None


def _label_text(meta, label):
    """The text meta.labels gives `label` (e.g. "Dashboard" for dashboard), or None."""
    entries = meta.get("labels") if isinstance(meta.get("labels"), list) else []
    entry = next((e for e in entries if _label_entry(e) == label), None)
    return entry["label"] if isinstance(entry, dict) and isinstance(entry.get("label"), str) else None


def _check_options(globs, review):
    if any(not glob for glob in globs):
        raise Invalid("--path needs a glob, e.g. 'dashboard/**'")
    if review and review not in REVIEW_MODES:
        raise Invalid(f"--review {review!r}: use one of {', '.join(REVIEW_MODES)}")


def _move_and_assign(data, archive_data, component, label, assign):
    """On every task of both files: `label` (if given) becomes the component, and with `assign` a task whose files
    the component's globs match gets it too; `label` leaves meta.labels. Returns the task IDs from the label and those
    by their files. A label no task has and meta.labels doesn't define (most likely a typo) is refused before anything
    changes."""
    meta = data["meta"]
    known_label = any(_label_entry(e) == label for e in meta.get("labels") or [])
    if label and not known_label and not any(label in (t.get("labels") or []) for t in _tasks(data, archive_data)):
        raise Invalid(f"no task has the label {label!r}")
    from_label, by_files = [], []
    for task in _tasks(data, archive_data):
        labels = task.get("labels") if isinstance(task.get("labels"), list) else []
        moved = bool(label) and label in labels
        current = task.get("components") if isinstance(task.get("components"), list) else []
        matched = (
            assign
            and component["id"] not in current
            and any(matches(component, f) for f in task.get("files") or [] if isinstance(f, str))
        )
        if not (moved or matched):
            continue
        if moved:
            task["labels"] = [x for x in labels if x != label]
            if not task["labels"]:
                del task["labels"]
        if component["id"] not in current:
            task["components"] = current + [component["id"]]
        (from_label if moved else by_files).append(task["id"])
    if label and isinstance(meta.get("labels"), list):
        meta["labels"] = [e for e in meta["labels"] if _label_entry(e) != label]
        if not meta["labels"]:
            del meta["labels"]
    return from_label, by_files


def add_component(project, component_id, label=None, paths=(), review=None, from_label=None, assign=False):
    """Define a component; `from_label` moves that label onto it on every task (both files) and drops it from
    meta.labels; `assign` gives it to every task whose files its globs match. One change. Returns {component,
    from_label, by_files}."""
    if not RELEASE_ID.fullmatch(component_id):
        raise Invalid(f"{component_id!r}: a component ID is letters, digits, '.', '-' and '_' (at most 64)")
    globs = list(paths or [])
    _check_options(globs, review)
    if assign and not globs:
        raise Invalid("--assign gives the component to the tasks its globs match: it needs --path")

    def change(work):
        data = work.document
        if any(c["id"] == component_id for c in vocabulary(data)["components"]):
            raise Invalid(f"there is already a component {component_id!r} (atm components edit changes it)")
        meta = data.setdefault("meta", {})
        component = {"id": component_id}
        # The name shown: the label given, else the label's own text (meta.labels), else none
        label_text = label or (_label_text(meta, from_label) if from_label else None)
        if label_text:
            component["label"] = label_text
        component["paths"] = globs
        if review:
            component["review"] = review
        meta["components"] = list(meta.get("components") or []) + [component]
        moved, by_files = _move_and_assign(data, work.archive(), component, from_label, assign)
        vocabulary(data)  # checks meta.components, as atm init would
        return {"component": component, "from_label": moved, "by_files": by_files}

    return project.store.change(change)


def edit_component(project, component_id, label=None, add_paths=(), review=None, from_label=None, assign=False):
    """Change a component: `label` and `review` (empty: remove them), `add_paths`; `from_label` and `assign` as for
    add, the globs added included. One change. Returns {component, changed, from_label, by_files}."""
    globs = list(add_paths or [])
    _check_options(globs, review)

    def change(work):
        data = work.document
        known = [c["id"] for c in vocabulary(data)["components"]]
        if component_id not in known:
            raise NotFound(f"no component {component_id!r}; the components: {', '.join(known) or 'none yet'}", 2)
        component = next(c for c in data["meta"]["components"] if c.get("id") == component_id)
        changed = []
        for key, value in (("label", label), ("review", review)):
            if value == "" and key in component:
                del component[key]
                changed.append(key)
            elif value and component.get(key) != value:
                component[key] = value
                changed.append(key)
        added = [g for g in dict.fromkeys(globs) if g not in (component.get("paths") or [])]
        if added:
            component["paths"] = list(component.get("paths") or []) + added
            changed.append("paths")
        if assign and not component.get("paths"):
            raise Invalid(f"--assign gives {component_id} to the tasks its globs match, and it has none: add --add-path")
        moved, by_files = _move_and_assign(data, work.archive(), component, from_label, assign)
        vocabulary(data)  # checks meta.components, as atm init would
        return {"component": component, "changed": changed, "from_label": moved, "by_files": by_files}

    return project.store.change(change)


# ==== lib/sprints.py ====
# atm-lib: sprints and the backlog (docs/sprints-and-releases-design.md §3.1; docs/layered-architecture.md §5.1):
# list, add, edit, start, close and delete sprints (meta.sprints), move tasks between sprints and the backlog, list the
# backlog. Each write is one store change; a task's sprint changes only through set_sprint, which keeps its history.
#
# The options are named as atm's command line names them (goal, release, due, after, carry_to, new_name, reviewed,
# move_to, force): the CLI passes its parsed arguments; any object with those attributes does.

def sprint_overview(data, archived, include_closed):
    """The sprints in order, each with its task counts by kind (tasks.json and the archive): {..., tasks, by_kind}."""
    vocab = vocabulary(data)
    counts = {s["id"]: {kind: 0 for kind in KINDS} for s in vocab["sprints"]}
    pending = {s["id"]: 0 for s in vocab["sprints"]}
    for task in list(data["tasks"]) + list(archived):
        sprint = find_sprint(vocab, task.get("sprint")) if isinstance(task, dict) else None
        kind = kind_of(vocab, task.get("status")) if sprint else None
        if kind in KINDS:
            counts[sprint["id"]][kind] += 1
        if sprint and task.get("sprint_review") == "pending":
            pending[sprint["id"]] += 1
    return [
        {
            **s,
            "tasks": sum(counts[s["id"]].values()),
            "by_kind": {kind: n for kind, n in counts[s["id"]].items() if n},
            "review_pending": pending[s["id"]],
        }
        for s in vocab["sprints"]
        if include_closed or s["state"] != "closed"
    ]


def taken_names(data, archived):
    """Every sprint name in use: the sprints, and the sprints tasks in either file name (compared case-insensitively
    by the caller)."""
    names = [s.get("id") for s in data.get("meta", {}).get("sprints") or [] if isinstance(s, dict)]
    for task in list(data["tasks"]) + list(archived):
        if isinstance(task, dict):
            names.append(task.get("sprint"))
            names.extend(e.get("sprint") for e in task.get("sprint_history") or [] if isinstance(e, dict))
    return [n for n in names if isinstance(n, str)]


def check_due(due):
    if due and not DATE.fullmatch(due):
        raise Invalid(f"--due {due!r}: use a date, YYYY-MM-DD")
    return due


def apply_fields(sprint, options, vocab):
    """goal, release and due on a sprint; an empty value removes the field; a release must be one of meta.releases
    (SR-2.2). Returns the names changed."""
    changed = []
    release = known_release(vocab, options.release) if options.release else options.release
    for key, value in (("goal", options.goal), ("release", release), ("due", check_due(options.due))):
        if value is None:
            continue
        if value == "":
            if key in sprint:
                del sprint[key]
                changed.append(key)
        elif sprint.get(key) != value:
            sprint[key] = value
            changed.append(key)
    return changed


def in_backlog(vocab, task, finished=False):
    """A task with no sprint; a finished one only with `finished`."""
    return not task.get("sprint") and (finished or kind_of(vocab, task.get("status")) not in ("done", "skipped"))


def list_sprints(project, include_closed=False):
    """The sprints in order with their task counts (the closed ones too with `include_closed`), and how many tasks
    are in the backlog. Returns {sprints, backlog, has_sprints}."""
    store = project.store
    data = store.document()
    vocab = vocabulary(data)
    return {
        "sprints": sprint_overview(data, store.list_archived(), include_closed),
        "backlog": len([t for t in data["tasks"] if in_backlog(vocab, t)]),
        "has_sprints": bool(vocab["sprints"]),
    }


def backlog_tasks(project, finished=False):
    """The tasks with no sprint (the finished ones too with `finished`). Returns {tasks, vocab}."""
    data = project.store.document()
    vocab = vocabulary(data)
    return {"tasks": [t for t in data["tasks"] if in_backlog(vocab, t, finished=finished)], "vocab": vocab}


def add_sprint(project, options):
    """A planned sprint, named `options.name` (generated without one), after `options.after` (default: last), with
    its goal, release and due date. Returns the sprint."""

    def change(work):
        data = work.document
        vocab = vocabulary(data)
        name = (options.name or "").strip() or generate_sprint_name(
            vocab["sprint_names"], taken_names(data, work.archive()["tasks"])
        )
        if find_sprint(vocab, name):
            raise Invalid(f"there is already a sprint {find_sprint(vocab, name)['id']!r}")
        if len(name) > MAX_SPRINT_NAME:
            raise Invalid(f"a sprint name is at most {MAX_SPRINT_NAME} characters")
        sprint = {"id": name, "state": "planned"}
        apply_fields(sprint, options, vocab)
        meta = data.setdefault("meta", {})
        sprints = list(meta.get("sprints") or [])
        position = len(sprints)
        if options.after:
            after = find_sprint(vocab, options.after)
            if after is None:
                raise NotFound(f"no sprint {options.after!r} to add it after", 2)
            position = next(i for i, s in enumerate(sprints) if s.get("id") == after["id"]) + 1
        sprints.insert(position, sprint)
        meta["sprints"] = sprints
        vocabulary(data)
        return sprint

    return project.store.change(change)


def stored_sprint(data, name):
    """The sprint called `name` as stored in meta.sprints (to change it), or an error naming the sprints."""
    vocab = vocabulary(data)
    found = find_sprint(vocab, name)
    if found is None:
        known = ", ".join(s["id"] for s in vocab["sprints"]) or "none yet: atm sprints add"
        raise NotFound(f"no sprint {name!r}; the sprints: {known}", 2)
    return next(s for s in data["meta"]["sprints"] if s.get("id") == found["id"])


def edit_sprint(project, name, options):
    """Change a sprint's goal, release or due date (an empty value removes it). Returns {sprint, changed}."""

    def change(work):
        sprint = stored_sprint(work.document, name)
        changed = apply_fields(sprint, options, vocabulary(work.document))
        if changed:
            vocabulary(work.document)
        return {"sprint": sprint, "changed": changed}

    return project.store.change(change)


def start_sprint(project, name):
    """Make a planned sprint the active one (one at a time). Returns the sprint."""

    def change(work):
        data = work.document
        sprint = stored_sprint(data, name)
        state = sprint.get("state", "planned")
        if state == "closed":
            raise Invalid(f"the sprint {sprint['id']!r} is closed; closed sprints aren't started again")
        if state == "active":
            raise Invalid(f"the sprint {sprint['id']!r} is already active")
        active = [s["id"] for s in vocabulary(data)["sprints"] if s["state"] == "active"]
        if active:
            raise Invalid(f"the sprint {active[0]!r} is active; close it first (one active sprint at a time)")
        sprint["state"] = "active"
        sprint["start"] = now_iso()
        return sprint

    return project.store.change(change)


def in_sprint(task, sprint):
    """Whether the task is in the sprint (names compare case-insensitively)."""
    return (
        isinstance(task, dict)
        and isinstance(task.get("sprint"), str)
        and task["sprint"].casefold() == sprint["id"].casefold()
    )


def added_mid_sprint(task, sprint):
    """Whether the task joined the sprint after it started: its history entry for the sprint (the last one) begins
    after the sprint's start (SR-2.5)."""
    entries = [
        e
        for e in task.get("sprint_history") or []
        if isinstance(e, dict) and isinstance(e.get("sprint"), str) and e["sprint"].casefold() == sprint["id"].casefold()
    ]
    start = sprint.get("start")
    return bool(entries and start and isinstance(entries[-1].get("from"), str) and entries[-1]["from"] > start)


def carry_target(data, vocab, sprint, options, taken):
    """Where carry_to sends the unfinished tasks: a sprint ID, None for the backlog, or a new planned sprint (added
    to meta.sprints after the one closing). Returns (target, created sprint or None)."""
    to = options.carry_to
    if options.new_name and to != "new":
        raise Invalid("--name names a new sprint: use it with --carry-to new")
    if to == "backlog":
        return None, None
    if to == "new":
        name = (options.new_name or "").strip() or generate_sprint_name(vocab["sprint_names"], taken)
        if find_sprint(vocab, name):
            raise Invalid(f"there is already a sprint {find_sprint(vocab, name)['id']!r}")
        if len(name) > MAX_SPRINT_NAME:
            raise Invalid(f"a sprint name is at most {MAX_SPRINT_NAME} characters")
        created = {"id": name, "state": "planned"}
        sprints = data["meta"]["sprints"]
        sprints.insert(next(i for i, s in enumerate(sprints) if s.get("id") == sprint["id"]) + 1, created)
        return name, created
    target = open_sprint(vocab, to)
    if target == sprint["id"]:
        raise Invalid(f"the tasks can't be carried to the sprint that is closing ({sprint['id']!r})")
    return target, None


def close_sprint(project, name, options):
    """Close a sprint (SR-4.3): its unfinished tasks go to options.carry_to, keeping their status and history (SR-4.5);
    the sprint gets `end` and its report {done, carried, added, to}; with options.reviewed its batch tasks are marked
    reviewed (SR-4.4). Returns {sprint, created, target, was_active}."""
    actor = project.actor

    def change(work):
        data = work.document
        archived = work.archive()["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, name)
        if sprint.get("state") == "closed":
            raise Invalid(f"the sprint {sprint['id']!r} is already closed")
        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        own_archived = [t for t in archived if in_sprint(t, sprint)]

        def kind(task):
            return kind_of(vocab, task.get("status"))

        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise Invalid(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before closing the sprint"
            )
        # Batch tasks wait for the end-of-sprint review (SR-4.4): /atm:close-sprint reviews them, then --reviewed
        pending = [t for t in own if t.get("sprint_review") == "pending"]
        if pending and not options.reviewed:
            ids = ", ".join(t["id"] for t in pending)
            raise Invalid(
                f"{ids} {'waits' if len(pending) == 1 else 'wait'} for the end-of-sprint review: review "
                f"{'it' if len(pending) == 1 else 'them'} with the user (/atm:close-sprint), then close with --reviewed"
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        if unfinished and not options.carry_to:
            ids = ", ".join(t["id"] for t in unfinished)
            raise Invalid(
                f"{ids} {'is' if len(unfinished) == 1 else 'are'}n't finished: say where "
                f"{'it goes' if len(unfinished) == 1 else 'they go'} with --carry-to <sprint>, new or backlog"
            )
        target, created = (None, None)
        if unfinished:
            target, created = carry_target(data, vocab, sprint, options, taken_names(data, archived))
        stamp = now_iso()
        report = {
            "done": [t["id"] for t in own + own_archived if kind(t) == "done"],
            "carried": [t["id"] for t in unfinished],
            "added": [t["id"] for t in own + own_archived if added_mid_sprint(t, sprint)],
        }
        if unfinished:
            report["to"] = target or "backlog"
        for task in unfinished:
            set_sprint(task, target, stamp)
            touch(task, actor, stamp)
        for task in pending:
            task["sprint_review"] = "reviewed"
            touch(task, actor, stamp)
        was_active = sprint.get("state") == "active"
        sprint["state"] = "closed"
        sprint["end"] = stamp
        sprint["report"] = report
        vocabulary(data)
        return {"sprint": sprint, "created": created, "target": target, "was_active": was_active}

    return project.store.change(change)


def mentions_sprint(task, sprint):
    """Whether the task's sprint history names the sprint (case-insensitively)."""
    return any(
        isinstance(e, dict) and isinstance(e.get("sprint"), str) and e["sprint"].casefold() == sprint["id"].casefold()
        for e in task.get("sprint_history") or []
    )


def delete_sprint(project, name, options):
    """Delete a sprint created by mistake (T-339). An empty one (no task in it, none ever) goes at once, whatever its
    state; an open one's unfinished tasks go to options.move_to (the backlog by default); one that tasks finished in
    or moved on from needs options.force when open, and a closed one with any history stays (its record and report).
    Returns {sprint, state, moved, target}."""
    actor = project.actor

    def change(work):
        data = work.document
        archived = work.archive()["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, name)
        state = sprint.get("state", "planned")

        def kind(task):
            return kind_of(vocab, task.get("status"))

        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise Invalid(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before deleting the sprint"
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        # Its history: tasks that finished in it (archived ones too), or were in it and moved on
        history = [t["id"] for t in own if t not in unfinished]
        history += [t.get("id") for t in archived if isinstance(t, dict) and in_sprint(t, sprint)]
        history += [
            t.get("id")
            for t in list(data["tasks"]) + list(archived)
            if isinstance(t, dict) and not in_sprint(t, sprint) and mentions_sprint(t, sprint)
        ]
        if history and state == "closed":
            raise Invalid(
                f"the sprint {sprint['id']!r} is closed and had tasks ({', '.join(history)}): its record and report "
                "stay; only an empty sprint can be deleted"
            )
        if history and not options.force:
            raise Invalid(
                f"tasks finished in the sprint {sprint['id']!r} or moved on from it ({', '.join(history)}): delete it "
                "with --force to drop that record (its tasks go to --move-to, the backlog by default)"
            )
        to = options.move_to or "backlog"
        target = None if to == "backlog" else open_sprint(vocab, to)
        if target is not None and target.casefold() == sprint["id"].casefold():
            raise Invalid(f"the tasks can't move to the sprint being deleted ({sprint['id']!r})")
        # With --force, the finished tasks still in it move too, so no task names a sprint that is gone
        moving = own if options.force else unfinished
        stamp = now_iso()
        for task in moving:
            set_sprint(task, target, stamp)
            touch(task, actor, stamp)
        data["meta"]["sprints"] = [s for s in data["meta"]["sprints"] if s is not sprint]
        vocabulary(data)
        return {"sprint": sprint, "state": state, "moved": [t["id"] for t in moving], "target": target}

    return project.store.change(change)


def move_target(vocab, task, to):
    """The sprint ID (or None, the backlog) `atm move --to` means for `task`."""
    if to == "backlog":
        return None
    if to == "active":
        active = [s["id"] for s in vocab["sprints"] if s["state"] == "active"]
        if not active:
            raise Invalid("no sprint is active (atm sprints start <name>)")
        return active[0]
    if to == "next":
        # The first planned sprint after the task's own (for a task in the backlog or a closed sprint: the first
        # planned sprint)
        sprints = vocab["sprints"]
        current = find_sprint(vocab, task.get("sprint"))
        start = sprints.index(current) + 1 if current else 0
        following = [s["id"] for s in sprints[start:] if s["state"] == "planned"]
        if not following:
            raise Invalid(
                f"{task['id']}: no planned sprint after {current['id'] if current else 'the active one'}; "
                'add one with atm sprints add ["<name>"]'
            )
        return following[0]
    return open_sprint(vocab, to)


def move_tasks(project, ids, to):
    """Move to-do, blocked or in-review tasks into a sprint, the next planned one, the active one, or the backlog
    (claimed and finished tasks stay where they are), all or none. Returns [{id, from, to}]."""
    actor = project.actor

    def change(work):
        data = work.document
        vocab = vocabulary(data)
        plans = []
        for task_id in ids:
            task = find_task(data, task_id)
            kind = kind_of(vocab, task.get("status"))
            if kind == "active":
                raise Invalid(f"{task_id} is in progress (claimed): it stays where the work is happening")
            if kind in ("done", "skipped"):
                raise Invalid(f"{task_id} is {task.get('status')}: finished work stays in the sprint it was done in")
            plans.append((task, move_target(vocab, task, to)))
        stamp = now_iso()
        moved = []
        for task, target in plans:
            before = task.get("sprint")
            if set_sprint(task, target, stamp):
                touch(task, actor, stamp)
            moved.append({"id": task["id"], "from": before, "to": target})
        return moved

    return project.store.change(change)


# ==== lib/releases.py ====
# atm-lib: releases (docs/sprints-and-releases-design.md §3.3, SR-2.2; docs/layered-architecture.md §5.1): list them
# with their tasks by kind and stage, add one (with PRs, giving it their tasks, archived ones too), edit and archive one,
# and draft its notes. A task's release is its own, else its sprint's. Each write is one store change.
#
# The options are named as atm's command line names them (title for --name, target, pr, released_at).

STAGES = ("done", "in_pr", "merged")


def stage_of(vocab, task):
    """A done task's stage (DM-4.3): merged once its PR merged, in_pr while its PR is open, else done; None when it
    isn't done."""
    if kind_of(vocab, task.get("status")) != "done":
        return None
    pr = task.get("pr") if isinstance(task.get("pr"), dict) else None
    if pr and task.get("merged_at"):
        return "merged"
    return "in_pr" if pr else "done"


def release_overview(data, archived, include_archived):
    """The releases in order, each with its tasks (by effective release, both files) by kind and by stage."""
    vocab = vocabulary(data)
    counts = {r["id"]: ({k: 0 for k in KINDS}, {s: 0 for s in STAGES}) for r in vocab["releases"]}
    for task in list(data["tasks"]) + list(archived):
        if not isinstance(task, dict):
            continue
        release = effective_release(vocab, task)
        if release not in counts:
            continue
        by_kind, by_stage = counts[release]
        kind = kind_of(vocab, task.get("status"))
        if kind in by_kind:
            by_kind[kind] += 1
        stage = stage_of(vocab, task)
        if stage:
            by_stage[stage] += 1
    return [
        {
            **r,
            "tasks": sum(counts[r["id"]][0].values()),
            "by_kind": {k: n for k, n in counts[r["id"]][0].items() if n},
            "by_stage": {s: n for s, n in counts[r["id"]][1].items() if n},
        }
        for r in vocab["releases"]
        if include_archived or r["state"] != "archived"
    ]


def list_releases(project, include_archived=False):
    """The releases in order with their tasks by kind and stage. Returns {releases, has_releases}."""
    data = project.store.document()
    return {
        "releases": release_overview(data, project.store.list_archived(), include_archived),
        "has_releases": bool(vocabulary(data)["releases"]),
    }


def check_target(target):
    if target and not DATE.fullmatch(target):
        raise Invalid(f"--target {target!r}: use a date, YYYY-MM-DD")
    return target


def release_fields(release, options):
    """name (options.title) and target on a release; an empty value removes the field. Returns the names changed."""
    changed = []
    for key, value in (("name", options.title), ("target", check_target(options.target))):
        if value is None:
            continue
        if value == "":
            if key in release:
                del release[key]
                changed.append(key)
        elif release.get(key) != value:
            release[key] = value
            changed.append(key)
    return changed


def unknown_release(vocab, release_id):
    known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm releases add <R>"
    return NotFound(f"no release {release_id!r}; the releases: {known}", 2)


def stored_release(data, release_id):
    """The release as stored in meta.releases (to change it), or an error naming the releases."""
    vocab = vocabulary(data)
    if find_release(vocab, release_id) is None:
        raise unknown_release(vocab, release_id)
    return next(r for r in data["meta"]["releases"] if r.get("id") == release_id)


def add_release(project, release_id, options):
    """A planned release; with options.pr (pull request numbers) every task of those PRs, in both files, goes into
    it; with options.released_at it shipped already. Returns {release, tasks}: the tasks given to it."""
    if not RELEASE_ID.fullmatch(release_id):
        raise Invalid(f"{release_id!r}: a release ID is letters, digits, '.', '-' and '_' (at most 64)")
    prs = options.pr or []
    if any(n <= 0 for n in prs):
        raise Invalid("--pr needs a pull request number")
    if options.released_at is not None:
        try:
            parse_iso(options.released_at)
        except ValueError:
            raise Invalid(f"--released-at needs a time like 2026-10-24T18:00:00Z, not {options.released_at!r}")

    def change(work):
        data, archive_data = work.document, work.archive()
        vocab = vocabulary(data)
        if find_release(vocab, release_id):
            raise Invalid(f"there is already a release {release_id!r}")
        release = {"id": release_id, "state": "planned"}
        release_fields(release, options)
        given = []
        # A release recovered from pull requests (SR-8.2): their tasks, in both files, go into it; the release records
        # the last PR given (the one that shipped it)
        for number in prs:
            own = []
            for task in list(data["tasks"]) + list(archive_data["tasks"]):
                pr = task.get("pr") if isinstance(task, dict) and isinstance(task.get("pr"), dict) else None
                if pr and pr.get("number") == number:
                    task["release"] = release_id
                    own.append(task["id"])
                    if isinstance(pr.get("url"), str):
                        release["pr"] = {"number": number, "url": pr["url"]}
            if not own:
                raise Invalid(f"no task is in pull request #{number}")
            given += own
        if options.released_at is not None:
            # One that shipped already: released at that time (its PR's merge, for one recovered from PRs)
            release["state"] = "released"
            release["released_at"] = options.released_at
        meta = data.setdefault("meta", {})
        meta["releases"] = list(meta.get("releases") or []) + [release]
        vocabulary(data)
        return {"release": release, "tasks": given}

    return project.store.change(change)


def edit_release(project, release_id, options):
    """Change a release's name or target (an empty value removes it). Returns {release, changed}."""

    def change(work):
        release = stored_release(work.document, release_id)
        changed = release_fields(release, options)
        if changed:
            vocabulary(work.document)
        return {"release": release, "changed": changed}

    return project.store.change(change)


def archive_release(project, release_id):
    """Archive a release: it leaves the open releases (`atm releases --all` shows it). Returns the release."""

    def change(work):
        release = stored_release(work.document, release_id)
        if release.get("state") == "archived":
            raise Invalid(f"the release {release['id']!r} is already archived")
        release["state"] = "archived"
        return release

    return project.store.change(change)


def outcome_of(task):
    """The task's last outcome note (what `atm done --note` said), or None."""
    notes = [n for n in task.get("notes") or [] if isinstance(n, dict) and n.get("kind") == "outcome"]
    return notes[-1].get("text") if notes else None


def release_notes(project, release_id):
    """A draft of the release's changes (§3.3): its done tasks (by effective release, both files) grouped by type in
    the project's order, each with its ID, name and outcome note; and the tasks in it not done yet. Returns
    {release, groups, open}."""
    data = project.store.document()
    vocab = vocabulary(data)
    release = find_release(vocab, release_id)
    if release is None:
        raise unknown_release(vocab, release_id)
    tasks = [t for t in list(data["tasks"]) + project.store.list_archived() if isinstance(t, dict)]
    own = [t for t in tasks if effective_release(vocab, t) == release_id]
    done = [t for t in own if kind_of(vocab, t.get("status")) == "done"]
    types = [(t["id"], t["label"]) for t in vocab["types"]]
    types += [(t, t) for t in dict.fromkeys(x.get("type") for x in done) if t not in dict(types)]
    groups = []
    for type_id, label in types:
        items = [{"id": t.get("id"), "name": t.get("name"), "outcome": outcome_of(t)} for t in done if t.get("type") == type_id]
        if items:
            groups.append({"type": type_id, "label": label, "tasks": items})
    open_tasks = [
        {"id": t.get("id"), "name": t.get("name"), "status": t.get("status")}
        for t in own
        if kind_of(vocab, t.get("status")) not in ("done", "skipped")
    ]
    return {"release": release, "groups": groups, "open": open_tasks}


# ==== lib/archive.py ====
# atm-lib: the archive (docs/v4/data-model.md §5; docs/layered-architecture.md §5.1): finished tasks moved out of the
# working tasks into the archive, so the working file stays small, and brought back.
#
# A move is one store change that tells the store it moves tasks into the archive (`work.moving_into_archive()`): the
# JSON-in-git store writes the archive first, then tasks.json; a restore the other way round. A task found in both
# keeps its working copy and loses the archive's: an archive that crashed halfway simply hasn't happened, an unarchive
# that crashed halfway is complete, and no task is ever lost.

def drop_duplicates(data, archive):
    """Tasks in both files keep their tasks.json copy (see the module docstring). Returns the dropped IDs."""
    ids = {t.get("id") for t in data["tasks"]}
    dropped = [t.get("id") for t in archive["tasks"] if t.get("id") in ids]
    archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
    return dropped


def kept_finished(data, moving, before, keep, sprints):
    """The finished tasks that stay in tasks.json, and why (T-352), as archivable() decides: in an open PR, finished
    in the last keep days, not before --before, not in the --sprint chosen. [{id, reason}], in file order."""
    vocab = vocabulary(data)
    days = archive_settings(data)["keep_days"] if keep is None else keep
    recent = now() - timedelta(days=days)
    moved = {t.get("id") for t in moving}
    kept = []
    for t in data["tasks"]:
        if not isinstance(t, dict) or t.get("id") in moved:
            continue
        if kind_of(vocab, t.get("status")) not in ("done", "skipped"):
            continue
        finished = finished_at(t)
        if isinstance(t.get("pr"), dict) and not t.get("merged_at"):
            reason = f"in open PR #{t['pr'].get('number')}"
        elif finished is not None and finished > recent:
            reason = f"finished in the last {days} days"
        elif before is not None and (finished is None or finished >= before):
            reason = f"finished on or after {before.date().isoformat()}"
        elif sprints is not None:
            reason = "not in the sprints chosen"
        else:
            continue
        kept.append({"id": t.get("id"), "reason": reason})
    return kept


def parse_before(before):
    """--before as a time: a date (2026-05-01) or a full ISO time; None without one."""
    if not before:
        return None
    try:
        return parse_iso(before if "T" in before else before + "T00:00:00Z")
    except ValueError:
        raise Invalid(f"--before {before}: use a date like 2026-05-01")


def plan_archive(data, archive, sprints, before, keep_days):
    """What an archive would do: (moving, dropped, kept, names). Changes `archive` (drops its duplicates)."""
    dropped = drop_duplicates(data, archive)
    moving = archivable(data, before, keep_days)
    names = None
    if sprints:
        vocab = vocabulary(data)
        names = set()
        for name in sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise NotFound(f"no sprint {name!r}", 2)
            names.add(sprint["id"])
        moving = [t for t in moving if (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names]
    return moving, dropped, kept_finished(data, moving, before, keep_days, names)


def archive_finished(project, sprints=None, before=None, keep_days=None, dry_run=False):
    """`atm archive`: move the finished tasks (DM-5.1) to the archive, those of `sprints` only and finished before
    `before` if given, keeping those finished in the last `keep_days`. With `dry_run`, only what it would do. Returns
    {moved, dry_run, duplicates_dropped, kept, sizes}."""
    before = parse_before(before)
    if dry_run:
        # Nothing changes: worked out on copies of what the store reads
        data = {"meta": project.store.document().get("meta"), "tasks": list(project.store.list_tasks())}
        archive = {"meta": {}, "tasks": list(project.store.list_archived())}
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        return {"moved": ids, "dry_run": True, "duplicates_dropped": dropped, "kept": kept, "sizes": project.store.sizes()}

    def change(work):
        data, archive = work.document, work.archive()
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        if moving or dropped:
            work.moving_into_archive()
            # New IDs must never come back to an archived ID (DM-2.3)
            if not isinstance(data.get("meta"), dict):
                data["meta"] = {}
            data["meta"]["next_id"] = next_number(data)
            archive.setdefault("meta", {})["archived_at"] = now_iso()
            archive["tasks"].extend(moving)
            moved = set(ids)
            data["tasks"] = [t for t in data["tasks"] if t.get("id") not in moved]
            # The dashboard shows the count without loading the archive (dashboard D-5.4)
            data["meta"]["archived"] = len(archive["tasks"])
        return {"moved": ids, "dry_run": False, "duplicates_dropped": dropped, "kept": kept}

    result = project.store.change(change)
    return {**result, "sizes": project.store.sizes()}


def unarchive_tasks(project, ids):
    """`atm unarchive`: bring archived tasks back. Returns {restored, moves_back}: those the next archive would move
    straight back (finished and old, T-357)."""

    def change(work):
        data, archive = work.document, work.archive()
        drop_duplicates(data, archive)
        by_id = {t.get("id"): t for t in archive["tasks"]}
        missing = [i for i in ids if i not in by_id]
        if missing:
            # Already back, or never there at all: two different mistakes (T-357)
            live = {t.get("id") for t in data["tasks"]}
            here = [i for i in missing if i in live]
            unknown = [i for i in missing if i not in live]
            parts = ([f"{', '.join(here)} (already in tasks.json)"] if here else []) + (
                [f"{', '.join(unknown)} (no such task)"] if unknown else []
            )
            raise NotFound(f"not in the archive: {'; '.join(parts)}")
        data["tasks"].extend(by_id[i] for i in ids)
        # Still finished and old: the next atm archive would move it straight back (T-357)
        again = [t.get("id") for t in archivable(data, None, None) if t.get("id") in ids]
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        data["meta"]["archived"] = len(archive["tasks"]) - len(set(ids))
        archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
        return {"restored": list(ids), "moves_back": again}

    return project.store.change(change)


def archive_hint_now(project):
    """The suggestion to archive (CLI-5.1), or None: when the working tasks are over the threshold."""
    return archive_hint_for(project.store.document(), project.store.sizes()["tasks"])


# ==== lib/code.py ====
# atm-lib: the project's workflow (meta.workflow: direct or pull-request, docs/v4/skills-and-workflows.md §1) and the
# code side of tasks: committing a task's files and recording the commit, the PR that contains tasks, its merge
# (docs/v4/atm-cli.md §4.5, data-model.md §4; docs/layered-architecture.md §2, §5.1).
#
# The commit goes through the project's version control (`project.vcs`); the files that travel with a code commit
# (tasks.json, for the JSON-in-git store) are the store's to say (`store.files_for_commit()`).

HTTP_URL = re.compile(r"https?://[^\s]+\Z", re.IGNORECASE)
CO_AUTHORED = re.compile(r"^co-authored-by:", re.IGNORECASE)


def workflow_settings(project):
    """The project's workflow (meta.workflow with its defaults), or None when none is set."""
    return workflow_of(project.store.document())


def set_workflow(project, workflow_type, base=None, verify=None, max_rounds=None, commit_template=None, trailers=None, trailer=()):
    """Set the workflow: its type, and what is given of the rest; `trailers` forbid, require (with `trailer` lines) or
    none (removes the rule). Returns the workflow with its defaults."""
    if workflow_type not in WORKFLOW_TYPES:
        raise Invalid(f"atm workflow set needs a type: {' or '.join(WORKFLOW_TYPES)}")
    changes = {"type": workflow_type}
    if base is not None:
        changes["base"] = base
    if verify is not None:
        changes["verify"] = verify
    if max_rounds is not None:
        changes["max_rounds"] = max_rounds
    if commit_template is not None:
        changes["commit_template"] = commit_template
    if trailers == "forbid":
        changes["trailers"] = "forbid"
    elif trailers == "require":
        if not trailer:
            raise Invalid("--trailers require needs at least one --trailer LINE")
        changes["trailers"] = {"require": list(trailer)}

    def change(work):
        data = work.document
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        current = data["meta"].get("workflow") if isinstance(data["meta"].get("workflow"), dict) else {}
        workflow = {**current, **changes}
        if trailers == "none":
            workflow.pop("trailers", None)
        data["meta"]["workflow"] = workflow
        return workflow_of(data)  # validates before anything is written

    return project.store.change(change)


def commit_message(text, trailers):
    """Apply the workflow's trailer rule: forbid removes Co-Authored-By lines, require adds the missing lines."""
    if trailers == "forbid":
        text = "\n".join(line for line in text.splitlines() if not CO_AUTHORED.match(line.strip())).rstrip()
    elif isinstance(trailers, dict):
        missing = [line for line in trailers["require"] if line not in text.splitlines()]
        if missing:
            text = text.rstrip() + "\n\n" + "\n".join(missing)
    return text


def record_on_task(project, task_id, sha):
    """The task's commit is `sha` (the SHA is known only once the commit exists: a change for the next commit)."""

    def change(work):
        task = find_task(work.document, task_id)
        task["commit"] = sha
        touch(task, project.actor, now_iso())
        return task

    return project.store.change(change)


def commit_task(project, task_id, files, cwd, message=None):
    """`atm commit`: commit exactly `files` (named relative to `cwd`) plus the task data that travels with code commits,
    with the workflow's message, then record the commit on the task. Returns {task, commit, message, files}."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm commit needs a git repository")
    if not files:
        raise Invalid("atm commit needs --file (once per file the task changed), or --record REV")
    data = project.store.document()
    task = find_task(data, task_id)
    workflow = workflow_of(data) or WORKFLOW_DEFAULTS
    text = message if message is not None else workflow["commit_template"]
    for key in ("id", "name", "phase", "type"):
        text = text.replace("{" + key + "}", str(task.get(key, "")))
    text = commit_message(text, workflow.get("trailers"))
    if not text.strip():
        raise Invalid("the commit message is empty")
    picked = vcs.paths(files, cwd)
    picked += [f for f in project.store.files_for_commit() if f not in picked]
    sha = vcs.commit(picked, text)
    task = record_on_task(project, task_id, sha)
    return {"task": task["id"], "commit": sha, "message": text, "files": picked}


def record_commit(project, task_id, rev):
    """`atm commit --record` (T-343): record an existing commit on the current branch, e.g. the one a rebase or a
    cherry-pick made of the task's commit, without committing anything. Returns {task, commit, recorded, summary,
    hint}: `hint` when the commit's message doesn't mention the task."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm commit needs a git repository")
    if rev.startswith("-"):
        raise Invalid(f"--record takes a revision, not an option: {rev!r}")
    sha = vcs.resolve(rev)
    if not sha:
        raise NotFound(f"no commit {rev} in this repository")
    if not vcs.in_history(sha):
        raise AtmFailure(f"{sha[:7]} is not on the current branch (an ancestor of HEAD)")
    text = vcs.message(sha)
    task = record_on_task(project, task_id, sha)
    hint = None
    if not re.search(rf"(?<![\w-]){re.escape(task['id'])}(?![\w-])", text):
        hint = f"its message doesn't mention {task['id']}: is it the task's commit?"
    return {"task": task["id"], "commit": sha, "recorded": True, "summary": text.splitlines()[0] if text else "", "hint": hint}


def record_pr(project, ids, number, url):
    """`atm pr`: the PR that contains the tasks (DM-4.3: the tasks are then In PR). Returns the task IDs."""
    if number <= 0:
        raise Invalid("--number must be a positive whole number")
    # The dashboard links to it: only a web address, never javascript: or data: (docs/security-review.md F9)
    if not HTTP_URL.match(url):
        raise Invalid(f"--url must be an http:// or https:// address, not {url!r}")

    def change(work):
        found = [find_task(work.document, task_id) for task_id in ids]
        stamp = now_iso()
        for task in found:
            task["pr"] = {"number": number, "url": url}
            task.pop("merged_at", None)
            touch(task, project.actor, stamp)
        return [t["id"] for t in found]

    return project.store.change(change)


def release_with_merge(data, merged, pr_number, at):
    """Release the planned releases the merged PR completes (SR-3.7): every task of the release in tasks.json (its own
    release, or its sprint's) is in this PR and merged; archived and skipped tasks don't hold it back. Sets its state,
    `pr` and `released_at`. Returns (the released IDs, {release: the tasks still keeping it planned})."""
    vocab = vocabulary(data)
    touched = {effective_release(vocab, t) for t in merged} - {None}
    released, pending = [], {}
    for release in (data.get("meta") or {}).get("releases") or []:
        if not isinstance(release, dict) or release.get("id") not in touched:
            continue
        if release.get("state", "planned") != "planned":
            continue
        own = [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and effective_release(vocab, t) == release["id"]
            and kind_of(vocab, t.get("status")) != "skipped"
        ]
        waiting = [
            t.get("id")
            for t in own
            if not (isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr_number and t.get("merged_at"))
        ]
        if waiting:
            pending[release["id"]] = waiting
            continue
        url = next((t["pr"].get("url") for t in merged if isinstance(t["pr"].get("url"), str)), None)
        release["state"] = "released"
        release["pr"] = {"number": pr_number, **({"url": url} if url else {})}
        release["released_at"] = at
        released.append(release["id"])
    return released, pending


def record_merge(project, pr, at=None):
    """`atm merged`: PR `pr` was merged (at `at`, default now), on every task that records it (DM-4.3: the tasks are
    then Merged); the releases it completes are released. Returns {pr, tasks, already, released, pending}."""
    if at is not None:
        try:
            parse_iso(at)
        except ValueError:
            raise Invalid(f"--at needs a time like 2026-10-24T18:00:00Z, not {at!r}")

    def change(work):
        data = work.document
        tasks = [
            t for t in data["tasks"] if isinstance(t, dict) and isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr
        ]
        if not tasks:
            raise NotFound(f"no task records PR #{pr} (atm pr records it)")
        stamp = now_iso()
        changed = [t for t in tasks if not t.get("merged_at")]
        for task in changed:
            task["merged_at"] = at or stamp
            touch(task, project.actor, stamp)
        released, pending = release_with_merge(data, tasks, pr, at or stamp)
        return {
            "pr": pr,
            "tasks": [t["id"] for t in changed],
            "already": [t["id"] for t in tasks if t not in changed],
            "released": released,
            "pending": pending,
        }

    return project.store.change(change)


# ==== boot/open.py ====
# Opening a project: its task files (the JSON-in-git store) and git.

def open_project(cwd, actor=None, tolerant=False):
    """The project at or above `cwd`. `tolerant`: read a malformed task file as far as it can be read (T-377)."""
    return project_at(find_paths(cwd), actor, tolerant)


def project_at(paths, actor=None, tolerant=False):
    """The project whose task files `paths` locates (atm serve opens each registered project this way)."""
    return Project(JsonGitStore(paths, tolerant=tolerant), GitVcs(paths["root"]), actor)


# ==== cli/render.py ====
# Text for people and agents: how the CLI shows atm-lib's results. Short and stable, for agents to read (CLI-3.3).

def list_lines(vocab, tasks):
    """One line per task, as `atm list` prints them: ID, status, priority, labels, name."""
    id_width = max(len(str(t.get("id"))) for t in tasks)
    status_width = max(len(str(t.get("status"))) for t in tasks)
    for t in tasks:
        labels = t.get("labels") or []
        yield (
            f"{str(t.get('id')).ljust(id_width)}  {str(t.get('status')).ljust(status_width)}  "
            f"{priority_of(vocab, t)}  "
            + (f"[{', '.join(labels)}] " if labels else "")
            + str(t.get("name", ""))
        )


def describe_task(data, task):
    """A task in full, as text, for the commands not on the layers yet (the status changes print the task they
    start): the same text as `atm show`, from atm-lib's view of it."""
    # A task not in tasks.json is from the archive
    archived = all(t is not task for t in data["tasks"])
    return render_task(view_task(task, archived, data["tasks"], archived_task, vocabulary(data)))


def render_task(view):
    """A task in full, as text (from atm-lib's `view_task`): what an agent needs to work on it."""
    task = view["task"]
    labels = task.get("labels") or []
    lines = [
        f"{task.get('id')}  {task.get('name', '')}",
        f"  {task.get('status')} · {task.get('sprint') or 'backlog'}"
        # A task from the archive says so (found in T-277, T-353)
        + (" · archived" if view["archived"] else "")
        + f" · {task.get('type')} · {view['priority']}"
        + (f" · labels: {', '.join(labels)}" if labels else ""),
    ]
    if view["dependencies"]:
        lines.append(
            "  depends on: "
            + ", ".join(
                f"{d['id']} (missing)"
                if d["status"] is None
                else f"{d['id']} ({d['status']}" + (", archived" if d["archived"] else "") + ")"
                for d in view["dependencies"]
            )
        )
    # How it ended, when it isn't the default (done, or won't do for a skipped task), and its relations (SR-2.8)
    if task.get("resolution"):
        lines.append(
            f"  resolution: {task['resolution']}" + (f" (of {task['duplicate_of']})" if task.get("duplicate_of") else "")
        )
    if view["related"]:
        lines.append("  related: " + ", ".join(view["related"]))
    if view["duplicated_by"]:
        lines.append("  duplicated by: " + ", ".join(view["duplicated_by"]))
    for key in ("requirements", "use_cases", "files"):
        if task.get(key):
            lines.append(f"  {key.replace('_', ' ')}: {', '.join(task[key])}")
    who = []
    for what, at, by in (
        ("created", "created_at", task.get("created_by")),
        ("started", "started_at", view["started_by"]),
        ("review requested", "review_requested_at", None),
        ("completed", "completed_at", task.get("completed_by")),
    ):
        if task.get(at):
            who.append(f"{what} {task[at]}" + (f" by {by}" if by else ""))
    if who:
        lines.append("  " + " · ".join(who))
    if task.get("commit"):
        lines.append(f"  commit: {task['commit']}")
    description = (task.get("description") or "").strip()
    if description:
        lines.append("  description:")
        lines.extend("    " + line for line in description.splitlines())
    notes = task.get("notes") or []
    if notes:
        lines.append("  notes:")
        for n in notes:
            lines.append(f"    {n.get('at')} {n.get('kind')}" + (f" ({n['by']})" if n.get("by") else "") + f": {n.get('text')}")
    elif task.get("last_failure"):
        failure = task["last_failure"]
        lines.append(f"  last failure: {failure.get('at')}: {failure.get('reason')}")
    return "\n".join(lines)


# ==== migrate.py ====
# `atm migrate`: move a legacy tasks.json into the task folder and set the folder up (.gitignore, merge driver), and
# clean up after per-project dashboards (0.10.0-0.14.0): the dashboard now comes from the plugin, served by
# `atm serve` for all projects.

TEMPLATE = os.path.join(PLUGIN_ROOT, "templates", DASHBOARD_FILE)
# Runtime files in the task folder that must not be committed (the dashboard and server files are from before
# 0.15.0; they stay listed so leftovers stay out of git)
IGNORED = ["tasks-dashboard.html", ".server.json", ".server.log", "claims.json", "claims-store.json", ".lock/"]
OLD_SERVER_RECORD = ".server.json"
OLD_SERVER_LOG = ".server.log"
VERSION_META = re.compile(r'<meta name="atm-dashboard-version" content="([^"]*)"')
BUILD_META = re.compile(r'<meta name="atm-dashboard-build" content="([^"]*)"')
# The tags sit at the top of the page; reading this much finds them without reading the whole (inlined) dashboard
HEAD_BYTES = 16384


def page_tags(head):
    """(version, build ID) from the start of a dashboard page (text), "" for a missing tag."""
    version, build = VERSION_META.search(head), BUILD_META.search(head)
    return (version.group(1) if version else "", build.group(1) if build else "")


def dashboard_head(p):
    """(version, build ID) of a dashboard file, "" for a missing tag; None if there is no file."""
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            head = f.read(HEAD_BYTES)
    except OSError:
        return None
    return page_tags(head)


def written_by_atm(path):
    """Whether an old server record is one atm wrote here: owned by this user and not tracked by git (atm always kept
    it out of git). A record that came with the repository is only deleted: atm never contacts its port or signals its
    pid (docs/security-review.md F14)."""
    try:
        if os.stat(path).st_uid != os.getuid():
            return False
    except (OSError, AttributeError):  # missing, or no uids (Windows)
        return False
    return not is_tracked(os.path.dirname(path), path)


def clean_up_old_dashboard(task_dir):
    """Per-project dashboards (0.10.0-0.14.0): delete the project's copy of the dashboard, stop its server if one still
    runs (only when whoami confirms it serves this folder), and remove the server's record and log. Returns what was
    done, or None when there was nothing left."""
    removed = False
    copy = os.path.join(task_dir, DASHBOARD_FILE)
    if os.path.isfile(copy):
        os.remove(copy)
        removed = True
    stopped = None
    record_path = os.path.join(task_dir, OLD_SERVER_RECORD)
    record = read_json(record_path) if written_by_atm(record_path) else None
    if record is not None:
        info = whoami(record.get("port")) if isinstance(record, dict) else None
        if info and info.get("folder") == task_dir and info.get("pid") == record.get("pid"):
            try:
                os.kill(record["pid"], signal.SIGTERM)
                stopped = record["pid"]
            except OSError:
                pass
    had_files = False
    for name in (OLD_SERVER_RECORD, OLD_SERVER_LOG):
        p = os.path.join(task_dir, name)
        if os.path.exists(p):
            os.remove(p)
            had_files = True
    if not (removed or stopped or had_files):
        return None
    return {"dashboard": removed, "server": stopped}


FINISH_FIELDS = {"done": "completed_at", "skipped": "skipped_at"}


def missing_finish_times(tasks, vocab):
    """The finished tasks without their finish time (done ones from before completed_at existed; skipped ones from
    before skipped_at existed), with the field each one needs."""
    result = []
    for task in tasks:
        field = FINISH_FIELDS.get(kind_of(vocab, task.get("status"))) if isinstance(task, dict) else None
        if field and not task.get(field) and task.get("updated_at"):
            result.append((task, field))
    return result


def backfill_finish_times(paths):
    """Give finished tasks without a finish time their updated_at, which is what the dashboard showed for them anyway,
    so a later edit (which moves updated_at) doesn't move them in time. In tasks.json and the archive; once per task;
    updated_at and updated_by stay. Returns how many tasks were filled in."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        if not missing_finish_times(data["tasks"], vocab) and not missing_finish_times(load_archive(paths)["tasks"], vocab):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    filled = 0
    with locked(paths["dir"]):
        data = load_tasks(paths)
        files = [(paths["tasks"], data)] + ([(archive, load_archive(paths))] if os.path.isfile(archive) else [])
        for path, contents in files:
            missing = missing_finish_times(contents["tasks"], vocab)
            for task, field in missing:
                task[field] = task["updated_at"]
            if missing:
                write_json_atomic(path, contents)
                filled += len(missing)
    return filled


FINISHED_KINDS = ("done", "skipped")


def task_phase(task):
    """The task's phase (a number), or None: a task without one goes to the backlog."""
    phase = task.get("phase") if isinstance(task, dict) else None
    if not isinstance(phase, (int, float)) or isinstance(phase, bool):
        return None
    return int(phase) if float(phase).is_integer() else phase


def phase_sprint_names(meta, tasks):
    """{phase: sprint name} for every phase with tasks or a name, in phase order: its name, or "Phase n" when
    unnamed, with " (phase n)" added to a name already taken (the names the dashboard shows for unmigrated projects,
    dashboard/src/lib/sprints.ts)."""
    named = {}
    for p in meta.get("phases") or []:
        if isinstance(p, dict) and task_phase({"phase": p.get("id")}) is not None:
            name = p.get("name").strip()[:MAX_SPRINT_NAME].rstrip() if isinstance(p.get("name"), str) else ""
            if name:
                named[task_phase({"phase": p["id"]})] = name
    phases = set(named) | {task_phase(t) for t in tasks} - {None}
    names, taken = {}, set()
    for phase in sorted(phases):
        name = named.get(phase) or f"Phase {phase}"
        if name.casefold() in taken:
            suffix = f" (phase {phase})"
            name = name[: MAX_SPRINT_NAME - len(suffix)].rstrip() + suffix
        taken.add(name.casefold())
        names[phase] = name
    return names


def sprint_from_phase(name, tasks, vocab):
    """The sprint a phase becomes: closed when all its tasks are finished (start, end and report from them), else
    planned (SR-8.1)."""
    finished = bool(tasks) and all(kind_of(vocab, t.get("status")) in FINISHED_KINDS for t in tasks)
    if not finished:
        return {"id": name, "state": "planned"}
    sprint = {"id": name, "state": "closed"}
    starts = [t["started_at"] for t in tasks if isinstance(t.get("started_at"), str)]
    ends = [
        t.get("completed_at") or t.get("skipped_at") or t.get("updated_at")
        for t in tasks
        if isinstance(t.get("completed_at") or t.get("skipped_at") or t.get("updated_at"), str)
    ]
    if starts:
        sprint["start"] = min(starts)
    if ends:
        sprint["end"] = max(ends)
    sprint["report"] = {
        "done": [t["id"] for t in tasks if kind_of(vocab, t.get("status")) == "done" and "id" in t],
        "carried": [],
    }
    return sprint


def phases_to_sprints(data, archived, vocab, stamp):
    """Turn phases into sprints in tasks.json's data and the archive's tasks, in place (SR-8.1). A project that already
    has meta.sprints only gets its leftover phases converted (a task merged in from an unmigrated branch, SR-8.5).
    Returns the sprints it added, or [] when there was nothing to convert."""
    meta = data.setdefault("meta", {})
    tasks = [t for t in list(data["tasks"]) + list(archived) if isinstance(t, dict)]
    if not meta.get("phases") and not any("phase" in t for t in tasks):
        return []
    names = phase_sprint_names(meta, tasks)
    sprints = meta.get("sprints") if isinstance(meta.get("sprints"), list) else []
    existing = {s["id"].casefold() for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    added = []
    for phase, name in names.items():
        if name.casefold() in existing:
            continue
        sprint = sprint_from_phase(name, [t for t in tasks if task_phase(t) == phase], vocab)
        sprints.append(sprint)
        added.append(sprint)
    meta["sprints"] = sprints
    meta.pop("phases", None)
    by_name = {s["id"].casefold(): s["id"] for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    for task in tasks:
        phase = task_phase(task)
        task.pop("phase", None)
        if phase is None or task.get("sprint"):
            continue
        task["sprint"] = by_name[names[phase].casefold()]
        task.setdefault("sprint_history", []).append(
            {"sprint": task["sprint"], "from": task.get("created_at") if isinstance(task.get("created_at"), str) else stamp}
        )
    return added


def migrate_phases(paths):
    """phases_to_sprints on the files under the lock, staging them in git (not committing). Returns the sprints it
    added, or None when there was nothing to convert."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        archive_data = load_archive(paths)
        if not data.get("meta", {}).get("phases") and not any(
            isinstance(t, dict) and "phase" in t for t in list(data["tasks"]) + list(archive_data["tasks"])
        ):
            return None
    except AtmError:
        return None  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive_data = load_archive(paths)
        added = phases_to_sprints(data, archive_data["tasks"], vocab, now_iso())
        if isinstance(archive_data.get("meta"), dict):
            archive_data["meta"].pop("phases", None)
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
        written = [paths["tasks"]]
        if os.path.isfile(archive):
            write_json_atomic(archive, archive_data)
            written.append(archive)
    if in_git(paths["root"]):
        git(["add", "--"] + [rel(paths["root"], p) for p in written], paths["root"])
    return added


WITH_USER_MIGRATION = "interactive-label"


def migrate_interactive(paths):
    """Tasks labelled `interactive` (work done with the user, T-240) get `with: user` (SR-6.5), once: the change is
    recorded in meta.migrations, so a `with` taken off later isn't put back. The label stays. Only unfinished tasks in
    tasks.json; nothing is written when none has the label. Returns how many tasks it changed."""
    def todo(data):
        meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
        if WITH_USER_MIGRATION in (meta.get("migrations") or []):
            return []
        vocab = vocabulary(data)
        return [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and "interactive" in (t.get("labels") or [])
            and not t.get("with")
            and kind_of(vocab, t.get("status")) not in ("done", "skipped")
        ]

    try:
        if not todo(load_tasks(paths)):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        tasks = todo(data)
        for task in tasks:
            task["with"] = "user"
        meta = data.setdefault("meta", {})
        meta["migrations"] = list(meta.get("migrations") or []) + [WITH_USER_MIGRATION]
        write_json_atomic(paths["tasks"], data)
    if in_git(paths["root"]):
        git(["add", "--", rel(paths["root"], paths["tasks"])], paths["root"])
    return len(tasks)


def ensure_gitignore(task_dir):
    """Add any missing runtime entries to the folder's .gitignore, keeping the user's lines. Returns True if changed."""
    p = os.path.join(task_dir, ".gitignore")
    try:
        with open(p, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    lines = text.splitlines()
    missing = [entry for entry in IGNORED if entry not in lines]
    if not missing:
        return False
    if not text:
        text = "# atm runtime files (managed by atm)\n"
    elif not text.endswith("\n"):
        text += "\n"
    with open(p, "w", encoding="utf-8") as f:
        f.write(text + "".join(entry + "\n" for entry in missing))
    return True


def migrate(paths):
    """Move a legacy tasks.json into the task folder, set the folder up, and turn phases into sprints. Never commits."""
    root, task_dir, legacy = paths["root"], paths["dir"], paths["legacy"]
    result = {
        "moved": None,
        "removed_dashboard": None,
        "warning": None,
        "gitignore_updated": False,
        "merge_driver_updated": False,
        "cleaned_up": None,
        "finish_times_filled": 0,
        "sprints_from_phases": None,
        "with_user_from_label": 0,
    }
    if legacy and paths["tasks_exists"]:
        result["warning"] = (
            f"both {rel(root, legacy['tasks'])} and {rel(root, paths['tasks'])} exist; "
            f"using {rel(root, paths['tasks'])}. Remove the old file when you no longer need it."
        )
    elif legacy:
        os.makedirs(task_dir, exist_ok=True)
        tracked = is_tracked(root, legacy["tasks"])
        if tracked:
            if git(["mv", "--", rel(root, legacy["tasks"]), rel(root, paths["tasks"])], root) is None:
                raise AtmError(f"git mv {rel(root, legacy['tasks'])} failed")
        else:
            os.rename(legacy["tasks"], paths["tasks"])
        result["moved"] = {"from": rel(root, legacy["tasks"]), "to": rel(root, paths["tasks"]), "git": tracked}
        if legacy["dashboard"]:
            if is_tracked(root, legacy["dashboard"]):
                git(["rm", "--cached", "-q", "--", rel(root, legacy["dashboard"])], root)
            os.remove(legacy["dashboard"])
            result["removed_dashboard"] = rel(root, legacy["dashboard"])
    if not os.path.isfile(paths["tasks"]):
        return result
    if ensure_gitignore(task_dir):
        result["gitignore_updated"] = True
        if in_git(root):
            git(["add", "--", rel(root, os.path.join(task_dir, ".gitignore"))], root)
    result["merge_driver_updated"] = ensure_merge_driver(paths)
    if not os.path.exists(os.path.join(task_dir, CLAIMS_SNAPSHOT)):
        # So the dashboard finds the file from the start instead of logging a 404 on every refresh
        write_claims_snapshot(paths, read_store(paths)["claims"])
    result["cleaned_up"] = clean_up_old_dashboard(task_dir)
    result["finish_times_filled"] = backfill_finish_times(paths)
    result["sprints_from_phases"] = migrate_phases(paths)
    result["with_user_from_label"] = migrate_interactive(paths)
    try:
        hint = archive_hint(paths, load_tasks(paths))
    except AtmError:
        hint = None  # the command the skill runs next reports the problem
    if hint:
        result["suggest"] = {"archive": hint}
    return result


def describe_sprints_from_phases(sprints):
    """What the phase migration did, said once (it doesn't run again)."""
    if not sprints:
        return ["Moved the tasks' phases to their sprints (staged, not committed)."]
    closed = [s["id"] for s in sprints if s["state"] == "closed"]
    planned = [s["id"] for s in sprints if s["state"] == "planned"]
    count = "1 phase" if len(sprints) == 1 else f"{len(sprints)} phases"
    lines = [f"Phases are now sprints: turned {count} into sprints (staged, not committed)."]
    if closed:
        lines.append(f"Closed (all their tasks finished): {', '.join(closed)}.")
    if planned:
        lines.append(f"Planned: {', '.join(planned)}.")
        lines.append("None is active yet: plan one with /atm:plan-sprint.")
    return lines


def describe_migration(r, paths):
    lines = []
    if r["warning"]:
        lines.append("warning: " + r["warning"])
    if r["moved"]:
        line = f"Moved {r['moved']['from']} to {r['moved']['to']}"
        if r["moved"]["git"]:
            line += " (staged with git mv, not committed)"
        if r["removed_dashboard"]:
            line += f"; removed {r['removed_dashboard']} (the dashboard now comes with the plugin: /atm:serve)"
        lines.append(line + ".")
    elif not os.path.isfile(paths["tasks"]):
        lines.append("No tasks.json found. Run /atm:create-tasks to create one.")
    cleaned = r["cleaned_up"]
    if cleaned:
        line = "Removed this project's old dashboard files"
        if cleaned["server"]:
            line += f" and stopped its old dashboard server (pid {cleaned['server']})"
        lines.append(line + ": one dashboard for all your projects now comes with the plugin (/atm:serve).")
    filled = r.get("finish_times_filled")
    if filled:
        tasks = "1 finished task" if filled == 1 else f"{filled} finished tasks"
        lines.append(f"Filled in the finish time of {tasks} that had none, from the last update (completed_at/skipped_at).")
    sprints = r.get("sprints_from_phases")
    if sprints is not None:
        lines.extend(describe_sprints_from_phases(sprints))
    together = r.get("with_user_from_label")
    if together:
        tasks = "1 task" if together == 1 else f"{together} tasks"
        lines.append(
            f"Gave {tasks} labelled interactive `with: user` (done together with you; unattended loops skip them)."
        )
    if not lines:
        lines.append("Nothing to migrate.")
    if r.get("suggest"):
        lines.append(r["suggest"]["archive"]["text"])
    return lines


def cmd_migrate(args):
    paths = find_paths(os.getcwd())
    r = migrate(paths)
    if os.path.isfile(paths["tasks"]):
        register(paths)  # the dashboard lists every project atm runs in (every skill runs migrate first)
    if args.json:
        out(to_json(r))
        return
    for line in describe_migration(r, paths):
        out(line)


# ==== api/server.py ====
# `atm serve`: one dashboard server per user, a detached process serving every registered project.
#
# The server holds the plugin's dashboard in memory and serves each registered project's task files under /p/<id>/.
# Its routes and record are described in docs/multi-project-api.md.

SERVER_RECORD = "server.json"
SERVER_LOG = "server.log"
DEFAULT_PORT = 8765
PORT_TRIES = 100
PROJECT_FILES = ("tasks.json", "claims.json", "tasks-archive.json", "worktrees.json")
# A task file the server will read into memory and send (a big project's archive is a few MB)
MAX_SERVED_BYTES = 100_000_000
PROJECT_FILE = re.compile(r"/p/([^/]+)/(" + "|".join(re.escape(f) for f in PROJECT_FILES + ("logo",)) + r")")
# A logo is the project's own file, which could be an SVG with a script: it is shown as an image, never run
LOGO_HEADERS = {
    "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; sandbox",
    "X-Content-Type-Options": "nosniff",
}
# On every response: no content sniffing, and no other site may embed it (docs/security-review.md F11)
SECURITY_HEADERS = {"X-Content-Type-Options": "nosniff", "Cross-Origin-Resource-Policy": "same-origin"}
INLINE_SCRIPT = re.compile(rb"<script\b([^>]*)>(.*?)</script>", re.S | re.I)


def page_csp(page):
    """The dashboard's Content-Security-Policy: only its own inline scripts run (by hash), it talks only to this
    server, and no page may frame it. Any script injected into it, or a `javascript:` link, is refused by the browser.
    tests/e2e/serve.mjs builds the same policy, so the e2e suite runs under it."""
    hashes = [
        "'sha256-" + base64.b64encode(hashlib.sha256(body).digest()).decode("ascii") + "'"
        for attrs, body in INLINE_SCRIPT.findall(page)
        if b"src=" not in attrs
    ]
    return "; ".join(
        [
            "default-src 'none'",
            "script-src " + (" ".join(hashes) or "'none'"),
            "style-src 'unsafe-inline'",
            "img-src 'self' data:",
            "connect-src 'self'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ]
    )


def server_url(port, project=None):
    return f"http://localhost:{port}/" + (f"#project={project}" if project else "")


def live_server(home):
    """The record of the user's running server, or None. A stale record is removed."""
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        return None
    info = whoami(record.get("port"))
    if info and info.get("pid") == record.get("pid"):
        return record
    os.remove(record_path)
    return None


def open_browser(url):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.Popen([opener, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        out(f"Could not open a browser; open {url} yourself.")


def ensure_server(home, port):
    """The running server's record, starting the server if none runs. Returns (record, started)."""
    record = live_server(home)
    if record:
        return record, False
    ensure_home(home)
    with open(os.path.join(home, SERVER_LOG), "ab") as log:
        child = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "_serve-child", home, str(port)],
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
    deadline = time.time() + 10
    while time.time() < deadline:
        record = read_json(os.path.join(home, SERVER_RECORD))
        if record and record.get("pid") == child.pid and whoami(record["port"]):
            return record, True
        if child.poll() is not None:
            break
        time.sleep(0.05)
    raise AtmFailure(f"the dashboard server did not start; see {os.path.join(home, SERVER_LOG)}")


def project_for(paths):
    """(id, listed) of the project at `paths`, after registering it."""
    register(paths)
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"]) or {}
    # The id the registry gave it: meta.id, or another when that one is taken (registry.unique_id)
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE)) or {}
    entry = next((e for e in registry.get("projects", []) if isinstance(e, dict) and e.get("path") == path), None)
    return (entry or {}).get("id") or project_id(meta, path), listed(meta)


def serve_start(paths, port, overview):
    """Start (or reuse) the server; returns the URL to show: the project's, or the overview's."""
    project = None
    if not overview:
        for line in describe_migration(migrate(paths), paths):
            if line != "Nothing to migrate.":
                out(line)
        if not os.path.isfile(paths["tasks"]):
            raise AtmFailure("no tasks.json to serve. Run /atm:create-tasks first.")
        project, shown = project_for(paths)
        if not shown:
            out("This project isn't listed on the dashboard (meta.dashboard.listed is false).")
            project = None
    home = atm_home()
    record, started = ensure_server(home, port)
    url = server_url(record["port"], project)
    out(f"Dashboard server {'started' if started else 'already running'}: {url}")
    return url


def stop_server(home, pid):
    """Stop the user's server (whose pid whoami confirmed) and remove its record."""
    os.kill(pid, signal.SIGTERM)
    deadline = time.time() + 5
    while time.time() < deadline and pid_alive(pid):
        time.sleep(0.05)
    record_path = os.path.join(home, SERVER_RECORD)
    if os.path.exists(record_path):
        os.remove(record_path)


def serve_stop(home):
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        out("Dashboard server is not running.")
        return
    info = whoami(record.get("port"))
    if not (info and info.get("pid") == record.get("pid")):
        os.remove(record_path)
        out(f"Dashboard server is not running (removed a stale record for pid {record.get('pid')}).")
        return
    stop_server(home, record["pid"])
    out(f"Dashboard server stopped (pid {record['pid']}).")


def outdated(record):
    """True when the server runs an older dashboard than this plugin's: an older version, or the same version but
    another build of this plugin's own dashboard file (it was rebuilt since the server started). A newer server is
    left alone, so a session still on an older plugin never downgrades it. Another install of the same version (a
    development checkout, a worktree, the plugin cache) is left alone too: neither build is newer, and replacing it
    would make each install restart the server in turn, every open page offering a reload each time."""
    mine, mine_build = dashboard_head(TEMPLATE) or ("", "")
    theirs, theirs_build = str(record.get("version") or ""), str(record.get("build") or "")
    if parse_version(mine) != parse_version(theirs):
        return parse_version(mine) > parse_version(theirs)
    theirs_root = record.get("plugin_root")
    if theirs_root and os.path.realpath(str(theirs_root)) != os.path.realpath(PLUGIN_ROOT):
        return False
    return bool(mine_build) and mine_build != theirs_build


def refresh_server(home=None):
    """Restart the user's server on the same port if it runs an older dashboard than this plugin's (after a plugin
    update). Cheap when there is nothing to do: one read of server.json, and no request unless it is outdated.
    Never starts a server that isn't running. Returns the new record, or None."""
    home = home or atm_home()
    record = read_json(os.path.join(home, SERVER_RECORD))
    if not isinstance(record, dict) or not outdated(record):
        return None
    live = live_server(home)
    if not live:
        return None
    stop_server(home, live["pid"])
    return ensure_server(home, live["port"])[0]


def serve_refresh(home):
    record = refresh_server(home)
    if record:
        out(f"Dashboard server restarted with dashboard {record.get('version')}: {server_url(record['port'])}")


def serve_status(home, as_json):
    record = live_server(home)
    if as_json:
        out(to_json({"running": bool(record), **(record or {}), "url": server_url(record["port"]) if record else None}))
        return
    if not record:
        out("Dashboard server is not running.")
        return
    up = format_duration((datetime.now(timezone.utc) - parse_iso(record["started_at"])).total_seconds())
    out(
        f"Dashboard server running: {server_url(record['port'])} "
        f"(pid {record['pid']}, dashboard {record.get('version')}, up {up})"
    )


def cmd_serve(args):
    # --dir acts on another project without a cd (which permission rules can't check)
    cwd = os.path.abspath(args.dir) if args.dir else os.getcwd()
    if not os.path.isdir(cwd):
        raise AtmFailure(f"--dir {args.dir}: not a directory", 2)
    home = atm_home()
    if args.refresh:
        serve_refresh(home)
    elif args.action == "start":
        refresh_server(home)  # an outdated server is replaced before its URL is shown
        url = serve_start(find_paths(cwd), args.port, args.all)
        if args.open:
            open_browser(url)
    elif args.action == "stop":
        serve_stop(home)
    else:
        serve_status(home, args.json)


def port_in_use(port):
    """Something accepts connections on 127.0.0.1:port or [::1]:port (`localhost` may resolve to either)."""
    for family, host in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
        try:
            with socket.socket(family, socket.SOCK_STREAM) as s:
                s.settimeout(0.5)
                if s.connect_ex((host, port)) == 0:
                    return True
        except OSError:  # no IPv6 on this machine
            pass
    return False


def file_stamp(p):
    """(size, nanosecond mtime) of a file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


def file_etag(stamp):
    """A strong ETag from a file's size and nanosecond mtime. Last-Modified alone has one-second resolution, so two
    atm changes in the same second would look unchanged to the dashboard."""
    return f'"{stamp[0]:x}-{stamp[1]:x}"'


class Registry:
    """The registry as the server sees it, re-read when projects.json changes."""

    def __init__(self, home):
        self.path = os.path.join(home, REGISTRY_FILE)
        self.stamp = None
        self.projects = []

    def entries(self):
        stamp = file_stamp(self.path)
        if stamp != self.stamp:
            data = read_json(self.path)
            entries = data.get("projects") if isinstance(data, dict) else None
            self.projects = [e for e in entries or [] if isinstance(e, dict) and isinstance(e.get("path"), str)]
            self.stamp = stamp
        return self.projects

    def find(self, project):
        return next((e for e in self.entries() if e.get("id") == project), None)


def task_file(entry, name):
    return os.path.join(entry["path"], TASK_DIR, name)


def served_file(entry, name):
    """The project's task file `name` if it may be served: a regular file, not a symlink, in a task folder that isn't
    one either (docs/security-review.md F8: a repository could otherwise link tasks-archive.json to ~/.ssh/id_rsa).
    None otherwise."""
    folder = os.path.join(entry["path"], TASK_DIR)
    p = os.path.join(folder, name)
    if os.path.islink(folder) or os.path.islink(p) or not os.path.isfile(p):
        return None
    real_folder = os.path.realpath(folder)
    return p if os.path.dirname(os.path.realpath(p)) == real_folder else None


class Summaries:
    """Each project's summary (atm-lib's project_summary), computed again only when its tasks.json or claims store
    changed, or the day did (the completions count by day)."""

    def __init__(self):
        self.paths = {}  # project folder -> (atm paths, claims store path): finding them runs git
        self.cache = {}  # project folder -> (stamp, summary or None)

    def store(self, entry):
        """The project's claims store, shared by all its worktrees."""
        if entry["path"] not in self.paths:
            paths = find_paths(entry["path"])
            self.paths[entry["path"]] = (paths, claims_store_path(paths))
        return self.paths[entry["path"]][1]

    def project(self, entry, tolerant=False):
        """The project, opened afresh for each request: atm-lib reads one snapshot of its files per call. `tolerant`
        reads a malformed task file as far as it can be read, for showing it (T-377)."""
        self.store(entry)
        return project_at(self.paths[entry["path"]][0], tolerant=tolerant)

    def stamp(self, entry):
        """What the summary depends on, or None when the project is missing: tasks.json, the claims store, the day,
        and .git/HEAD (the branch on the card)."""
        tasks = file_stamp(task_file(entry, TASKS_FILE))
        if tasks is None:
            return None
        head = file_stamp(os.path.join(entry["path"], ".git", "HEAD"))
        return (tasks, file_stamp(self.store(entry)), now().date().isoformat(), head)

    def summary(self, entry, stamp):
        cached = self.cache.get(entry["path"])
        if cached and cached[0] == stamp:
            return cached[1]
        try:
            summary = project_summary(self.project(entry))
        except AtmFailure:
            summary = None  # unreadable tasks.json or an unusable vocabulary: listed as missing
        self.cache[entry["path"]] = (stamp, summary)
        return summary


def project_logo(entry):
    """(absolute path, content type) of a registered project's logo (meta.logo, or one found), or None."""
    logo = find_logo(entry["path"], project_meta(entry["path"]) or {})
    return (logo[1], logo[2]) if logo else None


def quietly(fn, default):
    """fn(), or `default` if it fails: one project's broken files never break the list of all projects (F18)."""
    try:
        return fn()
    except Exception:  # noqa: BLE001 - any failure in one project's files
        return default


def api_projects(registry, summaries):
    """(body, ETag) of /api/projects. The ETag changes with the registry, and with each project's tasks.json, claims,
    logo and the day. A project whose files can't be read is listed as missing; the others are unaffected."""
    entries = registry.entries()
    stamps = [quietly(lambda e=e: summaries.stamp(e), None) for e in entries]
    trees = [quietly(lambda e=e: worktrees(e["path"]), []) if stamp else [] for e, stamp in zip(entries, stamps)]
    logos = [quietly(lambda e=e: project_logo(e), None) if stamp else None for e, stamp in zip(entries, stamps)]
    logo_stamps = [file_stamp(logo[0]) if logo else None for logo in logos]
    etag = (
        '"'
        + hashlib.sha1(repr([registry.stamp] + stamps + trees + logo_stamps).encode("utf-8")).hexdigest()[:16]
        + '"'
    )

    def body():
        projects = []
        for entry, stamp, tree, logo in zip(entries, stamps, trees, logos):
            summary = quietly(lambda: summaries.summary(entry, stamp), None) if stamp else None
            projects.append(
                {
                    "id": entry.get("id"),
                    "name": entry.get("name"),
                    "path": entry["path"],
                    "missing": summary is None,
                    "summary": summary,
                    "worktrees": tree if summary else [],
                    # Relative to the dashboard, like the project's files; a new one shows as a new ETag
                    "logo": f"p/{entry.get('id')}/logo" if logo and summary else None,
                }
            )
        return to_json({"generated_at": now_iso(), "projects": projects}).encode("utf-8")

    return body, etag


SEARCH_QUERY_CAP = 200  # characters of the query that count
SEARCH_RESULTS_CAP = 200  # task IDs per project


def api_search(registry, summaries, query):
    """The body of /api/search?q=: per project, the IDs of the tasks.json tasks (not the archive) in which every word of
    the query appears, case-insensitively, as the dashboard's own search finds them (atm-lib's search_tasks); the
    project's name counts as part of every task's text. Read-only, like every route; `{}` for an empty query."""
    query = query[:SEARCH_QUERY_CAP]
    results = {}
    if not query.split():
        return to_json(results).encode("utf-8")
    for entry in registry.entries():

        def matches(entry=entry):
            p = served_file(entry, TASKS_FILE)
            stamp = file_stamp(p) if p else None
            if stamp is None or stamp[0] > MAX_SERVED_BYTES:
                return []
            return search_tasks(summaries.project(entry), query, also=str(entry.get("name") or ""))

        ids = quietly(matches, [])
        if ids:
            results[entry.get("id")] = ids[:SEARCH_RESULTS_CAP]
    return to_json(results).encode("utf-8")


def api_claims(summaries, entry):
    """(body, ETag) of a project's claims.json: the claims of all its worktrees (atm-lib's list_claims, from the shared
    claims store; each worktree's own snapshot has only the claims made there). None when there is no store yet."""
    stamp = file_stamp(summaries.store(entry))
    if stamp is None:
        return None

    def body():
        snapshot = {"generated_at": now_iso(), "claims": list_claims(summaries.project(entry))}
        return to_json(snapshot).encode("utf-8")

    return body, file_etag(stamp)


V1_TASKS = re.compile(r"/api/v1/projects/([^/]+)/tasks")
LIST_PARAMETERS = ("status", "label", "sprint")  # repeatable: ?status=todo&status=blocked
FLAG_PARAMETERS = ("backlog", "ready", "blocked", "in_pr", "with_user", "archived")
VALUE_PARAMETERS = ("type", "priority", "agent", "release", "component")


def task_filters(query):
    """atm-lib's Filters from a v1 route's query string (docs/layered-architecture.md §6.2): the list filters
    repeatable, the flags `true` or `1`, other parameters ignored."""
    given = parse_qs(query)
    filters = {name: given[name] for name in LIST_PARAMETERS if name in given}
    filters.update({name: given[name][0] for name in VALUE_PARAMETERS if name in given})
    filters.update({name: given[name][0].lower() in ("true", "1") for name in FLAG_PARAMETERS if name in given})
    return Filters(**filters)


def api_v1_tasks(summaries, entry, query):
    """(body, ETag) of GET /api/v1/projects/<p>/tasks: the project's tasks the filters keep, each with its derived
    values (atm-lib's task_views), and in the same snapshot the project's meta and how big its task data is. The ETag
    changes with tasks.json, the archive and the query."""
    stamps = [file_stamp(task_file(entry, name)) for name in (TASKS_FILE, "tasks-archive.json")]
    etag = '"' + hashlib.sha1(repr([stamps, query]).encode("utf-8")).hexdigest()[:16] + '"'

    def body():
        # Read tolerantly: the dashboard shows a malformed task file, with what is wrong with it (T-377)
        project = summaries.project(entry, tolerant=True)
        views = task_views(project, task_filters(query))
        snapshot = {
            "generated_at": now_iso(),
            "meta": project.store.document().get("meta") or {},
            "bytes": project.store.sizes()["tasks"],
            "tasks": views,
            "problems": read_problems(project),
        }
        hint = archive_hint_now(project) if not task_filters(query).archived else None
        if hint:
            snapshot["archive_hint"] = hint  # atm's suggestion to archive: the dashboard's footer notice (T-372)
        return to_json(snapshot).encode("utf-8")

    return body, etag


def api_worktrees(entry, store):
    """(body, ETag) of a project's worktrees.json: its git worktrees, main first, and for each linked one the tasks
    whose copy there differs from the main worktree's (or that only it has). The dashboard shows them all on one
    board, each task from the worktree where it got furthest. A linked worktree's copy of a task decided in another
    worktree (answered, reviewed: the claims store's `resolved`, T-204) is left out: it is stale until atm next runs
    there, and would otherwise outrank the decision."""
    trees = worktrees(entry["path"])
    stamps = [file_stamp(os.path.join(t["path"], TASK_DIR, TASKS_FILE)) for t in trees]
    etag = '"' + hashlib.sha1(repr([trees, stamps, file_stamp(store)]).encode("utf-8")).hexdigest()[:16] + '"'

    def tasks_of(tree):
        data = read_json(os.path.join(tree["path"], TASK_DIR, TASKS_FILE))
        tasks = data.get("tasks") if isinstance(data, dict) else None
        return [t for t in tasks or [] if isinstance(t, dict) and isinstance(t.get("id"), str)]

    def body():
        data = read_json(store)
        resolved = data.get("resolved") if isinstance(data, dict) and isinstance(data.get("resolved"), dict) else {}
        stale = {(r.get("worktree"), task_id) for task_id, r in resolved.items() if isinstance(r, dict)}
        listed_trees = []
        main = {}
        for tree in trees:
            if tree["main"]:
                main = {t["id"]: t for t in tasks_of(tree)}
                listed_trees.append(tree)
            else:
                differing = [
                    t for t in tasks_of(tree) if main.get(t["id"]) != t and (tree["path"], t["id"]) not in stale
                ]
                listed_trees.append({**tree, "tasks": differing})
        return to_json({"generated_at": now_iso(), "worktrees": listed_trees}).encode("utf-8")

    return body, etag


def cmd_serve_child(args):
    """The detached server process: serve on the first free port from `port`, record it, run until stopped."""
    import http.server
    import socketserver

    home, port = args.home, args.port
    record_path = os.path.join(home, SERVER_RECORD)
    with open(TEMPLATE, "rb") as f:
        page = f.read()
    # From the bytes it serves, not a second read of the file: a build writing the file in between would make whoami
    # report another build than the page has, and the page would offer a reload forever
    version, build = page_tags(page[:HEAD_BYTES].decode("utf-8", errors="replace"))
    page_headers = {"Content-Security-Policy": page_csp(page)}
    registry = Registry(home)
    summaries = Summaries()

    class Handler(http.server.BaseHTTPRequestHandler):
        server_version = "atm"
        # A connection that sends nothing (or trickles) is dropped instead of holding a thread forever (F18)
        timeout = 30

        def send(self, status, body=b"", content_type="application/json", etag=None, headers=None):
            self.send_response(status)
            if body or status == 200:
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
            if etag:
                self.send_header("ETag", etag)
            for name, value in {"Cache-Control": "no-store", **SECURITY_HEADERS, **(headers or {})}.items():
                self.send_header(name, value)
            self.end_headers()
            if body and self.command != "HEAD":
                self.wfile.write(body)

        def send_error_json(self, status, message):
            self.send(status, to_json({"error": message}).encode("utf-8"))

        def send_conditional(self, body_fn, etag, content_type="application/json", headers=None):
            if self.headers.get("If-None-Match") == etag:
                # Unchanged since the dashboard's last poll: it skips parsing and re-rendering (D-5.1)
                self.send(304, etag=etag, headers=headers)
                return
            self.send(200, body_fn(), content_type, etag, headers)

        def do_GET(self):
            # One bad request, or one project's broken files, gets a 500, never a dead handler thread with no answer
            try:
                self.handle_get()
            except Exception:  # noqa: BLE001
                try:
                    self.send_error_json(500, "internal error")
                except OSError:
                    pass

        def handle_get(self):
            port_now = self.server.server_address[1]
            if self.headers.get("Host") not in (f"localhost:{port_now}", f"127.0.0.1:{port_now}"):
                # A page on another site that points its own domain at 127.0.0.1 (DNS rebinding) is refused
                self.send_error_json(403, "unexpected Host header")
                return
            path = urlsplit(self.path).path
            if path == "/":
                self.send(200, page, "text/html; charset=utf-8", headers=page_headers)
            elif path == WHOAMI:
                info = {"atm": True, "pid": os.getpid(), "port": port_now, "version": version, "build": build}
                self.send(200, to_json(info).encode("utf-8"))
            elif path == "/api/projects":
                body, etag = api_projects(registry, summaries)
                self.send_conditional(body, etag)
            elif path == "/api/search":
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                self.send(200, api_search(registry, summaries, query))
            elif V1_TASKS.fullmatch(path):
                self.serve_v1_tasks(V1_TASKS.fullmatch(path).group(1), urlsplit(self.path).query)
            else:
                self.serve_project_file(path)

        def serve_v1_tasks(self, project, query):
            entry = registry.find(project)
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            p = served_file(entry, TASKS_FILE)
            stamp = file_stamp(p) if p else None
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp[0] > MAX_SERVED_BYTES:
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return
            body, etag = api_v1_tasks(summaries, entry, query)
            if self.headers.get("If-None-Match") == etag:
                self.send(304, etag=etag)
                return
            try:
                content = body()
            except AtmFailure as e:
                # A filter naming no such sprint or release (400), an unreadable task file (500)
                self.send_error_json(400 if e.code == 2 else 500, str(e))
                return
            self.send(200, content, "application/json", etag)

        do_HEAD = do_GET

        def serve_project_file(self, path):
            match = PROJECT_FILE.fullmatch(path)
            if not match:
                self.send_error_json(404, "not found")
                return
            entry = registry.find(match.group(1))
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            name = match.group(2)
            if served_file(entry, TASKS_FILE) is None:
                self.send_error_json(404, "not found")
                return
            if name == "logo":
                self.serve_logo(entry)
                return
            computed = None
            if name == "worktrees.json":
                computed = api_worktrees(entry, summaries.store(entry))
            elif name == "claims.json":
                computed = api_claims(summaries, entry)
            if computed:
                self.send_conditional(*computed)
                return
            p = served_file(entry, name)
            stamp = file_stamp(p) if p else None
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp[0] > MAX_SERVED_BYTES:
                # Each poll reads the whole file into memory: a runaway file is refused, not served
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return

            def read():
                with open(p, "rb") as f:
                    return f.read()

            self.send_conditional(read, file_etag(stamp))

        def serve_logo(self, entry):
            """The project's logo (logo.find_logo checks it is inside the project and small enough). The browser
            may keep it, asking again each time (no-cache) and getting a 304 while it is unchanged."""
            logo = project_logo(entry)
            stamp = file_stamp(logo[0]) if logo else None
            if stamp is None:
                self.send_error_json(404, "no logo")
                return

            def read():
                with open(logo[0], "rb") as f:
                    return f.read()

            self.send_conditional(read, file_etag(stamp), logo[1], {**LOGO_HEADERS, "Cache-Control": "no-cache"})

        def not_allowed(self):
            self.send_error_json(405, "only GET and HEAD")

        do_POST = do_PUT = do_DELETE = do_PATCH = do_OPTIONS = not_allowed

    class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
        daemon_threads = True
        # A restarted server takes its port back while the old one's connections linger in TIME_WAIT. On macOS the
        # same option would also let it share a port another server listens on, hence port_in_use() first.
        allow_reuse_address = True

    class Server6(Server):
        address_family = socket.AF_INET6

    # Browsers, curl and urllib try [::1] before 127.0.0.1 for `localhost`: the server listens on both, so no other
    # local user can take [::1] on our port and answer for us (docs/security-review.md F10). Without IPv6, 127.0.0.1
    # alone is enough: nobody can listen on [::1] then.
    server = server6 = None
    for candidate in range(port, port + PORT_TRIES):
        if port_in_use(candidate):
            continue
        try:
            server = Server(("127.0.0.1", candidate), Handler)
        except OSError:
            continue
        try:
            server6 = Server6(("::1", candidate), Handler)
        except OSError as e:
            if e.errno == errno.EADDRINUSE:
                server.server_close()
                server = None
                continue
        break
    if server is None:
        sys.stderr.write(f"atm: no free port in {port}-{port + PORT_TRIES - 1}\n")
        return
    if server6 is not None:
        threading.Thread(target=server6.serve_forever, daemon=True).start()
    record = {
        "pid": os.getpid(),
        "port": server.server_address[1],
        "version": version,
        "build": build,
        "plugin_root": PLUGIN_ROOT,
        "started_at": now_iso(),
    }
    write_json_atomic(record_path, record)

    def stop(signum, frame):
        if (read_json(record_path) or {}).get("pid") == os.getpid():
            os.remove(record_path)
        os._exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    server.serve_forever()


# ==== cli/query.py ====
# The read commands, through atm-lib (docs/layered-architecture.md §9, step 3): list, deps, status, summary.
#
# Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3).

def cmd_list(args):
    if args.derived:
        # JSON either way: it is for tools (the dashboard's tests take atm's own values from it)
        if args.tolerant:
            project = open_project(os.getcwd(), tolerant=True)
            views = task_views(project, args)
            meta = project.store.document().get("meta") or {}  # without what doesn't work, as atm serve sends it
            read = {"meta": meta, "tasks": views, "problems": read_problems(project)}
            hint = archive_hint_now(project) if not args.archived else None
            if hint:
                read["archive_hint"] = hint
            out(to_json(read))
        else:
            out(to_json(task_views(open_project(os.getcwd()), args)))
        return
    found = list_matching(open_project(os.getcwd()), args)
    vocab, tasks = found["vocab"], found["tasks"]
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
        return
    if not tasks:
        out("No tasks match.")
        return
    for line in list_lines(vocab, tasks):
        out(line)


def cmd_deps(args):
    deps = task_dependencies(open_project(os.getcwd()), args.id)
    if args.json:
        out(to_json(deps))
        return
    line = lambda items: ", ".join(f"{i['id']} ({i['status']})" for i in items) or "nothing"
    out(f"{args.id} waits for: {line(deps['waits_for'])}")
    out(f"{args.id} is waited on by: {line(deps['waited_on_by'])}")


def cmd_summary(args):
    """The project's summary as atm serve lists it in api/projects (3.2); the dashboard's dev server plays atm serve
    with it, so its numbers and Next up come from atm-cli too. JSON either way: it is for tools."""
    out(to_json(project_summary(open_project(os.getcwd()))))


def cmd_status(args):
    status = status_report(
        open_project(os.getcwd()),
        Filters(sprint=args.sprint, backlog=args.backlog, release=args.release, component=args.component),
    )
    result = status["report"]
    if args.json:
        out(to_json(result))
        return
    sprint, stranded = result["sprint"], result.get("open_in_closed_sprints")
    pairs = lambda d: ", ".join(f"{k} {v}" for k, v in d.items())
    elsewhere = lambda i: f" [in another worktree, {i['branch'] or 'detached'}]" if "branch" in i else ""
    if sprint:
        details = [f"{sprint['done']} of {sprint['tasks']} done"] + [
            f"{k} {sprint[k]}" for k in ("due", "release") if sprint.get(k)
        ]
        out(f"sprint: {sprint['id']} (active): {' · '.join(details)}")
        if sprint.get("goal"):
            out(f"  goal: {sprint['goal']}")
        if sprint["review_pending"]:
            out(f"  waiting for the sprint review: {', '.join(sprint['review_pending'])} (/atm:close-sprint)")
    elif status["has_sprints"]:
        out("sprint: none active (plan one with /atm:plan-sprint)")
    if stranded:
        n = len(stranded)
        out(
            f"warning: {n} open task{'' if n == 1 else 's'} in closed sprints, which the loop won't take: "
            + ", ".join(f"{t['id']} ({t['sprint']})" for t in stranded)
            + " (atm move <id> --to backlog or --to active)"
        )
    out(f"{result['tasks']} tasks: {pairs(result['by_status'])}")
    if status["has_sprints"]:
        out(f"by sprint: {pairs(result['by_sprint'])}")
    out(f"by type: {pairs(result['by_type'])}")
    if result["by_label"]:
        out(f"by label: {pairs(result['by_label'])}")
    for title, items, extra in (
        ("in progress", result["in_progress"], lambda i: f" ({i['by']})" if i["by"] else ""),
        ("in review", result["in_review"], lambda i: (f" (since {i['since']})" if i["since"] else "") + elsewhere(i)),
        ("blocked", result["blocked"], lambda i: (f" (on {i['waiting_on']})" if i["waiting_on"] else "") + elsewhere(i)),
        ("waiting on dependencies", result["waiting_on_dependencies"], lambda i: f" (on {', '.join(i['on'])})"),
    ):
        if items:
            out(f"{title}: " + "; ".join(f"{i['id']} {i['name']}{extra(i)}" for i in items))
    nxt = result["next"]
    out(f"next: {nxt['id']} {nxt['name']}" if nxt else f"next: none ({result['next_reason']})")
    largest = ", ".join(f"{i['id']} {i['bytes'] // 1024 or 1} KB" for i in result["largest_tasks"])
    out(f"file: {result['file_bytes'] // 1024} KB" + (f"; largest tasks: {largest}" if largest else ""))
    if result["archived"]:
        out(f"archive: {result['archived']} tasks, {result['archive_bytes'] // 1024} KB")
    hint = (result.get("suggest") or {}).get("archive")
    if hint:
        out(hint["text"])


# ==== cli/status.py ====
# The status commands and `atm note`, through atm-lib (docs/layered-architecture.md §9, step 8).

def cmd_status_change(command):
    def run(args):
        project = open_project(os.getcwd(), agent_identity(args.agent))
        result = change_task_status(
            project,
            command,
            args.id,
            status=args.status,
            note=getattr(args, "note", None),
            reason=getattr(args, "reason", None),
            commit=getattr(args, "commit", None),
            on=getattr(args, "on", None),
            questions=getattr(args, "question", None) or [],
            answers=getattr(args, "answer", None) or [],
            resolution=getattr(args, "resolution", None),
            duplicate_of=getattr(args, "duplicate_of", None),
        )
        report_expired(result["expired"])
        task, hint, stranded = result["task"], result["hint"], result["stranded"]
        if args.json:
            extra = {"unblocked": result["unblocked"]} if command == "done" else {}
            if hint:
                extra["suggest"] = {"archive": hint}
            if stranded:
                extra["hint"] = stranded
            out(to_json({**task, "claim": result["claim"], **extra}))
            return
        # New lines only after the existing one (CLI-3.2)
        out(f"{task['id']}: {result['previous']} -> {task['status']}")
        if stranded:
            out(f"  {stranded}")
        if command == "start":
            out(describe_task(result["document"], task))
        if result["unblocked"]:
            out("Now ready: " + ", ".join(result["unblocked"]))
        if hint:
            out(hint["text"])

    return run


def cmd_note(args):
    """Add a note to a task, whatever its status (DM-4)."""
    task = add_task_note(open_project(os.getcwd(), agent_identity(args.agent)), args.id, args.text)
    if args.json:
        out(to_json(task["notes"][-1]))
    else:
        out(f"{task['id']}: note added ({len(task['notes'])} in all)")


# ==== cli/claims.py ====
# `atm next`, `heartbeat`, `run` and `claims`, through atm-lib (docs/layered-architecture.md §9, step 9).

def cmd_next(args):
    sel = selection(
        sprints=args.sprint,
        tasks=[i.strip() for i in (args.tasks or "").split(",") if i.strip()],
        all_tasks=args.all,
        backlog=args.backlog,
        task_type=args.type,
    )
    project = open_project(os.getcwd(), agent_identity(args.agent))
    result = next_task_for(project, sel, with_user=args.with_user, claim=args.claim, status=args.status)
    report_expired(result["expired"])
    task, claim, reason = result["task"], result["claim"], result["reason"]
    if args.json:
        out(to_json({"task": task, "claim": claim, "reason": reason}))
    elif task:
        verb = "Claimed" if claim else "Next"
        where = task.get("sprint") or "backlog"
        out(f"{verb}: {task['id']} {task.get('name', '')} ({where}, {task.get('type')})")
    else:
        out(f"No task available: {reason}.")
    if not task:
        raise AtmExit(3)


def hook_input():
    """The JSON a Claude Code hook gets on stdin ({session_id, cwd, ...}), or {}."""
    if sys.stdin is None or sys.stdin.isatty():
        return {}
    try:
        value = json.loads(sys.stdin.read() or "{}")
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {}


def auto_heartbeat(args):
    """`heartbeat --auto`, run by the plugin's PostToolUse hook after every tool call (CLI-6.2): renew this agent's
    live claims once their last renewal is ten minutes old. It never prints and never fails, so a tool call is never
    disturbed."""
    try:
        payload = hook_input()
        cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) and os.path.isdir(payload["cwd"]) else os.getcwd()
        if payload.get("session_id") and not (args.agent or os.environ.get("ATM_AGENT")):
            # The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
            agent = session_identity(payload["session_id"], cwd)
        else:
            agent = agent_identity(args.agent)
        paths = find_paths(cwd)
        if not paths["tasks_exists"]:
            return
        try:
            register_if_needed(paths)  # list the project on the dashboard (one read of the registry when it is)
        except Exception:
            pass
        renew_due_claims(open_project(cwd, agent))
    except Exception:  # a hook must never break the tool call it follows
        return


def cmd_heartbeat(args):
    if args.auto:
        auto_heartbeat(args)
        return
    if not args.id:
        raise Invalid("atm heartbeat needs a task ID (or --auto)")
    result = renew_lease(open_project(os.getcwd(), agent_identity(args.agent)), args.id)
    claim = result["claim"]
    if args.json:
        out(to_json(claim))
    else:
        out(f"{args.id}: lease extended to {claim['lease_expires_at']}")


def cmd_run(args):
    """`atm run <id> -- <command>`: run a long command (a full test suite, a build, a wait on CI) while keeping the
    claim on <id> alive. The plugin's hook renews claims only when the agent makes a tool call, and an agent waiting
    for a command makes none, so a wait longer than the lease would hand the task to someone else (T-203). The claim
    is renewed at the start (so a wrong ID or someone else's claim fails before anything runs) and then three times per
    lease; the command's output passes through unchanged, and atm exits with its exit code."""
    if not args.child:
        raise Invalid("atm run needs a command after --: atm run <id> -- <command> [args...]")
    if args.every is not None and args.every <= 0:
        raise Invalid("--every must be more than 0 seconds")
    project = open_project(os.getcwd(), agent_identity(args.agent))
    renew = lambda: renew_lease(open_project(os.getcwd(), project.actor), args.id, expire=False)
    minutes = renew()["minutes"]
    every = args.every if args.every is not None else max(60.0, minutes * 60 / 3)
    try:
        child = subprocess.Popen(args.child)
    except OSError as e:
        raise AtmFailure(f"can't run {args.child[0]}: {e.strerror}", 127)
    # Renewed from this (the main) thread between waits: the lock uses signals, which only work here
    renewing = True
    while True:
        try:
            code = child.wait(timeout=every)
            break
        except subprocess.TimeoutExpired:
            pass
        except KeyboardInterrupt:  # the command got the same interrupt; wait for it to end
            continue
        if renewing:
            try:
                renew()
            except Exception as e:  # never stops the command; say so once and stop renewing
                sys.stderr.write(f"atm run: stopped renewing the claim on {args.id}: {e}\n")
                renewing = False
    if code:
        raise AtmExit(128 - code if code < 0 else code)  # a signal -N ends like a shell's 128+N


def cmd_claims(args):
    result = live_claims(open_project(os.getcwd()))
    live = result["claims"]
    if args.json:
        out(to_json(live))
        return
    if not live:
        out("No live claims.")
        return
    current = now()
    for c in live:
        left = format_duration((parse_iso(c["lease_expires_at"]) - current).total_seconds())
        out(f"{c['task']}  {c.get('agent')}  {c.get('branch') or '-'}  {left} left  ({c.get('worktree')})")


# ==== cli/edit.py ====
# `atm add`, `atm edit` and `atm check`, through atm-lib (docs/layered-architecture.md §9, step 10). The files these
# options name (--json-file, --description-file) are read here: atm-lib takes their content.

def suggestion_line(vocab, task):
    """`atm add` / `edit --add-file` (SR-2.7): the components the task's files suggest that it doesn't have yet, as a
    line with the command that sets them; None when there are none (or the project has no components)."""
    missing = [c for c in suggest_components(vocab, task.get("files") or []) if c not in (task.get("components") or [])]
    if not missing:
        return None
    adds = " ".join(f"--add-component {c}" for c in missing)
    return f"  {task['id']}: its files suggest the component{'s' if len(missing) > 1 else ''} {', '.join(missing)} (atm edit {task['id']} {adds})"


def read_entries(path):
    """The tasks of an `atm add --json-file`: a list of task objects, or {"tasks": [...]}."""
    value = read_json(path)
    if isinstance(value, dict):
        value = value.get("tasks")
    if not isinstance(value, list) or not all(isinstance(e, dict) for e in value):
        raise Invalid(f"{path}: expected a JSON list of tasks (or {{\"tasks\": [...]}})")
    return value


def description_of(args):
    """--description, or the content of --description-file."""
    if getattr(args, "description_file", None):
        with open(args.description_file, encoding="utf-8") as f:
            return f.read()
    return args.description


def cmd_add(args):
    if args.json_file:
        entries = read_entries(args.json_file)
    else:
        entries = [
            {
                "name": args.name,
                "type": args.type,
                "priority": args.priority,
                "labels": args.label,
                "description": description_of(args),
                "dependencies": args.dep,
                "files": args.file,
                "requirements": args.requirement,
                "sprint": args.sprint,
                "release": args.release,
                "components": "auto" if args.components == "auto" else args.component,
                "review": args.review,
                "with": args.with_,
                "related": args.related,
            }
        ]
    result = add_new_tasks(open_project(os.getcwd(), agent_identity(args.agent)), entries)
    if args.json:
        out(to_json(result["tasks"]))
        return
    for task in result["tasks"]:
        out(f"Added {task['id']} {task['name']}")
        line = suggestion_line(result["vocab"], task)
        if line:
            out(line)


def cmd_edit(args):
    changes = Changes(**{**vars(args), "description": description_of(args)})
    result = edit_existing_task(open_project(os.getcwd(), agent_identity(args.agent)), args.id, changes)
    task, changed = result["task"], result["changed"]
    if args.json:
        out(to_json(task))
        return
    out(f"{task['id']}: changed {', '.join(changed)}" if changed else f"{task['id']}: nothing changed")
    # New files may put the task in components it isn't in yet (SR-2.7): suggested, not set
    line = suggestion_line(result["vocab"], task) if args.add_file else None
    if line:
        out(line)


def cmd_check(args):
    result = check_project(open_project(os.getcwd()), files=args.files)
    errors, warnings = result["errors"], result["warnings"]
    if args.json:
        out(to_json({"errors": errors, "warnings": warnings}))
    else:
        for e in errors:
            out(f"error: {e}")
        for w in warnings:
            out(f"warning: {w}")
        if not errors:
            count = result["count"]
            out(f"tasks.json is fine ({count} tasks" + (f", {len(warnings)} warnings)" if warnings else ")"))
    if errors:
        raise AtmExit(1)


# ==== cli/components.py ====
# `atm components`, through atm-lib (docs/layered-architecture.md §9, step 10).

def moved_text(label, from_label, assign, by_files):
    """What --from-label and --assign did, e.g. "6 tasks from the label atm-cli, 2 more by their files"."""
    parts = []
    if label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {label}")
    if assign:
        parts.append(f"{len(by_files)} {'more ' if label else ''}by their files")
    return ", ".join(parts)


def show_list(project, as_json):
    listed = list_components(project)
    if as_json:
        out(to_json(listed))
        return
    if not listed:
        out("No components yet: atm components add <id> --path GLOB.")
    for c in listed:
        details = [f"“{c['label']}”"] if c.get("label") else []
        details.append(", ".join(c["paths"]) if c["paths"] else "no paths")
        if c.get("review"):
            details.append(f"review {c['review']}")
        out(f"{c['id']} · {' · '.join(details)}: {c['tasks']} task{'' if c['tasks'] == 1 else 's'}")


def cmd_components(args):
    project = open_project(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise Invalid(f"unexpected argument '{args.id}'")
        for option in ("label", "path", "add_path", "review", "from_label"):
            if getattr(args, option) is not None:
                raise Invalid(f"--{option.replace('_', '-')} is for atm components add or edit")
        if args.assign:
            raise Invalid("--assign is for atm components add or edit")
        show_list(project, args.json)
        return
    if not args.id:
        raise Invalid(f"atm components {args.action} needs the component's ID")
    if args.action == "add":
        if args.add_path is not None:
            raise Invalid("--add-path is for atm components edit (add takes --path)")
        result = add_component(project, args.id, args.label, args.path, args.review, args.from_label, args.assign)
        if args.json:
            out(to_json(result))
            return
        text = moved_text(args.from_label, result["from_label"], args.assign, result["by_files"])
        out(f"Added the component {args.id}" + (f": {text}." if text else "."))
        return
    if args.path is not None:
        raise Invalid("--path is for atm components add (edit takes --add-path)")
    result = edit_component(project, args.id, args.label, args.add_path, args.review, args.from_label, args.assign)
    if args.json:
        out(to_json(result))
        return
    moved = moved_text(args.from_label, result["from_label"], args.assign, result["by_files"])
    text = ", ".join(([f"changed {', '.join(result['changed'])}"] if result["changed"] else []) + [t for t in [moved] if t])
    out(f"{args.id}: {text or 'nothing changed'}")


# ==== cli/sprints.py ====
# `atm sprints`, `atm move` and `atm backlog`, through atm-lib (docs/layered-architecture.md §9, step 11).

NONE_ACTIVE = "No sprint is active now: start the next one (atm sprints start <name>), or plan one with /atm:plan-sprint."


def describe_sprint(s):
    details = [s["state"]] + [f"{key} {s[key]}" for key in ("release", "due") if s.get(key)]
    counts = ", ".join(f"{kind} {n}" for kind, n in s["by_kind"].items())
    line = f"{s['id']} · {' · '.join(details)}: {s['tasks']} task{'' if s['tasks'] == 1 else 's'}"
    line += f" ({counts})" if counts else ""
    if s.get("review_pending"):
        line += f"; {s['review_pending']} waiting for the sprint review"
    return line + (f"\n  goal: {s['goal']}" if s.get("goal") else "")


def show_sprints(project, include_closed, as_json):
    listed = list_sprints(project, include_closed)
    sprints, backlog = listed["sprints"], listed["backlog"]
    if as_json:
        out(to_json({"sprints": sprints, "backlog": backlog}))
        return
    if not sprints:
        out("No open sprints (--all lists the closed ones)." if listed["has_sprints"] else "No sprints yet: atm sprints add creates one.")
    for s in sprints:
        out(describe_sprint(s))
    out(f"backlog: {backlog} task{'' if backlog == 1 else 's'} (atm backlog)")


def show_close(result, as_json):
    sprint, created, target = result["sprint"], result["created"], result["target"]
    if as_json:
        out(to_json({"sprint": sprint, "created": created}))
        return
    report = sprint["report"]
    line = f"Closed the sprint “{sprint['id']}”: {len(report['done'])} done"
    if report["carried"]:
        where = "the backlog" if target is None else f"“{target}”" + (" (new, planned)" if created else "")
        line += f", {len(report['carried'])} carried to {where} ({', '.join(report['carried'])})"
    if report["added"]:
        line += f", {len(report['added'])} added mid-sprint"
    out(line + ".")
    if result["was_active"]:
        out(NONE_ACTIVE)


def show_delete(result, as_json):
    sprint, state, moved, target = result["sprint"], result["state"], result["moved"], result["target"]
    if as_json:
        out(to_json({"deleted": sprint["id"], "state": state, "moved": moved, "to": target or "backlog"}))
        return
    line = f"Deleted the sprint “{sprint['id']}” ({state})"
    if moved:
        where = "the backlog" if target is None else f"“{target}”"
        line += f": {len(moved)} task{'' if len(moved) == 1 else 's'} moved to {where} ({', '.join(moved)})"
    out(line + ".")
    if state == "active":
        out(NONE_ACTIVE)


def cmd_sprints(args):
    project = open_project(os.getcwd(), agent_identity(getattr(args, "agent", None)))
    if args.action in (None, "list"):
        if args.name is not None:
            raise Invalid(f"unexpected argument '{args.name}'")
        show_sprints(project, args.all, args.json)
        return
    if args.action == "add":
        sprint = add_sprint(project, args)
        out(to_json(sprint) if args.json else f"Added the sprint “{sprint['id']}” (planned).")
        return
    if not args.name:
        raise Invalid(f"atm sprints {args.action} needs a sprint name")
    if args.action == "edit":
        result = edit_sprint(project, args.name, args)
        sprint, changed = result["sprint"], result["changed"]
        if args.json:
            out(to_json(sprint))
        else:
            out(f"{sprint['id']}: changed {', '.join(changed)}" if changed else f"{sprint['id']}: nothing changed")
    elif args.action == "start":
        sprint = start_sprint(project, args.name)
        out(to_json(sprint) if args.json else f"The sprint “{sprint['id']}” is now active.")
    elif args.action == "close":
        show_close(close_sprint(project, args.name, args), args.json)
    else:
        show_delete(delete_sprint(project, args.name, args), args.json)


def cmd_move(args):
    moved = move_tasks(open_project(os.getcwd(), agent_identity(args.agent)), args.ids, args.to)
    if args.json:
        out(to_json(moved))
        return
    for m in moved:
        before, after = m["from"] or "backlog", m["to"] or "backlog"
        out(f"{m['id']}: {before} -> {after}" if before != after else f"{m['id']}: already in {after}")


def cmd_backlog(args):
    found = backlog_tasks(open_project(os.getcwd()), finished=args.all)
    vocab, tasks = found["vocab"], found["tasks"]
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
    elif not tasks:
        out("The backlog is empty.")
    else:
        for line in list_lines(vocab, tasks):
            out(line)


# ==== cli/releases.py ====
# `atm releases`, through atm-lib (docs/layered-architecture.md §9, step 11).

def describe_release(r):
    details = [r["state"]] + ([f"target {r['target']}"] if r.get("target") else [])
    if r.get("released_at"):
        details.append(f"released {r['released_at'][:10]}")
    if isinstance(r.get("pr"), dict) and r["pr"].get("number"):
        details.append(f"PR #{r['pr']['number']}")
    title = r["id"] + (f" “{r['name']}”" if r.get("name") else "")
    kinds = ", ".join(f"{k} {n}" for k, n in r["by_kind"].items())
    stages = ", ".join(f"{s.replace('_', ' ')} {n}" for s, n in r["by_stage"].items())
    line = f"{title} · {' · '.join(details)}: {r['tasks']} task{'' if r['tasks'] == 1 else 's'}"
    return line + (f" ({kinds})" if kinds else "") + (f"; {stages}" if stages else "")


def describe_notes(notes):
    """The draft as Markdown: a heading per type, a line per task; what's not done yet at the end."""
    release = notes["release"]
    lines = [f"# {release['id']}" + (f" “{release['name']}”" if release.get("name") else "")]
    if not notes["groups"]:
        lines += ["", "Nothing done in it yet."]
    for group in notes["groups"]:
        lines += ["", f"## {group['label']}", ""]
        for t in group["tasks"]:
            lines.append(f"- **{t['id']}** {t['name']}" + (f": {t['outcome']}" if t["outcome"] else ""))
    if notes["open"]:
        lines += ["", "## Not done yet", ""]
        lines += [f"- {t['id']} {t['name']} ({t['status']})" for t in notes["open"]]
    return lines


def cmd_releases(args):
    project = open_project(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise Invalid(f"unexpected argument '{args.id}'")
        listed = list_releases(project, args.all)
        if args.json:
            out(to_json(listed["releases"]))
            return
        if not listed["releases"]:
            out(
                "No open releases (--all lists the archived ones)."
                if listed["has_releases"]
                else "No releases yet: atm releases add <R>."
            )
        for r in listed["releases"]:
            out(describe_release(r))
        return
    if not args.id:
        raise Invalid(f"atm releases {args.action} needs a release ID")
    if args.pr is not None and args.action != "add":
        raise Invalid("--pr is for atm releases add")
    if args.released_at is not None and args.action != "add":
        raise Invalid("--released-at is for atm releases add")
    if args.action == "add":
        result = add_release(project, args.id, args)
        if args.json:
            out(to_json(result))
            return
        release, given, prs = result["release"], result["tasks"], args.pr or []
        line = f"Added the release “{args.id}” ({release['state']})"
        if given:
            numbers = ", ".join(f"#{n}" for n in prs)
            line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
        out(line + ".")
    elif args.action == "edit":
        result = edit_release(project, args.id, args)
        release, changed = result["release"], result["changed"]
        if args.json:
            out(to_json(release))
        else:
            out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")
    elif args.action == "archive":
        release = archive_release(project, args.id)
        out(to_json(release) if args.json else f"Archived the release “{release['id']}”.")
    else:
        notes = release_notes(project, args.id)
        if args.json:
            out(to_json(notes))
            return
        for line in describe_notes(notes):
            out(line)


# ==== cli/archive.py ====
# `atm archive` and `atm unarchive`, through atm-lib (docs/layered-architecture.md §9, step 12).

def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())


def cmd_archive(args):
    result = archive_finished(open_project(os.getcwd()), args.sprint, args.before, args.keep_days, args.dry_run)
    ids, dropped, kept, sizes = result["moved"], result["duplicates_dropped"], result["kept"], result["sizes"]
    shown = {
        "moved": ids,
        "dry_run": bool(args.dry_run),
        "tasks_kb": sizes["tasks"] // 1024,
        "archive_kb": sizes["archive"] // 1024,
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if args.json:
        out(to_json(shown))
        return
    if dropped and not args.dry_run:
        out(f"Kept {describe_ids(dropped)} in tasks.json and dropped the archive's copy (an interrupted move).")
    if not ids:
        out("Nothing to archive.")
    elif args.dry_run:
        out(f"Would archive {len(ids)} task(s): {describe_ids(ids)}.")
    else:
        out(
            f"Archived {len(ids)} task(s): {describe_ids(ids)}. "
            f"tasks.json is now {shown['tasks_kb']} KB, tasks-archive.json {shown['archive_kb']} KB."
        )
    # What stays, and why (T-352)
    if kept:
        out(f"{'Would keep' if args.dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}.")


def cmd_unarchive(args):
    result = unarchive_tasks(open_project(os.getcwd()), args.ids)
    again = result["moves_back"]
    if args.json:
        out(to_json(result))
        return
    out(f"Brought back {', '.join(args.ids)} from the archive.")
    if again:
        out(
            f"{describe_ids(again)} {'is' if len(again) == 1 else 'are'} finished: the next `atm archive` moves "
            f"{'it' if len(again) == 1 else 'them'} back, unless reopened (`atm reopen <id> --reason …`)."
        )


# ==== cli/show.py ====
# `atm show`: tasks in full, through atm-lib (docs/layered-architecture.md §9, step 1).

def cmd_show(args):
    views = show_tasks(open_project(os.getcwd()), args.ids)
    if args.json:
        # An archived task says so (T-353)
        out(to_json([{**v["task"], "archived": True} if v["archived"] else v["task"] for v in views]))
    else:
        out("\n\n".join(render_task(v) for v in views))


# ==== cli/code.py ====
# `atm workflow`, `atm commit`, `atm pr` and `atm merged`, through atm-lib (docs/layered-architecture.md §9, step 13).

def describe_workflow(workflow, base_default):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm workflow set direct|pull-request`."
    trailers = workflow.get("trailers")
    lines = [
        f"workflow: {workflow['type']}",
        "  base: " + (workflow["base"] if workflow.get("base") else f"{base_default()} (the remote's default branch)"),
        f"  verify before pushing: {'yes' if workflow['verify'] else 'no'}",
        f"  CI fix rounds: {workflow['max_rounds']}",
        f"  commit message: {workflow['commit_template']}",
        "  trailers: "
        + (
            "none (Co-Authored-By lines are removed)"
            if trailers == "forbid"
            else ("required: " + "; ".join(trailers["require"]) if isinstance(trailers, dict) else "as the agent writes them")
        ),
    ]
    return "\n".join(lines)


def cmd_workflow(args):
    project = open_project(os.getcwd())
    if args.action is None:
        workflow = workflow_settings(project)
    else:
        workflow = set_workflow(
            project,
            args.type,
            base=args.base,
            verify=None if args.verify is None else args.verify == "true",
            max_rounds=args.max_rounds,
            commit_template=args.commit_template,
            trailers=args.trailers,
            trailer=args.trailer,
        )
    if args.json:
        out(to_json(workflow or {"type": None}))
    else:
        out(describe_workflow(workflow, project.vcs.default_base))


def cmd_commit(args):
    """Commit exactly the files the agent names plus tasks.json, then record the commit on the task."""
    project = open_project(os.getcwd(), agent_identity(args.agent))
    if args.record is not None:
        if args.file or args.message is not None:
            raise Invalid("--record records an existing commit: no --file or --message")
        result = record_commit(project, args.id, args.record)
        if args.json:
            shown = {k: result[k] for k in ("task", "commit", "recorded")}
            out(to_json({**shown, **({"hint": result["hint"]} if result["hint"] else {})}))
            return
        out(f"{result['task']}: recorded {result['commit'][:7]} {result['summary']}".rstrip())
        if result["hint"]:
            out(f"  {result['hint']}")
        return
    result = commit_task(project, args.id, args.file, os.getcwd(), args.message)
    if args.json:
        out(to_json(result))
    else:
        out(f"{result['task']}: committed {result['commit'][:7]} {result['message'].splitlines()[0]}")
        out("  " + ", ".join(result["files"]))


def cmd_pr(args):
    """Record the PR that contains the tasks (DM-4.3: the tasks are then In PR)."""
    ids = record_pr(open_project(os.getcwd(), agent_identity(args.agent)), args.ids, args.number, args.url)
    if args.json:
        out(to_json({"tasks": ids, "pr": {"number": args.number, "url": args.url}}))
    else:
        out(f"{', '.join(ids)}: in PR #{args.number} ({args.url})")


def cmd_merged(args):
    """Record that a PR was merged, on every task that records it (DM-4.3: the tasks are then Merged)."""
    result = record_merge(open_project(os.getcwd(), agent_identity(args.agent)), args.pr, args.at)
    if args.json:
        out(to_json(result))
        return
    ids = result["tasks"]
    out(f"PR #{args.pr} merged: {', '.join(ids)}" if ids else f"PR #{args.pr}: already recorded as merged")
    for release_id in result["released"]:
        out(f"Released {release_id}: all its tasks are merged in PR #{args.pr}.")
    for release_id, waiting in result["pending"].items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{args.pr}.")


# ==== cli/main.py ====
# The command line: one argparse parser, a subparser per command, and main().
#
# Every command takes --json. Usage errors keep atm's format ("atm: <message>", exit code 2) followed by the command's
# usage line; with --json, every error is {"error": "...", "code": N} on stderr, with the same exit code.

USAGE = """usage: atm [--dir PATH] <command> [options]

  --dir PATH     run as if started in PATH: on the project there (relative paths in the command count from it)

commands:
  version        print the plugin version
  paths [--json] print where this project's task files are
  migrate [--json]
                 move tasks.json from docs/ or the root into .agentic-task-manager/,
                 and set up the folder (.gitignore, current dashboard)
  serve [start|stop|status] [--open] [--all] [--port N] [--dir PATH] [--refresh]
                 run the dashboard server in the background: one per user, for all registered projects
                 (start registers this project, or the one at PATH, and prints its URL; --all the overview's;
                 --refresh only restarts a running server whose dashboard is older than this plugin's)
  status         the overview: counts, what is in progress, in review or blocked, the next task
  summary        the project's numbers as atm serve gives them to the dashboard (JSON)
  show <id>...   tasks in full (description, dependencies, files, notes)
  list [--status S] [--sprint S | --backlog] [--type T] [--priority P] [--label L] [--agent A] [--ready]
       [--blocked] [--in-pr] [--with-user] [--derived]
                 one line per task: ID, status, priority, labels, name (--derived: each task in full with what atm
                 derives for it, as JSON)
  deps <id>      what a task waits for, and what waits for it
  start <id>     mark a task in_progress and claim it (its dependencies must be done); prints the task
  review <id> [--note TEXT]
                 ask for a review of a finished task: in_review, and its claim is released
  done <id> [--note TEXT] [--commit SHA] [--resolution R]
                 mark an in_progress or in_review task done and drop its claim; the note is a one-line
                 outcome (for a task in review, done means the review passed); prints the tasks now ready
  reject <id> --reason TEXT
                 send a task in review back to todo, with what needs to change
  approve <id> [--note TEXT] | request-changes <id> --reason TEXT
                 decide a review: the task stays in review, approved or with changes asked for, until an agent takes
                 it back (atm next offers it first, to the agent that worked on it for one lease length)
  block <id> --on user|agent (--question TEXT)... [--reason TEXT]
                 a task that can't go on until someone decides: blocked, its claim released
  unblock <id> [--answer TEXT]...
                 back to todo, with the questions and answers kept as a note
  reopen <id> --reason TEXT
                 a done or skipped task back to todo (done by mistake, or not finished after all)
  skip <id> --reason TEXT [--resolution wont_do|obsolete|duplicate] [--duplicate-of ID]
                 a todo or blocked task that isn't needed: skipped, with the reason as a note
  fail <id> [--reason TEXT]
                 put an in_progress task back to todo, recording why (a failure note)
  release <id> [--note TEXT]
                 put an in_progress task back to todo; the note is a handoff for the next agent
  note <id> TEXT add a note to a task (at most 500 characters)
  add --name N --type T [--sprint S] [--priority P] [--label L]... [--description D] [--dep ID]... [--file F]...
                 add a task with the next ID (in the backlog without --sprint); add --json-file F adds many
  edit <id> [--name] [--description] [--sprint S | --backlog] [--type] [--priority]
            [--add-dep/--remove-dep ID] [--add-file/--remove-file F] [--add-label/--remove-label L]
                 change a task's definition (its status changes only through the status commands)
  check          check tasks.json: schema, vocabularies, dependencies (exit code 1 on errors)
  archive [--sprint S]... [--before DATE] [--keep-days N] [--dry-run]
                 move finished tasks to tasks-archive.json, to keep tasks.json small
  unarchive <id>...
                 bring archived tasks back into tasks.json
  workflow [set direct|pull-request] [--base B] [--verify true|false] [--max-rounds N] ...
                 print the project's workflow (meta.workflow), or set it
  commit <id> --file F [--file F]... [--message TEXT]
                 commit exactly those files plus tasks.json (message from meta.workflow.commit_template),
                 and record the commit on the task
  pr <id>... --number N --url URL
                 record the PR that contains the tasks
  merged --pr N [--at TIME]
                 record that PR N was merged, on its tasks
  next [--sprint S | --tasks T-1,T-5 | --all | --backlog] [--type T] [--with-user] [--claim] [--json]
                 the task /atm:loop-tasks works on next: of the active sprint by default (every task without
                 sprints), tasks back from review first, then the most urgent priority, then lowest ID; --claim
                 also starts and claims it, atomically (exit code 3 if there is none)
  heartbeat <id> extend your claim's lease (heartbeat --auto: the plugin's hook renews your claims for you)
  run <id> [--every SECONDS] -- <command> [args...]
                 run a long command (tests, a build, a wait on CI) while keeping your claim on <id> alive; the
                 hook renews claims only between tool calls. Exits with the command's exit code
  init [--meta-file F]
                 give tasks.json a meta.id if it has none, and register the tasks.json merge driver in this
                 clone (migrate registers the driver too); --meta-file sets meta (project, vocabularies)
                 from a JSON object, creating tasks.json if there is none
  merge-driver <base> <ours> <theirs>
                 git merge driver for tasks.json (see .agentic-task-manager/.gitattributes)
  claims [--json]
                 list live claims, shared by all worktrees of this clone
  repository [set URL [--from OLD]]
                 print or set the repository's web address (meta.repository), for commit and PR links; a git
                 remote is turned into one; atm init fills it from origin. Setting a new one moves the tasks'
                 recorded PR links from the old address (--from, else the one it replaces)
  logo [set PATH | unset]
                 print, set or unset the project's logo for the dashboard (meta.logo: an SVG, PNG, WebP or ICO
                 file inside the project, at most 512 KB); without one, a logo.svg, .github/logo.png, ... is used
  sprints [--all] | sprints add ["<name>"] [--goal T] [--release R] [--due DATE] [--after S]
          | sprints edit <S> [--goal] [--release] [--due] | sprints start <S>
          | sprints close <S> [--carry-to <S2>|new [--name N]|backlog]
                 the sprints in order with their task counts (closed ones with --all); add one (a name is
                 generated without one), change one, make one active (one at a time), or close one: its
                 unfinished tasks go to --carry-to (needed when there are any), keeping their status, and it
                 keeps a report of what was done, carried and added mid-sprint
  releases [--all] | releases add <R> [--name N] [--target DATE] [--pr N]... [--released-at TIME]
          | releases edit <R> [--name] [--target] | releases archive <R> | releases notes <R>
                 the releases in order with their tasks by kind and stage (archived ones with --all); add one
                 (--pr gives it every task of those pull requests, --released-at records one that already shipped),
                 change one, archive it, or print a Markdown
                 draft of its changes (done tasks by type, with their outcome notes; what isn't done yet). A
                 sprint's or a task's release must be one of them
  components | components add <id> [--label T] [--path GLOB]... [--review MODE] [--from-label L] [--assign]
             | components edit <id> [--label T] [--add-path GLOB]... [--review MODE] [--from-label L] [--assign]
                 the components (meta.components) with their globs and task counts; add one, or change one:
                 --from-label moves a label onto it on every task (archived ones too) and drops the label,
                 --assign gives it to every task whose files its globs match
  move <id>... --to <sprint>|next|active|backlog
                 move to-do, blocked or in-review tasks into a sprint, the next planned one, the active one, or
                 the backlog (claimed and finished tasks stay where they are)
  backlog [--all]
                 the tasks with no sprint (with --all, finished ones too)
  projects [add [PATH] | remove <id|PATH> | prune]
                 the projects the dashboard shows (~/.agentic-task-manager/projects.json, or $ATM_HOME):
                 list them, add one, remove one, or drop the ones whose folder or tasks.json is gone
                 (status commands and heartbeat take --agent NAME and --json; status commands and next --claim
                 take --status ID to pick a status when the project has several of that kind)
  help           show this help

Every command takes --json, and `atm <command> --help` describes it.
"""


# Commands that don't check for an outdated dashboard server first: serve does it itself, the server process is the
# server, and git runs the merge driver in the middle of a merge
NO_REFRESH = ("serve", "_serve-child", "merge-driver", "_lock-test", "help", "version")


class Parser(argparse.ArgumentParser):
    """Usage errors as AtmError (exit code 2), so they are reported like every other error."""

    def error(self, message):
        raise AtmError(f"{message}\n{self.format_usage().strip()}", 2)


def cmd_version(args):
    out(to_json({"version": plugin_version()}) if args.json else plugin_version())


def cmd_help(args):
    if args.json:
        out(to_json({"usage": USAGE}))
    else:
        sys.stdout.write(USAGE)


def build_parser():
    parser = Parser(prog="atm", add_help=False)
    commands = parser.add_subparsers(dest="command", parser_class=Parser)

    def command(name, run, description, hidden=False):
        sub = commands.add_parser(name, description=description, help=argparse.SUPPRESS if hidden else description)
        sub.add_argument("--json", action="store_true", help="print the result as JSON")
        sub.set_defaults(run=run)
        return sub

    def agent_option(sub):
        sub.add_argument("--agent", metavar="NAME", help="who you are (default: ATM_AGENT, else this session)")

    def status_option(sub):
        sub.add_argument(
            "--status",
            metavar="ID",
            help="the status to move to when the project has several of that kind (default: the first listed)",
        )

    command("version", cmd_version, "print the plugin version")
    command("help", cmd_help, "show the list of commands")
    command("paths", cmd_paths, "print where this project's task files are")
    command("migrate", cmd_migrate, "move an old tasks.json into .agentic-task-manager/ and set the folder up")

    serve = command("serve", cmd_serve, "run the dashboard server, one for all your projects, in the background")
    serve.add_argument("action", nargs="?", default="start", choices=["start", "stop", "status"])
    serve.add_argument("--open", action="store_true", help="also open the dashboard in the browser")
    serve.add_argument("--all", action="store_true", help="show the overview of all projects, not this project")
    serve.add_argument(
        "--refresh",
        action="store_true",
        help="only restart a running server that has an older dashboard than this plugin (the SessionStart hook)",
    )
    serve.add_argument("--port", type=int, default=DEFAULT_PORT, help="the port to try first")
    serve.add_argument("--dir", metavar="PATH", help="the project to show (default: the current directory's)")

    for name, description in (
        ("start", "mark a task in_progress and claim it (its dependencies must be done)"),
        ("review", "ask for a review of a finished task: in_review, its claim released"),
        ("done", "mark an in_progress or in_review task done and drop its claim"),
        ("fail", "put an in_progress task back to todo, recording why"),
        ("release", "put an in_progress task back to todo"),
        ("reject", "send a task in review back to todo, recording why"),
        ("approve", "approve a task in review: it stays in review until an agent takes it back to finish"),
        ("request-changes", "ask for changes to a task in review: its agent takes it back first (a rejection note)"),
        ("block", "a task that can't go on until someone decides: blocked, with the questions; claim released"),
        ("unblock", "a blocked task goes back to todo, with the answers as a note"),
        ("reopen", "a done or skipped task goes back to todo, with the reason as a note"),
        ("skip", "a todo or blocked task that isn't needed: skipped, with the reason as a note"),
    ):
        sub = command(name, cmd_status_change(name), description)
        sub.add_argument("id", help="the task ID")
        agent_option(sub)
        status_option(sub)
        if name == "fail":
            sub.add_argument("--reason", metavar="TEXT", help="why it failed (kept as a failure note)")
        if name == "release":
            sub.add_argument("--note", metavar="TEXT", help="a handoff for whoever picks it up next")
        if name == "review":
            sub.add_argument("--note", metavar="TEXT", help="what the reviewer should look at")
        if name in ("reject", "request-changes"):
            sub.add_argument("--reason", metavar="TEXT", required=True, help="what needs to change (a rejection note)")
        if name == "approve":
            sub.add_argument("--note", metavar="TEXT", help="what is left to do, or a word for the agent")
        if name == "reopen":
            sub.add_argument("--reason", metavar="TEXT", required=True, help="why it isn't done after all (a note)")
        if name == "skip":
            sub.add_argument("--reason", metavar="TEXT", required=True, help="why it isn't needed (a note)")
            sub.add_argument(
                "--resolution", choices=["wont_do", "obsolete", "duplicate"], help="why it ended (default: wont_do)"
            )
            sub.add_argument("--duplicate-of", metavar="ID", help="the task it duplicates (with --resolution duplicate)")
        if name == "block":
            sub.add_argument("--on", required=True, choices=["user", "agent"], help="who has to decide")
            sub.add_argument("--question", action="append", default=[], metavar="TEXT", help="an open question")
            sub.add_argument("--reason", metavar="TEXT", help="what it waits for, when it isn't a question")
        if name == "unblock":
            sub.add_argument("--answer", action="append", default=[], metavar="TEXT", help="an answer, in order")
        if name == "done":
            sub.add_argument("--note", metavar="TEXT", help="a one-line outcome")
            sub.add_argument("--commit", metavar="SHA", help="the task's commit")
            sub.add_argument(
                "--resolution", choices=["done", "wont_do", "duplicate", "obsolete"], help="how it ended (default: done)"
            )

    next_ = command("next", cmd_next, "the task /atm:loop-tasks works on next (exit code 3 if there is none)")
    next_.add_argument("--type", help="only this task type")
    next_.add_argument(
        "--with-user", action="store_true", help="also tasks done together with the user (the user is watching)"
    )
    pick = next_.add_mutually_exclusive_group()
    pick.add_argument(
        "--sprint", action="append", metavar="NAME", help="this sprint's tasks (repeatable; default: the active one)"
    )
    pick.add_argument("--tasks", metavar="ID,ID", help="these tasks, in this order")
    pick.add_argument("--all", action="store_true", help="every task: the active sprint, the planned ones, the backlog")
    pick.add_argument("--backlog", action="store_true", help="the tasks with no sprint")
    next_.add_argument("--claim", action="store_true", help="also start and claim it, atomically")
    agent_option(next_)
    status_option(next_)

    show = command("show", cmd_show, "tasks in full: what an agent needs to work on them")
    show.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    list_ = command("list", cmd_list, "one line per task: ID, status, priority, labels, name")
    list_.add_argument("--status", action="append", metavar="ID", help="only these statuses (repeatable)")
    list_.add_argument("--type", help="only this task type")
    list_.add_argument("--priority", help="only this priority (a missing one counts as the default)")
    list_.add_argument("--label", action="append", help="only tasks with this label (repeatable: all of them)")
    list_.add_argument("--agent", metavar="NAME", help="only tasks started by this agent")
    list_.add_argument("--ready", action="store_true", help="only to-do tasks whose dependencies are done")
    list_.add_argument("--blocked", action="store_true", help="only blocked tasks and to-do tasks still waiting")
    list_.add_argument("--in-pr", action="store_true", help="only tasks in a PR that isn't recorded as merged")
    list_.add_argument("--with-user", action="store_true", help="only tasks done together with the user (with: user)")
    list_.add_argument("--archived", action="store_true", help="list the archive (tasks-archive.json) instead")
    list_.add_argument(
        "--derived",
        action="store_true",
        help="each task in full with what atm derives for it (kind, ready, waiting for, effective review and release, "
        "stage), as JSON: what atm serve's api/v1/projects/<id>/tasks gives the dashboard",
    )
    list_.add_argument(
        "--tolerant",
        action="store_true",
        help="with --derived: read a malformed tasks.json as far as it can be read, as atm serve does for the "
        "dashboard, and print {meta, tasks, problems}",
    )
    status = command(
        "status",
        cmd_status,
        "the overview: counts, what is in progress, in review or blocked, the next task, the file's size",
    )

    command("summary", cmd_summary, "the project's numbers as atm serve gives them to the dashboard (JSON)")

    def scope_options(sub):
        sub.add_argument("--sprint", action="append", metavar="NAME", help="only this sprint's tasks (repeatable)")
        sub.add_argument("--backlog", action="store_true", help="only the tasks with no sprint")
        sub.add_argument("--release", metavar="R", help="only the tasks in this release (their own, or their sprint's)")
        sub.add_argument("--component", metavar="C", help="only the tasks in this component")

    scope_options(list_)
    scope_options(status)
    deps = command("deps", cmd_deps, "what a task waits for, and what waits for it")
    deps.add_argument("id", help="the task ID")

    add = command("add", cmd_add, "add a task with the next ID, or many from a JSON file")
    add.add_argument("--name", help="a short name (about 60 characters)")
    add.add_argument("--type", help="the task type (one of the project's types)")
    add.add_argument("--priority", help="the priority (default: none, which counts as the project's default)")
    add.add_argument("--label", action="append", default=[], help="a label (repeatable)")
    describe = add.add_mutually_exclusive_group()
    describe.add_argument("--description", default="", help="what exactly to do")
    describe.add_argument("--description-file", metavar="FILE", help="read the description from a file")
    add.add_argument("--dep", action="append", default=[], metavar="ID", help="a task it depends on (repeatable)")
    add.add_argument("--file", action="append", default=[], help="a file it will create or change (repeatable)")
    add.add_argument("--requirement", action="append", default=[], help="a requirement ID (repeatable)")
    add.add_argument("--sprint", metavar="NAME", help="the sprint it goes into (default: none, the backlog)")
    add.add_argument("--release", metavar="R", help="the release it ships in (default: its sprint's)")
    add.add_argument("--component", action="append", default=[], metavar="C", help="a component (repeatable)")
    add.add_argument(
        "--components", choices=["auto"], help="auto: the components its files are in (otherwise only suggested)"
    )
    add.add_argument(
        "--review", choices=list(REVIEW_MODES), help="how it is reviewed (default: its component's, else the project's)"
    )
    add.add_argument("--related", action="append", default=[], metavar="ID", help="a related task (repeatable)")
    add.add_argument(
        "--with", dest="with_", choices=["user"], help="user: done together with the user (unattended loops skip it)"
    )
    add.add_argument(
        "--json-file",
        metavar="FILE",
        help='add many: a JSON list of tasks; an "id" there is a placeholder (e.g. "new-1") other tasks\' '
        '"dependencies" can use, replaced by the real ID',
    )
    agent_option(add)

    edit = command("edit", cmd_edit, "change a task's definition (not its status: use the status commands)")
    edit.add_argument("id", help="the task ID")
    edit.add_argument("--name")
    edit_description = edit.add_mutually_exclusive_group()
    edit_description.add_argument("--description")
    edit_description.add_argument("--description-file", metavar="FILE")
    edit_sprint = edit.add_mutually_exclusive_group()
    edit_sprint.add_argument("--sprint", metavar="NAME", help="put it in this sprint (planned or active)")
    edit_sprint.add_argument("--backlog", action="store_true", help="take it out of its sprint, into the backlog")
    edit.add_argument("--type")
    edit.add_argument("--priority")
    edit.add_argument("--release", metavar="R", help="the release it ships in (empty: its sprint's)")
    edit.add_argument(
        "--review", choices=list(REVIEW_MODES) + [""], help="how it is reviewed (empty: its component's, else the project's)"
    )
    for option, help_text in (
        ("file", "a file it creates or changes"),
        ("dep", "a task it depends on"),
        ("label", "a label"),
        ("component", "a component"),
        ("related", "a related task"),
    ):
        edit.add_argument(f"--add-{option}", action="append", default=[], metavar="VALUE", help=f"add {help_text}")
        edit.add_argument(f"--remove-{option}", action="append", default=[], metavar="VALUE", help=f"remove {help_text}")
    edit.add_argument(
        "--components", choices=["auto"], help="auto: add the components its files are in (otherwise only suggested)"
    )
    edit.add_argument(
        "--with", dest="with_", choices=["user", ""], help="user: done together with the user (empty: not any more)"
    )
    agent_option(edit)

    check = command("check", cmd_check, "check tasks.json: the schema, the vocabularies, dependencies; exit 1 on errors")
    check.add_argument("--files", action="store_true", help="also warn about missing files listed by finished tasks")

    archive = command("archive", cmd_archive, "move finished tasks to tasks-archive.json, to keep tasks.json small")
    archive.add_argument("--sprint", action="append", metavar="NAME", help="only this sprint's tasks (repeatable)")
    archive.add_argument("--before", metavar="DATE", help="only tasks finished before this date (2026-05-01)")
    archive.add_argument(
        "--keep-days", type=float, help="keep tasks finished in the last N days (default: meta.archive.keep_days, 14)"
    )
    archive.add_argument("--dry-run", action="store_true", help="only say what would move")
    unarchive = command("unarchive", cmd_unarchive, "bring archived tasks back into tasks.json")
    unarchive.add_argument("ids", nargs="+", metavar="id", help="task IDs")

    workflow = command("workflow", cmd_workflow, "print the project's workflow, or set it (atm workflow set TYPE)")
    workflow.add_argument("action", nargs="?", choices=["set"], help="set: change the workflow")
    workflow.add_argument("type", nargs="?", help="direct or pull-request")
    workflow.add_argument("--base", help="the branch PRs target, or that direct pushes to")
    workflow.add_argument("--verify", choices=["true", "false"], help="run the project's verify before pushing")
    workflow.add_argument("--max-rounds", type=int, help="CI fix rounds before /atm:push stops")
    workflow.add_argument("--commit-template", metavar="TEXT", help='commit messages, e.g. "feat({id}): {name}"')
    workflow.add_argument(
        "--trailers",
        choices=["forbid", "require", "none"],
        help="forbid Co-Authored-By lines, require the --trailer lines, or leave it to the agent",
    )
    workflow.add_argument("--trailer", action="append", default=[], metavar="LINE", help="a required trailer line")

    commit = command("commit", cmd_commit, "commit exactly the named files plus tasks.json, and record the commit")
    commit.add_argument("id", help="the task ID")
    commit.add_argument(
        "--file", action="append", default=[], metavar="F", help="a file the task changed (repeat for each; required)"
    )
    commit.add_argument("--message", metavar="TEXT", help="the message (default: meta.workflow.commit_template)")
    commit.add_argument(
        "--record",
        metavar="REV",
        help="record this existing commit on the current branch instead of committing (e.g. after a rebase)",
    )
    agent_option(commit)

    pr = command("pr", cmd_pr, "record the PR that contains the tasks")
    pr.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    pr.add_argument("--number", type=int, required=True, help="the PR number")
    pr.add_argument("--url", required=True, help="the PR's URL")
    agent_option(pr)

    merged = command("merged", cmd_merged, "record that a PR was merged, on the tasks it contains")
    merged.add_argument("--pr", type=int, required=True, metavar="N", help="the PR number")
    merged.add_argument("--at", metavar="TIME", help="when it was merged (ISO 8601 UTC; default: now)")
    agent_option(merged)

    note = command("note", cmd_note, "add a note to a task (at most 500 characters)")
    note.add_argument("id", help="the task ID")
    note.add_argument("text", help="the note")
    agent_option(note)

    heartbeat = command("heartbeat", cmd_heartbeat, "extend your claim's lease")
    heartbeat.add_argument("id", nargs="?", help="the task ID")
    heartbeat.add_argument(
        "--auto",
        action="store_true",
        help="renew your claims if their last renewal is ten minutes old; silent (the plugin's PostToolUse hook)",
    )
    agent_option(heartbeat)

    run = command("run", cmd_run, "run a long command while keeping your claim on a task alive")
    run.add_argument("id", help="the task ID")
    run.add_argument(
        "--every",
        type=float,
        metavar="SECONDS",
        help="renew this often (default: three times per lease, at least once a minute)",
    )
    agent_option(run)

    init = command("init", cmd_init, "give tasks.json a meta.id, and register the tasks.json merge driver")
    init.add_argument(
        "--meta-file",
        metavar="F",
        help="set tasks.json meta from a JSON object (project, vocabularies); creates tasks.json if needed",
    )
    merge = command("merge-driver", cmd_merge_driver, "git merge driver for tasks.json")
    for side in ("base", "ours", "theirs"):
        merge.add_argument(side)
    command("claims", cmd_claims, "list live claims, shared by all worktrees of this clone")
    repository = command("repository", cmd_repository, "print or set the repository's web address (meta.repository)")
    repository.add_argument("action", nargs="?", choices=["set"], help="set: change it")
    repository.add_argument("url", nargs="?", help="an http(s) URL or a git remote (git@host:owner/repo.git)")
    repository.add_argument(
        "--from",
        dest="old",
        metavar="OLD",
        help="with set: move recorded PR links from this address (default: the address being replaced)",
    )
    logo = command("logo", cmd_logo, "print, set or unset the project's logo for the dashboard (meta.logo)")
    logo.add_argument("action", nargs="?", choices=["set", "unset"], help="set or unset it")
    logo.add_argument("path", nargs="?", help="set: the image (SVG, PNG, WebP or ICO), relative to here")
    sprints = command("sprints", cmd_sprints, "the sprints with their task counts; add, edit, start or delete one")
    sprints.add_argument(
        "action", nargs="?", choices=["list", "add", "edit", "start", "close", "delete"], help="what to do"
    )
    sprints.add_argument("name", nargs="?", help="the sprint's name (add: generated when left out)")
    sprints.add_argument(
        "--carry-to", metavar="WHERE", help="close: where unfinished tasks go: a sprint's name, new, or backlog"
    )
    sprints.add_argument(
        "--name", dest="new_name", metavar="NAME", help="close --carry-to new: the new sprint's name (default: generated)"
    )
    sprints.add_argument(
        "--reviewed", action="store_true", help="close: its batch tasks were reviewed with the user (the sprint review)"
    )
    agent_option(sprints)
    sprints.add_argument("--all", action="store_true", help="list: the closed sprints too")
    sprints.add_argument("--goal", help="add, edit: what the sprint is for (empty: remove it)")
    sprints.add_argument("--release", help="add, edit: the release it delivers to (empty: remove it)")
    sprints.add_argument("--due", metavar="DATE", help="add, edit: the planned end, YYYY-MM-DD (empty: remove it)")
    sprints.add_argument("--after", metavar="NAME", help="add: put it after this sprint (default: last)")
    sprints.add_argument(
        "--move-to", metavar="WHERE", help="delete: where its tasks go: a sprint's name or backlog (default)"
    )
    sprints.add_argument(
        "--force", action="store_true", help="delete: also a sprint tasks finished in or moved on from"
    )
    move = command("move", cmd_move, "move tasks into a sprint, the next sprint, the active one, or the backlog")
    move.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    move.add_argument(
        "--to", required=True, metavar="WHERE", help="a sprint's name, next, active, or backlog"
    )
    agent_option(move)
    backlog = command("backlog", cmd_backlog, "the tasks with no sprint (not finished ones, unless --all)")
    backlog.add_argument("--all", action="store_true", help="also the finished tasks with no sprint")
    releases = command("releases", cmd_releases, "the releases with their tasks; add, edit or archive one")
    releases.add_argument(
        "action", nargs="?", choices=["list", "add", "edit", "archive", "notes"], help="what to do"
    )
    releases.add_argument("id", nargs="?", help="the release's ID (letters, digits, '.', '-', '_')")
    releases.add_argument("--all", action="store_true", help="list: the archived releases too")
    releases.add_argument("--name", dest="title", help="add, edit: a name besides its ID (empty: remove it)")
    releases.add_argument("--target", metavar="DATE", help="add, edit: the planned date, YYYY-MM-DD (empty: remove it)")
    releases.add_argument(
        "--pr", type=int, action="append", metavar="N", help="add: give it the tasks of pull request N (repeatable)"
    )
    releases.add_argument(
        "--released-at", metavar="TIME", help="add: it shipped already, at this time (e.g. its PR's merge)"
    )
    components = command(
        "components", cmd_components, "the project's components; define or change one, from a label or by files"
    )
    components.add_argument("action", nargs="?", choices=["list", "add", "edit"], help="what to do")
    components.add_argument("id", nargs="?", help="the component's ID (letters, digits, '.', '-', '_')")
    components.add_argument(
        "--label", help="add, edit: the name shown (add: default the label's own text, with --from-label; edit: empty "
        "removes it)"
    )
    components.add_argument(
        "--path", action="append", metavar="GLOB", help="add: a glob over its files, e.g. 'dashboard/**' (repeatable)"
    )
    components.add_argument(
        "--add-path", action="append", metavar="GLOB", help="edit: add a glob over its files (repeatable)"
    )
    components.add_argument(
        "--review", help="add, edit: how its tasks are reviewed (blocker, async, batch, none; edit: empty removes it)"
    )
    components.add_argument(
        "--from-label", metavar="LABEL", help="add, edit: every task with this label gets the component instead"
    )
    components.add_argument(
        "--assign", action="store_true", help="add, edit: also give it to every task whose files its globs match"
    )
    projects = command("projects", cmd_projects, "the projects the dashboard shows: list, add, remove or prune them")
    projects.add_argument("action", nargs="?", default="list", choices=["list", "add", "remove", "prune"])
    projects.add_argument("target", nargs="?", help="add: the project's folder (default: here); remove: its ID or path")

    child = command("_serve-child", cmd_serve_child, "the detached server process", hidden=True)
    child.add_argument("home")
    child.add_argument("port", type=int)
    lock_test = command("_lock-test", cmd_lock_test, "test hook: hold the lock", hidden=True)
    lock_test.add_argument("seconds", type=float, nargs="?", default=0.0)
    return parser, commands.choices


def report(error, as_json):
    if as_json:
        sys.stderr.write(to_json({"error": str(error), "code": error.code}) + "\n")
    else:
        sys.stderr.write(f"atm: {error}\n")


def leading_dir(argv):
    """`atm --dir PATH <command> ...` (or `--dir=PATH`, T-351): the folder to run in, and the rest. Only before the
    command: `atm serve --dir PATH` keeps its own meaning."""
    if argv and argv[0] == "--dir":
        return (argv[1] if len(argv) > 1 else ""), argv[2:]
    if argv and argv[0].startswith("--dir="):
        return argv[0][len("--dir=") :], argv[1:]
    return None, argv


def main(argv):
    target, argv = leading_dir(argv)
    if target is not None:
        # As if started in that folder: the project is found from there, and relative paths count from there
        if not target or not os.path.isdir(target):
            report(AtmError(f"--dir: no folder '{target}'", 2), "--json" in argv)
            return 2
        os.chdir(target)
    if not argv or argv[0] in ("-h", "--help"):
        sys.stdout.write(USAGE)
        return 0
    parser, commands = build_parser()
    # `atm run <id> -- <command>`: what follows -- is the command to run, never atm's own options
    child = None
    if argv[0] == "run" and "--" in argv:
        split = argv.index("--")
        argv, child = argv[:split], argv[split + 1 :]
    as_json = "--json" in argv
    if argv[0] not in commands:
        if as_json:
            report(AtmError(f"unknown command '{argv[0]}'", 2), True)
        else:
            sys.stderr.write(f"atm: unknown command '{argv[0]}'\n" + USAGE)
        return 2
    try:
        args, extra = parser.parse_known_args(argv)
        args.child = child
        if extra:
            # Reported by the command's own parser, so the usage line is the command's
            commands[argv[0]].error(f"unrecognized arguments: {' '.join(extra)}")
        if argv[0] not in NO_REFRESH:
            try:
                refresh_server()  # after a plugin update, the first atm command brings the dashboard server along
            except Exception:
                pass  # never in the way of the command itself
        args.run(args)
    except AtmFailure as e:
        # atm-lib's errors and those of the modules from before the layers (AtmError is one of them)
        report(e, as_json)
        return e.code
    except AtmExit as e:
        return e.code
    except Exception as e:  # noqa: BLE001
        # A file atm didn't expect (a hostile or damaged one) is an error message, not a traceback: the skills and
        # the merge driver read stderr (docs/security-review.md F12)
        report(AtmError(f"unexpected error ({type(e).__name__}: {e}); run atm check, and report it if that is clean"), as_json)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
