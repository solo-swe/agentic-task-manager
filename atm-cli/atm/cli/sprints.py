"""How the `atm sprint` commands print sprints (docs/layered-architecture.md §9, step 11)."""

from ..common.core import out, to_json
from ..lib.sprints import list_sprints

NONE_ACTIVE = "No sprint is active now: start the next one (atm sprint start <name>), or plan one with /atm:plan-sprint."


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
        out("No open sprints (--all lists the closed ones)." if listed["has_sprints"] else "No sprints yet: atm sprint add creates one.")
    for s in sprints:
        out(describe_sprint(s))
    out(f"backlog: {backlog} task{'' if backlog == 1 else 's'} (atm sprint backlog)")


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
