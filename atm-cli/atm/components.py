"""Components (docs/sprints-and-releases-design.md §2, SR-2.7): the parts of a project (meta.components, each with
globs over its files), the components a task's files suggest, and the IDs a task may name. `atm components` lists
them; `atm components add` defines one, and can move a label onto it or give it to the tasks its globs match (T-324),
for projects that marked their parts with labels."""

import os
from fnmatch import fnmatchcase

from .core import AtmError, out, to_json, write_json_atomic
from .lock import locked
from .model import REVIEW_MODES, RELEASE_ID, archive_path, load_archive, load_tasks, vocabulary
from .paths import find_paths


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


def suggestion_line(vocab, task):
    """`atm add` / `edit --add-file` (SR-2.7): the components the task's files suggest that it doesn't have yet, as a
    line with the command that sets them; None when there are none (or the project has no components)."""
    missing = [c for c in suggest_components(vocab, task.get("files") or []) if c not in (task.get("components") or [])]
    if not missing:
        return None
    adds = " ".join(f"--add-component {c}" for c in missing)
    return f"  {task['id']}: its files suggest the component{'s' if len(missing) > 1 else ''} {', '.join(missing)} (atm edit {task['id']} {adds})"


def _tasks(*files):
    return [t for data in files for t in data["tasks"] if isinstance(t, dict)]


def components_list(paths, as_json):
    """meta.components in order, each with how many tasks have it, archived ones too."""
    data = load_tasks(paths)
    tasks = _tasks(data, load_archive(paths))
    listed = [
        {**c, "tasks": sum(1 for t in tasks if c["id"] in (t.get("components") or []))}
        for c in vocabulary(data)["components"]
    ]
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


def _label_entry(entry):
    """A meta.labels entry's ID: the entry itself when it is a string, else its "id"."""
    return entry if isinstance(entry, str) else entry.get("id") if isinstance(entry, dict) else None


def components_add(paths, args):
    """Define a component; --from-label moves that label onto it on every task (both files) and drops it from
    meta.labels; --assign gives it to every task whose files its globs match. One locked write."""
    component_id = args.id
    if not RELEASE_ID.fullmatch(component_id):
        raise AtmError(f"{component_id!r}: a component ID is letters, digits, '.', '-' and '_' (at most 64)", 2)
    globs = args.path or []
    if any(not glob for glob in globs):
        raise AtmError("--path needs a glob, e.g. 'dashboard/**'", 2)
    if args.review is not None and args.review not in REVIEW_MODES:
        raise AtmError(f"--review {args.review!r}: use one of {', '.join(REVIEW_MODES)}", 2)
    if args.assign and not globs:
        raise AtmError("--assign gives the component to the tasks its globs match: it needs --path", 2)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive_data = load_archive(paths)
        if any(c["id"] == component_id for c in vocabulary(data)["components"]):
            raise AtmError(f"there is already a component {component_id!r}", 2)
        meta = data.setdefault("meta", {})
        label_text = None
        if args.from_label:
            entries = meta.get("labels") if isinstance(meta.get("labels"), list) else []
            entry = next((e for e in entries if _label_entry(e) == args.from_label), None)
            if isinstance(entry, dict) and isinstance(entry.get("label"), str):
                label_text = entry["label"]
        component = {"id": component_id}
        # The name shown: --label, else the label's own text (meta.labels), else none
        if args.label or label_text:
            component["label"] = args.label or label_text
        component["paths"] = list(globs)
        if args.review is not None:
            component["review"] = args.review
        meta["components"] = list(meta.get("components") or []) + [component]
        vocabulary(data)  # checks the new list, as atm init would
        from_label, by_files, archive_changed = [], [], False
        for task in _tasks(data, archive_data):
            labels = task.get("labels") if isinstance(task.get("labels"), list) else []
            moved = bool(args.from_label) and args.from_label in labels
            matched = args.assign and any(matches(component, f) for f in task.get("files") or [] if isinstance(f, str))
            if not (moved or matched):
                continue
            if moved:
                task["labels"] = [label for label in labels if label != args.from_label]
                if not task["labels"]:
                    del task["labels"]
            current = task.get("components") if isinstance(task.get("components"), list) else []
            if component_id not in current:
                task["components"] = current + [component_id]
            (from_label if moved else by_files).append(task["id"])
            archive_changed = archive_changed or any(task is t for t in archive_data["tasks"])
        # A label no task has and meta.labels doesn't define is most likely a typo: nothing is written
        known_label = any(_label_entry(e) == args.from_label for e in meta.get("labels") or [])
        if args.from_label and not from_label and not known_label:
            raise AtmError(f"no task has the label {args.from_label!r}", 2)
        if args.from_label and isinstance(meta.get("labels"), list):
            meta["labels"] = [e for e in meta["labels"] if _label_entry(e) != args.from_label]
            if not meta["labels"]:
                del meta["labels"]
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
        if archive_changed:
            write_json_atomic(archive_path(paths), archive_data)
    if args.json:
        out(to_json({"component": component, "from_label": from_label, "by_files": by_files}))
        return
    parts = []
    if args.from_label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {args.from_label}")
    if args.assign:
        parts.append(f"{len(by_files)} {'more ' if args.from_label else ''}by their files")
    out(f"Added the component {component_id}" + (f": {', '.join(parts)}." if parts else "."))


def cmd_components(args):
    paths = find_paths(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise AtmError(f"unexpected argument '{args.id}'", 2)
        for option in ("label", "path", "review", "from_label"):
            if getattr(args, option):
                raise AtmError(f"--{option.replace('_', '-')} is for atm components add", 2)
        if args.assign:
            raise AtmError("--assign is for atm components add", 2)
        components_list(paths, args.json)
        return
    if not args.id:
        raise AtmError("atm components add needs the component's ID", 2)
    components_add(paths, args)
