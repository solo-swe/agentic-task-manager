"""The status commands and `atm note`, through atm-lib (docs/layered-architecture.md §9, step 8)."""

import os

from ..boot.open import open_project
from ..claims import agent_identity, report_expired
from ..common.core import out, to_json
from ..lib.status import add_task_note, change_task_status
from .render import describe_task


def cmd_status_change(command):
    def run(args):
        project = open_project(os.getcwd(), agent_identity(args.agent))
        result = change_task_status(
            project,
            command,
            args.id,
            status=args.status,
            note=getattr(args, "note", None),
            reason=getattr(args, "reason", None),
            commit=getattr(args, "commit", None),
            on=getattr(args, "on", None),
            questions=getattr(args, "question", None) or [],
            answers=getattr(args, "answer", None) or [],
            resolution=getattr(args, "resolution", None),
            duplicate_of=getattr(args, "duplicate_of", None),
        )
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
            out(describe_task(result["document"], task))
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
