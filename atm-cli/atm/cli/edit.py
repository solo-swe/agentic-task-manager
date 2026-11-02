"""`atm add`, `atm edit` and `atm check`, through atm-lib (docs/layered-architecture.md §9, step 10). The files these
options name (--json-file, --description-file) are read here: atm-lib takes their content."""

import os

from ..boot.open import open_project
from ..claims import agent_identity
from ..common.errors import Invalid
from ..components import suggestion_line
from ..core import AtmExit, out, read_json, to_json
from ..lib.edit import Changes, add_new_tasks, check_project, edit_existing_task


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
