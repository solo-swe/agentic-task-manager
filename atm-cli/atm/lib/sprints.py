"""atm-lib: sprints and the backlog (docs/sprints-and-releases-design.md §3.1; docs/layered-architecture.md §5.1):
list, add, edit, start, close and delete sprints (meta.sprints), move tasks between sprints and the backlog, list the
backlog. Each write is one store change; a task's sprint changes only through set_sprint, which keeps its history.
The functions take keyword parameters (T-383)."""

from ..common.errors import Invalid, NotFound
from ..common.vocabularies import KINDS
from ..common.core import now_iso
from ..common.model import (
    DATE,
    MAX_SPRINT_NAME,
    find_sprint,
    find_task,
    kind_of,
    known_release,
    set_sprint,
    touch,
    vocabulary,
)
from ..common.sprint_names import generate_sprint_name
from .edit import open_sprint
from .loop import archived_in


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


def apply_fields(sprint, vocab, goal, release, due):
    """goal, release and due on a sprint (None leaves one as it is); an empty value removes the field; a release must
    be one of meta.releases (SR-2.2). Returns the names changed."""
    changed = []
    release = known_release(vocab, release) if release else release
    for key, value in (("goal", goal), ("release", release), ("due", check_due(due))):
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


def add_sprint(project, name=None, goal=None, release=None, due=None, after=None):
    """A planned sprint, named `name` (generated without one), after the sprint `after` (default: last), with its
    goal, release and due date. Returns the sprint."""

    def change(work):
        data = work.document
        vocab = vocabulary(data)
        sprint_name = (name or "").strip() or generate_sprint_name(
            vocab["sprint_names"], taken_names(data, work.archive()["tasks"])
        )
        if find_sprint(vocab, sprint_name):
            raise Invalid(f"there is already a sprint {find_sprint(vocab, sprint_name)['id']!r}")
        if len(sprint_name) > MAX_SPRINT_NAME:
            raise Invalid(f"a sprint name is at most {MAX_SPRINT_NAME} characters")
        sprint = {"id": sprint_name, "state": "planned"}
        apply_fields(sprint, vocab, goal, release, due)
        meta = data.setdefault("meta", {})
        sprints = list(meta.get("sprints") or [])
        position = len(sprints)
        if after:
            previous = find_sprint(vocab, after)
            if previous is None:
                raise NotFound(f"no sprint {after!r} to add it after", 2)
            position = next(i for i, s in enumerate(sprints) if s.get("id") == previous["id"]) + 1
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


def edit_sprint(project, name, goal=None, release=None, due=None):
    """Change a sprint's goal, release or due date (None leaves it, an empty value removes it). Returns {sprint,
    changed}."""

    def change(work):
        sprint = stored_sprint(work.document, name)
        changed = apply_fields(sprint, vocabulary(work.document), goal, release, due)
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


def carry_target(data, vocab, sprint, to, new_name, taken):
    """Where `to` (carry_to) sends the unfinished tasks: a sprint ID, None for the backlog, or a new planned sprint
    (named `new_name`, added to meta.sprints after the one closing). Returns (target, created sprint or None)."""
    if new_name and to != "new":
        raise Invalid("--name names a new sprint: use it with --carry-to new")
    if to == "backlog":
        return None, None
    if to == "new":
        name = (new_name or "").strip() or generate_sprint_name(vocab["sprint_names"], taken)
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


def close_sprint(project, name, carry_to=None, new_name=None, reviewed=False):
    """Close a sprint (SR-4.3): its unfinished tasks go to `carry_to` (a sprint, `new`, named `new_name`, or
    `backlog`), keeping their status and history (SR-4.5); the sprint gets `end` and its report {done, carried, added,
    to}; with `reviewed` its batch tasks are marked reviewed (SR-4.4). Returns {sprint, created, target, was_active}."""
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
        if pending and not reviewed:
            ids = ", ".join(t["id"] for t in pending)
            raise Invalid(
                f"{ids} {'waits' if len(pending) == 1 else 'wait'} for the end-of-sprint review: review "
                f"{'it' if len(pending) == 1 else 'them'} with the user (/atm:close-sprint), then close with --reviewed"
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        if unfinished and not carry_to:
            ids = ", ".join(t["id"] for t in unfinished)
            raise Invalid(
                f"{ids} {'is' if len(unfinished) == 1 else 'are'}n't finished: say where "
                f"{'it goes' if len(unfinished) == 1 else 'they go'} with --carry-to <sprint>, new or backlog"
            )
        target, created = (None, None)
        if unfinished:
            target, created = carry_target(data, vocab, sprint, carry_to, new_name, taken_names(data, archived))
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


def delete_sprint(project, name, move_to=None, force=False):
    """Delete a sprint created by mistake (T-339). An empty one (no task in it, none ever) goes at once, whatever its
    state; an open one's unfinished tasks go to `move_to` (the backlog by default); one that tasks finished in or
    moved on from needs `force` when open, and a closed one with any history stays (its record and report).
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
        if history and not force:
            raise Invalid(
                f"tasks finished in the sprint {sprint['id']!r} or moved on from it ({', '.join(history)}): delete it "
                "with --force to drop that record (its tasks go to --move-to, the backlog by default)"
            )
        to = move_to or "backlog"
        target = None if to == "backlog" else open_sprint(vocab, to)
        if target is not None and target.casefold() == sprint["id"].casefold():
            raise Invalid(f"the tasks can't move to the sprint being deleted ({sprint['id']!r})")
        # With --force, the finished tasks still in it move too, so no task names a sprint that is gone
        moving = own if force else unfinished
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
        plans, archived = [], archived_in(work)
        for task_id in ids:
            task = find_task(data, task_id, archived)
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
