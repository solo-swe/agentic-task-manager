"""The status commands and `atm note`, through atm-lib (docs/layered-architecture.md §9, step 8)."""

import os

from ..boot.identity import agent_identity
from ..boot.open import open_project
from ..common.core import out, to_json
from ..lib.status import STATUS_FUNCTIONS, add_task_note
from .render import render_task, report_expired

# Each command's options, as its atm-lib function names them (§5.1): (parameter, the parsed argument)
COMMAND_OPTIONS = {
    "start": (),
    "review": (("note", "note"),),
    "done": (("note", "note"), ("commit", "commit"), ("resolution", "resolution")),
    "reject": (("reason", "reason"),),
    "approve": (("note", "note"),),
    "request-changes": (("reason", "reason"),),
    "block": (("on", "on"), ("questions", "question"), ("reason", "reason")),
    "unblock": (("answers", "answer"),),
    "reopen": (("reason", "reason"),),
    "skip": (("reason", "reason"), ("resolution", "resolution"), ("duplicate_of", "duplicate_of")),
    "fail": (("reason", "reason"),),
    "release": (("note", "note"),),
}
LIST_OPTIONS = ("questions", "answers")


def cmd_status_change(command):
    def run(args):
        project = open_project(os.getcwd(), agent_identity(args.agent))
        options = {
            parameter: getattr(args, name, None) or ([] if parameter in LIST_OPTIONS else None)
            for parameter, name in COMMAND_OPTIONS[command]
        }
        result = STATUS_FUNCTIONS[command](project, args.id, status=args.status, **options)
        report_expired(result["expired"])
        task, hint, stranded = result["task"], result["hint"], result["stranded"]
        if args.json:
            extra = {"unblocked": result["unblocked"]} if command == "done" else {}
            if hint:
                extra["suggest"] = {"archive": hint}
            if stranded:
                extra["hint"] = stranded
            out(to_json({**task, "claim": result["claim"], **extra}))
            return
        # New lines only after the existing one (CLI-3.2)
        out(f"{task['id']}: {result['previous']} -> {task['status']}")
        if stranded:
            out(f"  {stranded}")
        if command == "start":
            out(render_task(result["view"]))
        if result["unblocked"]:
            out("Now ready: " + ", ".join(result["unblocked"]))
        if hint:
            out(hint["text"])

    return run


def cmd_note(args):
    """Add a note to a task, whatever its status (DM-4)."""
    task = add_task_note(open_project(os.getcwd(), agent_identity(args.agent)), args.id, args.text)
    if args.json:
        out(to_json(task["notes"][-1]))
    else:
        out(f"{task['id']}: note added ({len(task['notes'])} in all)")
