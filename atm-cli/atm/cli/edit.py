"""What `atm task add` and `task edit` read and print beyond atm-lib (docs/layered-architecture.md §9, step 10): the
file --json-file names (atm-lib takes its content), and the components a task's files suggest."""

from ..common.errors import Invalid
from ..common.components import suggest_components
from ..common.core import read_json


def suggestion_line(vocab, task):
    """`atm task add` / `task edit --add-file` (SR-2.7): the components the task's files suggest that it doesn't have
    yet, as a line with the command that sets them; None when there are none (or the project has no components)."""
    missing = [c for c in suggest_components(vocab, task.get("files") or []) if c not in (task.get("components") or [])]
    if not missing:
        return None
    adds = " ".join(f"--add-component {c}" for c in missing)
    return f"  {task['id']}: its files suggest the component{'s' if len(missing) > 1 else ''} {', '.join(missing)} (atm task edit {task['id']} {adds})"


def read_entries(path):
    """The tasks of an `atm task add --json-file`: a list of task objects, or {"tasks": [...]}."""
    value = read_json(path)
    if isinstance(value, dict):
        value = value.get("tasks")
    if not isinstance(value, list) or not all(isinstance(e, dict) for e in value):
        raise Invalid(f"{path}: expected a JSON list of tasks (or {{\"tasks\": [...]}})")
    return value
