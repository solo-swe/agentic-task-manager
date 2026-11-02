"""`atm sprints`, `atm move` and `atm backlog`, through atm-lib (docs/layered-architecture.md §9, step 11)."""

import os

from ..boot.open import open_project
from ..boot.identity import agent_identity
from ..common.errors import Invalid
from ..common.core import out, to_json
from ..lib.query import summary
from ..lib.sprints import (
    add_sprint,
    backlog_tasks,
    close_sprint,
    delete_sprint,
    edit_sprint,
    list_sprints,
    move_tasks,
    start_sprint,
)
from .render import list_lines

NONE_ACTIVE = "No sprint is active now: start the next one (atm sprints start <name>), or plan one with /atm:plan-sprint."


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
        out("No open sprints (--all lists the closed ones)." if listed["has_sprints"] else "No sprints yet: atm sprints add creates one.")
    for s in sprints:
        out(describe_sprint(s))
    out(f"backlog: {backlog} task{'' if backlog == 1 else 's'} (atm backlog)")


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


def cmd_sprints(args):
    project = open_project(os.getcwd(), agent_identity(getattr(args, "agent", None)))
    if args.action in (None, "list"):
        if args.name is not None:
            raise Invalid(f"unexpected argument '{args.name}'")
        show_sprints(project, args.all, args.json)
        return
    if args.action == "add":
        sprint = add_sprint(project, args)
        out(to_json(sprint) if args.json else f"Added the sprint “{sprint['id']}” (planned).")
        return
    if not args.name:
        raise Invalid(f"atm sprints {args.action} needs a sprint name")
    if args.action == "edit":
        result = edit_sprint(project, args.name, args)
        sprint, changed = result["sprint"], result["changed"]
        if args.json:
            out(to_json(sprint))
        else:
            out(f"{sprint['id']}: changed {', '.join(changed)}" if changed else f"{sprint['id']}: nothing changed")
    elif args.action == "start":
        sprint = start_sprint(project, args.name)
        out(to_json(sprint) if args.json else f"The sprint “{sprint['id']}” is now active.")
    elif args.action == "close":
        show_close(close_sprint(project, args.name, args), args.json)
    else:
        show_delete(delete_sprint(project, args.name, args), args.json)


def cmd_move(args):
    moved = move_tasks(open_project(os.getcwd(), agent_identity(args.agent)), args.ids, args.to)
    if args.json:
        out(to_json(moved))
        return
    for m in moved:
        before, after = m["from"] or "backlog", m["to"] or "backlog"
        out(f"{m['id']}: {before} -> {after}" if before != after else f"{m['id']}: already in {after}")


def cmd_backlog(args):
    found = backlog_tasks(open_project(os.getcwd()), finished=args.all)
    vocab, tasks = found["vocab"], found["tasks"]
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
    elif not tasks:
        out("The backlog is empty.")
    else:
        for line in list_lines(vocab, tasks):
            out(line)
