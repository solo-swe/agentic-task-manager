"""`atm loop`: the work loop, the next task to work on."""

import os
from typing import Optional

import typer

from ...boot.identity import agent_identity
from ...boot.open import open_project
from ...common.core import AtmExit, out, to_json
from ...common.errors import Invalid
from ...lib.claims import next_task_for, selection
from ..app import app
from ..render import report_expired

loop_app = typer.Typer(help="The work loop")
app.add_typer(loop_app, name="loop")


@loop_app.command("next")
def loop_next(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(
        None, "--sprint", help="This sprint's tasks (repeatable; default: the active one)"
    ),
    tasks: Optional[str] = typer.Option(None, "--tasks", metavar="ID,ID", help="These tasks, in this order"),
    all_tasks: bool = typer.Option(
        False, "--all", help="Every task: the active sprint, the planned ones, the backlog"
    ),
    backlog: bool = typer.Option(False, "--backlog", help="The tasks with no sprint"),
    type_: Optional[str] = typer.Option(None, "--type", help="Only this task type"),
    with_user: bool = typer.Option(
        False, "--with-user", help="Also tasks done together with the user (the user is watching)"
    ),
    claim: bool = typer.Option(False, "--claim", help="Also start and claim it, atomically"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """The task /atm:loop-tasks works on next (exit code 3 if there is none)."""
    given = [flag for flag, value in (("--sprint", sprint), ("--tasks", tasks), ("--all", all_tasks), ("--backlog", backlog)) if value]
    if len(given) > 1:
        raise Invalid(f"{' and '.join(given)} can't both be given: they choose the tasks in different ways")
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {'claim' if claim else 'show'} the next task")
        return
    sel = selection(
        sprints=sprint or None,
        tasks=[i.strip() for i in (tasks or "").split(",") if i.strip()],
        all_tasks=all_tasks,
        backlog=backlog,
        task_type=type_,
    )
    project = open_project(os.getcwd(), agent_identity(agent))
    result = next_task_for(project, sel, with_user=with_user, claim=claim, status=status)
    report_expired(result["expired"])
    task, claimed, reason = result["task"], result["claim"], result["reason"]
    if ctx.obj.get("json_out", False):
        out(to_json({"task": task, "claim": claimed, "reason": reason}))
    elif task:
        verb = "Claimed" if claimed else "Next"
        where = task.get("sprint") or "backlog"
        out(f"{verb}: {task['id']} {task.get('name', '')} ({where}, {task.get('type')})")
    else:
        out(f"No task available: {reason}.")
    if not task:
        raise AtmExit(3)
