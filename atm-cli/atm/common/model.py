"""The rules of task data: the project's vocabularies, looking tasks up, and the status transitions themselves.
Reading and writing the files is the store's (db/jsongit/files.py).

Every rule is written against a status's kind, never its name (docs/v4/data-model.md §3): a project may name its own
statuses in meta.statuses, each with one of the six kinds below.
"""

import re
from datetime import timedelta

from .vocabularies import DEFAULT_PRIORITIES, DEFAULT_PRIORITY, DEFAULT_STATUSES, DEFAULT_TYPES, KINDS
from .core import AtmError, now, now_iso, parse_iso

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
