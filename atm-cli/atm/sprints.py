"""Sprints and the backlog (docs/sprints-and-releases-design.md §3.1): `atm sprints` lists, adds, edits and starts
sprints (meta.sprints), `atm move` moves tasks between sprints and the backlog, `atm backlog` lists the tasks with no
sprint. Every write is under the lock; a task's sprint changes only through set_sprint, which keeps its history."""

import os

from .claims import agent_identity
from .core import AtmError, now_iso, out, to_json, write_json_atomic
from .edit import open_sprint
from .lock import locked
from .common.vocabularies import KINDS
from .model import (
    DATE,
    MAX_SPRINT_NAME,
    find_sprint,
    find_task,
    kind_of,
    known_release,
    load_archive,
    load_tasks,
    set_sprint,
    touch,
    vocabulary,
)
from .paths import find_paths
from .cli.render import list_lines
from .lib.query import summary
from .sprint_names import generate_sprint_name


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


def describe_sprint(s):
    details = [s["state"]] + [f"{key} {s[key]}" for key in ("release", "due") if s.get(key)]
    counts = ", ".join(f"{kind} {n}" for kind, n in s["by_kind"].items())
    line = f"{s['id']} · {' · '.join(details)}: {s['tasks']} task{'' if s['tasks'] == 1 else 's'}"
    line += f" ({counts})" if counts else ""
    if s.get("review_pending"):
        line += f"; {s['review_pending']} waiting for the sprint review"
    return line + (f"\n  goal: {s['goal']}" if s.get("goal") else "")


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
        raise AtmError(f"--due {due!r}: use a date, YYYY-MM-DD", 2)
    return due


def apply_fields(sprint, args, vocab):
    """--goal, --release and --due on a sprint; an empty value removes the field; a release must be one of
    meta.releases (SR-2.2). Returns the names changed."""
    changed = []
    release = known_release(vocab, args.release) if args.release else args.release
    for key, value in (("goal", args.goal), ("release", release), ("due", check_due(args.due))):
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


def sprints_list(paths, include_closed, as_json):
    data = load_tasks(paths)
    sprints = sprint_overview(data, load_archive(paths)["tasks"], include_closed)
    vocab = vocabulary(data)
    backlog = [t for t in data["tasks"] if in_backlog(vocab, t)]
    if as_json:
        out(to_json({"sprints": sprints, "backlog": len(backlog)}))
        return
    if not sprints:
        out(
            "No open sprints (--all lists the closed ones)."
            if vocab["sprints"]
            else "No sprints yet: atm sprints add creates one."
        )
    for s in sprints:
        out(describe_sprint(s))
    out(f"backlog: {len(backlog)} task{'' if len(backlog) == 1 else 's'} (atm backlog)")


def sprints_add(paths, args):
    with locked(paths["dir"]):
        data = load_tasks(paths)
        vocab = vocabulary(data)
        name = (args.name or "").strip() or generate_sprint_name(
            vocab["sprint_names"], taken_names(data, load_archive(paths)["tasks"])
        )
        if find_sprint(vocab, name):
            raise AtmError(f"there is already a sprint {find_sprint(vocab, name)['id']!r}", 2)
        if len(name) > MAX_SPRINT_NAME:
            raise AtmError(f"a sprint name is at most {MAX_SPRINT_NAME} characters", 2)
        sprint = {"id": name, "state": "planned"}
        apply_fields(sprint, args, vocab)
        meta = data.setdefault("meta", {})
        sprints = list(meta.get("sprints") or [])
        position = len(sprints)
        if args.after:
            after = find_sprint(vocab, args.after)
            if after is None:
                raise AtmError(f"no sprint {args.after!r} to add it after", 2)
            position = next(i for i, s in enumerate(sprints) if s.get("id") == after["id"]) + 1
        sprints.insert(position, sprint)
        meta["sprints"] = sprints
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
    out(to_json(sprint) if args.json else f"Added the sprint “{name}” (planned).")


def stored_sprint(data, name):
    """The sprint called `name` as stored in meta.sprints (to change it), or an error naming the sprints."""
    vocab = vocabulary(data)
    found = find_sprint(vocab, name)
    if found is None:
        known = ", ".join(s["id"] for s in vocab["sprints"]) or "none yet: atm sprints add"
        raise AtmError(f"no sprint {name!r}; the sprints: {known}", 2)
    return next(s for s in data["meta"]["sprints"] if s.get("id") == found["id"])


def sprints_edit(paths, args):
    with locked(paths["dir"]):
        data = load_tasks(paths)
        sprint = stored_sprint(data, args.name)
        changed = apply_fields(sprint, args, vocabulary(data))
        if changed:
            vocabulary(data)
            write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json(sprint))
    else:
        out(f"{sprint['id']}: changed {', '.join(changed)}" if changed else f"{sprint['id']}: nothing changed")


def sprints_start(paths, args):
    with locked(paths["dir"]):
        data = load_tasks(paths)
        sprint = stored_sprint(data, args.name)
        state = sprint.get("state", "planned")
        if state == "closed":
            raise AtmError(f"the sprint {sprint['id']!r} is closed; closed sprints aren't started again", 2)
        if state == "active":
            raise AtmError(f"the sprint {sprint['id']!r} is already active", 2)
        active = [s["id"] for s in vocabulary(data)["sprints"] if s["state"] == "active"]
        if active:
            raise AtmError(f"the sprint {active[0]!r} is active; close it first (one active sprint at a time)", 2)
        sprint["state"] = "active"
        sprint["start"] = now_iso()
        write_json_atomic(paths["tasks"], data)
    out(to_json(sprint) if args.json else f"The sprint “{sprint['id']}” is now active.")


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


def carry_target(data, vocab, sprint, args, taken):
    """Where --carry-to sends the unfinished tasks: a sprint ID, None for the backlog, or a new planned sprint (added
    to meta.sprints after the one closing). Returns (target, created sprint or None)."""
    to = args.carry_to
    if args.new_name and to != "new":
        raise AtmError("--name names a new sprint: use it with --carry-to new", 2)
    if to == "backlog":
        return None, None
    if to == "new":
        name = (args.new_name or "").strip() or generate_sprint_name(vocab["sprint_names"], taken)
        if find_sprint(vocab, name):
            raise AtmError(f"there is already a sprint {find_sprint(vocab, name)['id']!r}", 2)
        if len(name) > MAX_SPRINT_NAME:
            raise AtmError(f"a sprint name is at most {MAX_SPRINT_NAME} characters", 2)
        created = {"id": name, "state": "planned"}
        sprints = data["meta"]["sprints"]
        sprints.insert(next(i for i, s in enumerate(sprints) if s.get("id") == sprint["id"]) + 1, created)
        return name, created
    target = open_sprint(vocab, to)
    if target == sprint["id"]:
        raise AtmError(f"the tasks can't be carried to the sprint that is closing ({sprint['id']!r})", 2)
    return target, None


def sprints_close(paths, args):
    """Close a sprint (SR-4.3): its unfinished tasks go to --carry-to, keeping their status and history (SR-4.5); the
    sprint gets `end` and its report {done, carried, added, to}."""
    actor = agent_identity(args.agent)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archived = load_archive(paths)["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, args.name)
        if sprint.get("state") == "closed":
            raise AtmError(f"the sprint {sprint['id']!r} is already closed", 2)
        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        own_archived = [t for t in archived if in_sprint(t, sprint)]

        def kind(task):
            return kind_of(vocab, task.get("status"))

        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise AtmError(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before closing the sprint",
                2,
            )
        # Batch tasks wait for the end-of-sprint review (SR-4.4): /atm:close-sprint reviews them, then --reviewed
        pending = [t for t in own if t.get("sprint_review") == "pending"]
        if pending and not args.reviewed:
            ids = ", ".join(t["id"] for t in pending)
            raise AtmError(
                f"{ids} {'waits' if len(pending) == 1 else 'wait'} for the end-of-sprint review: review "
                f"{'it' if len(pending) == 1 else 'them'} with the user (/atm:close-sprint), then close with --reviewed",
                2,
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        if unfinished and not args.carry_to:
            ids = ", ".join(t["id"] for t in unfinished)
            raise AtmError(
                f"{ids} {'is' if len(unfinished) == 1 else 'are'}n't finished: say where "
                f"{'it goes' if len(unfinished) == 1 else 'they go'} with --carry-to <sprint>, new or backlog",
                2,
            )
        target, created = (None, None)
        if unfinished:
            target, created = carry_target(data, vocab, sprint, args, taken_names(data, archived))
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
        write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json({"sprint": sprint, "created": created}))
        return
    line = f"Closed the sprint “{sprint['id']}”: {len(report['done'])} done"
    if report["carried"]:
        where = "the backlog" if target is None else f"“{target}”" + (" (new, planned)" if created else "")
        line += f", {len(report['carried'])} carried to {where} ({', '.join(report['carried'])})"
    if report["added"]:
        line += f", {len(report['added'])} added mid-sprint"
    out(line + ".")
    if was_active:
        out("No sprint is active now: start the next one (atm sprints start <name>), or plan one with /atm:plan-sprint.")


def mentions_sprint(task, sprint):
    """Whether the task's sprint history names the sprint (case-insensitively)."""
    return any(
        isinstance(e, dict) and isinstance(e.get("sprint"), str) and e["sprint"].casefold() == sprint["id"].casefold()
        for e in task.get("sprint_history") or []
    )


def sprints_delete(paths, args):
    """Delete a sprint created by mistake (T-339). An empty one (no task in it, none ever) goes at once, whatever its
    state; an open one's unfinished tasks go to --move-to (the backlog by default); one that tasks finished in or
    moved on from needs --force when open, and a closed one with any history stays (its record and report)."""
    actor = agent_identity(args.agent)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archived = load_archive(paths)["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, args.name)
        state = sprint.get("state", "planned")

        def kind(task):
            return kind_of(vocab, task.get("status"))

        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise AtmError(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before deleting the sprint",
                2,
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
            raise AtmError(
                f"the sprint {sprint['id']!r} is closed and had tasks ({', '.join(history)}): its record and report "
                "stay; only an empty sprint can be deleted",
                2,
            )
        if history and not args.force:
            raise AtmError(
                f"tasks finished in the sprint {sprint['id']!r} or moved on from it ({', '.join(history)}): delete it "
                "with --force to drop that record (its tasks go to --move-to, the backlog by default)",
                2,
            )
        to = args.move_to or "backlog"
        target = None if to == "backlog" else open_sprint(vocab, to)
        if target is not None and target.casefold() == sprint["id"].casefold():
            raise AtmError(f"the tasks can't move to the sprint being deleted ({sprint['id']!r})", 2)
        # With --force, the finished tasks still in it move too, so no task names a sprint that is gone
        moving = own if args.force else unfinished
        stamp = now_iso()
        for task in moving:
            set_sprint(task, target, stamp)
            touch(task, actor, stamp)
        data["meta"]["sprints"] = [s for s in data["meta"]["sprints"] if s is not sprint]
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
    moved = [t["id"] for t in moving]
    if args.json:
        out(to_json({"deleted": sprint["id"], "state": state, "moved": moved, "to": target or "backlog"}))
        return
    line = f"Deleted the sprint “{sprint['id']}” ({state})"
    if moved:
        where = "the backlog" if target is None else f"“{target}”"
        line += f": {len(moved)} task{'' if len(moved) == 1 else 's'} moved to {where} ({', '.join(moved)})"
    out(line + ".")
    if state == "active":
        out("No sprint is active now: start the next one (atm sprints start <name>), or plan one with /atm:plan-sprint.")


def cmd_sprints(args):
    paths = find_paths(os.getcwd())
    if args.action in (None, "list"):
        if args.name is not None:
            raise AtmError(f"unexpected argument '{args.name}'", 2)
        sprints_list(paths, args.all, args.json)
    elif args.action == "add":
        sprints_add(paths, args)
    else:
        if not args.name:
            raise AtmError(f"atm sprints {args.action} needs a sprint name", 2)
        actions = {"edit": sprints_edit, "start": sprints_start, "close": sprints_close, "delete": sprints_delete}
        actions[args.action](paths, args)


def move_target(vocab, task, to):
    """The sprint ID (or None, the backlog) `atm move --to` means for `task`."""
    if to == "backlog":
        return None
    if to == "active":
        active = [s["id"] for s in vocab["sprints"] if s["state"] == "active"]
        if not active:
            raise AtmError("no sprint is active (atm sprints start <name>)", 2)
        return active[0]
    if to == "next":
        # The first planned sprint after the task's own (for a task in the backlog or a closed sprint: the first
        # planned sprint)
        sprints = vocab["sprints"]
        current = find_sprint(vocab, task.get("sprint"))
        start = sprints.index(current) + 1 if current else 0
        following = [s["id"] for s in sprints[start:] if s["state"] == "planned"]
        if not following:
            raise AtmError(
                f"{task['id']}: no planned sprint after {current['id'] if current else 'the active one'}; "
                'add one with atm sprints add ["<name>"]',
                2,
            )
        return following[0]
    return open_sprint(vocab, to)


def cmd_move(args):
    actor = agent_identity(args.agent)
    paths = find_paths(os.getcwd())
    moved = []
    with locked(paths["dir"]):
        data = load_tasks(paths)
        vocab = vocabulary(data)
        plans = []
        for task_id in args.ids:
            task = find_task(data, task_id)
            kind = kind_of(vocab, task.get("status"))
            if kind == "active":
                raise AtmError(f"{task_id} is in progress (claimed): it stays where the work is happening", 2)
            if kind in ("done", "skipped"):
                raise AtmError(f"{task_id} is {task.get('status')}: finished work stays in the sprint it was done in", 2)
            plans.append((task, move_target(vocab, task, args.to)))
        stamp = now_iso()
        for task, target in plans:
            before = task.get("sprint")
            if set_sprint(task, target, stamp):
                touch(task, actor, stamp)
            moved.append({"id": task["id"], "from": before, "to": target})
        if any(m["from"] != m["to"] for m in moved):
            write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json(moved))
        return
    for m in moved:
        before, after = m["from"] or "backlog", m["to"] or "backlog"
        out(f"{m['id']}: {before} -> {after}" if before != after else f"{m['id']}: already in {after}")


def cmd_backlog(args):
    paths = find_paths(os.getcwd())
    data = load_tasks(paths)
    vocab = vocabulary(data)
    tasks = [t for t in data["tasks"] if in_backlog(vocab, t, finished=args.all)]
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
    elif not tasks:
        out("The backlog is empty.")
    else:
        for line in list_lines(vocab, tasks):
            out(line)
