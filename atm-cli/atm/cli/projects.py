"""`atm project list|add|remove|prune`: the projects the dashboard shows, through atm-lib."""

from ..common.errors import Invalid
from ..common.core import out, to_json
from ..lib.projects import add_project, list_projects, prune_projects, remove_project


def describe_entry(entry):
    return f"{entry['id']}  {entry['name']}  {entry['path']}"


def damaged_notice(damaged):
    return f"projects.json was unreadable: moved it to {damaged} and started an empty registry." if damaged else None


def projects_action(registry, action, target, as_json):
    """List, add, remove or prune the dashboard's projects (`atm project list|add|remove|prune`)."""
    if action == "add":
        result = add_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        out(f"{'Added' if result['result'] == 'added' else 'Already registered'}: {describe_entry(result['project'])}")
    elif action == "remove":
        result = remove_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        out(f"Removed: {describe_entry(result['removed'])}")
    elif action == "prune":
        result = prune_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["removed"]:
            out("Nothing to prune: every registered project is there.")
        for e in result["removed"]:
            out(f"Removed: {describe_entry(e)}")
    else:
        if target:
            raise Invalid(f"unexpected argument '{target}'")
        result = list_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["projects"]:
            out("No projects registered. `atm project add [PATH]` adds one; atm adds a project when it runs there.")
        for e in result["projects"]:
            out(describe_entry(e) + ("  (not found)" if e["missing"] else ""))
