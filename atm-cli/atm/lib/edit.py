"""atm-lib: adding and changing tasks (`atm add`, one task or many; `atm edit`) and checking the task data (`atm
check`), docs/layered-architecture.md §5.1. IDs come from meta.next_id inside the store's change, so two agents adding
at once never get the same one (docs/v4/data-model.md DM-2.3). Status and timestamps are not edited here: they change
only through the status commands."""

import os
import re

from ..common.components import component_ids, suggest_components
from ..common.core import AtmError, now_iso
from ..common.model import (
    RESOLUTIONS,
    REVIEW_MODES,
    find_release,
    find_sprint,
    find_task,
    set_sprint,
    kind_of,
    known_release,
    move_out_hint,
    open_in_closed_sprints,
    split_id,
    status_for,
    touch,
    valid_task_id,
    vocabulary,
)
from .loop import archived_in

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
        known = ", ".join(s["id"] for s in vocab["sprints"] if s["state"] != "closed") or "none yet: atm sprint add"
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


def check_related(task_id, related, known, archived):
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


def add_tasks(data, entries, actor, archived):
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


def add_new_tasks(project, entries):
    """`atm add`: the tasks `entries` describe (dicts of task fields; an "id" there is a placeholder the others'
    dependencies may use), with the next IDs, all or none, as `project.actor`. Returns {tasks, vocab}."""

    def change(work):
        tasks = add_tasks(work.document, entries, project.actor, archived_in(work))
        return {"tasks": tasks, "vocab": vocabulary(work.document)}

    return project.store.change(change)


class TaskEdit:
    """What edit_task applies, as attributes (name, description, sprint, backlog, type, priority, release, review,
    with_, components; add_/remove_ file, dep, label, component, related, requirement, use_case); what isn't given
    changes nothing."""

    def __init__(self, **given):
        self.__dict__.update(given)

    def __getattr__(self, name):
        return [] if name.startswith(("add_", "remove_")) else None


def edit_existing_task(
    project,
    task_id,
    *,
    name=None,
    description=None,
    sprint=None,
    backlog=False,
    task_type=None,
    priority=None,
    release=None,
    review=None,
    with_=None,
    components=None,
    add_files=(),
    remove_files=(),
    add_dependencies=(),
    remove_dependencies=(),
    add_labels=(),
    remove_labels=(),
    add_components=(),
    remove_components=(),
    add_related=(),
    remove_related=(),
    add_requirements=(),
    remove_requirements=(),
    add_use_cases=(),
    remove_use_cases=(),
):
    """`atm edit`: change a task's definition (never its status), as `project.actor`. None (or an empty list) leaves
    a field as it is; an empty `release`, `review` or `with_` removes it; `backlog` takes it out of its sprint;
    `components="auto"` adds the components its files are in. Returns {task, changed, vocab}: `changed` the names of
    the fields that changed (nothing is written when none did)."""
    changes = TaskEdit(
        name=name,
        description=description,
        sprint=sprint,
        backlog=backlog,
        type=task_type,
        priority=priority,
        release=release,
        review=review,
        with_=with_,
        components=components,
        add_file=list(add_files),
        remove_file=list(remove_files),
        add_dep=list(add_dependencies),
        remove_dep=list(remove_dependencies),
        add_label=list(add_labels),
        remove_label=list(remove_labels),
        add_component=list(add_components),
        remove_component=list(remove_components),
        add_related=list(add_related),
        remove_related=list(remove_related),
        add_requirement=list(add_requirements),
        remove_requirement=list(remove_requirements),
        add_use_case=list(add_use_cases),
        remove_use_case=list(remove_use_cases),
    )

    def change(work):
        archived = archived_in(work)
        task = find_task(work.document, task_id, archived)
        changed = edit_task(work.document, task, changes, archived)
        if changed:
            touch(task, project.actor, now_iso())
        return {"task": task, "changed": changed, "vocab": vocabulary(work.document)}

    return project.store.change(change)


def edit_task(data, task, args, archived):
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
    # An entry is an ID or a Markdown link to where it is defined (T-391): any text but an empty one
    for key, add, remove in (
        ("requirements", args.add_requirement, args.remove_requirement),
        ("use_cases", args.add_use_case, args.remove_use_case),
    ):
        if any(not isinstance(v, str) or not v.strip() for v in add):
            raise AtmError(f"an empty {key[:-1].replace('_', ' ')} can't be added", 2)
        edit_list(key, add, remove)
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
        errors.append(f"{task_id}: in both tasks.json and the archive (atm db archive keeps the tasks.json copy)")
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
