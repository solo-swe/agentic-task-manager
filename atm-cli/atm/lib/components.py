"""atm-lib: the project's components (docs/sprints-and-releases-design.md §2, SR-2.7; docs/layered-architecture.md
§5.1): list them, define one, change one; moving a label onto a component, or giving it to the tasks its globs match
(T-324), changes the archive's tasks too, in the same store change. The rules (which files a component has, the IDs a
task may name) are common's (common/components.py)."""

from ..common.components import matches
from ..common.errors import Invalid, NotFound
from ..common.model import RELEASE_ID, REVIEW_MODES, vocabulary


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
            raise Invalid(f"there is already a component {component_id!r} (atm component edit changes it)")
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
