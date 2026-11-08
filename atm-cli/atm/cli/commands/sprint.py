"""`atm sprint`: sprints (list, add, edit, start, close, delete, move, backlog)."""

import os
from typing import Annotated, Optional

import typer

from ...boot.identity import agent_identity
from ...boot.open import open_project
from ...common.core import out, to_json
from ...lib.query import summary
from ...lib.sprints import (
    add_sprint,
    backlog_tasks,
    close_sprint,
    delete_sprint,
    edit_sprint,
    list_sprints,
    move_tasks,
    start_sprint,
)
from ..app import _dry, app
from ..render import list_lines


# How the `atm sprint` commands print sprints (docs/layered-architecture.md §9, step 11).

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


sprint_app = typer.Typer(help="Sprint management")
app.add_typer(sprint_app, name="sprint")


@sprint_app.command("list")
def sprint_list(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="List closed sprints too"),
) -> None:
    """List the sprints with their task counts, and the backlog's size."""
    show_sprints(open_project(os.getcwd()), all_, ctx.obj.get("json_out", False))


AGENT_HELP = "Who you are (default: ATM_AGENT, else this session)"


@sprint_app.command("add")
def sprint_add(
    ctx: typer.Context,
    name: Optional[str] = typer.Argument(None, help="The sprint's name (generated when left out)"),
    goal: Optional[str] = typer.Option(None, "--goal", help="What the sprint is for"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it delivers to"),
    due: Optional[str] = typer.Option(None, "--due", metavar="DATE", help="The planned end, YYYY-MM-DD"),
    after: Optional[str] = typer.Option(None, "--after", metavar="NAME", help="Put it after this sprint (default: last)"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Add a sprint (planned)."""
    if _dry(ctx, f"add the sprint {name or '(a generated name)'}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    sprint = add_sprint(project, name, goal=goal, release=release, due=due, after=after)
    out(to_json(sprint) if ctx.obj.get("json_out", False) else f"Added the sprint “{sprint['id']}” (planned).")


@sprint_app.command("edit")
def sprint_edit(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    goal: Optional[str] = typer.Option(None, "--goal", help="What the sprint is for (empty: remove it)"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it delivers to (empty: remove it)"),
    due: Optional[str] = typer.Option(None, "--due", metavar="DATE", help="The planned end, YYYY-MM-DD (empty: remove it)"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Change a sprint's goal, release or due date."""
    if _dry(ctx, f"edit the sprint {name}"):
        return
    result = edit_sprint(open_project(os.getcwd(), agent_identity(agent)), name, goal=goal, release=release, due=due)
    sprint, changed = result["sprint"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(sprint))
    else:
        out(f"{sprint['id']}: changed {', '.join(changed)}" if changed else f"{sprint['id']}: nothing changed")


@sprint_app.command("start")
def sprint_start(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Make a planned sprint the active one."""
    if _dry(ctx, f"start the sprint {name}"):
        return
    sprint = start_sprint(open_project(os.getcwd(), agent_identity(agent)), name)
    out(to_json(sprint) if ctx.obj.get("json_out", False) else f"The sprint “{sprint['id']}” is now active.")


@sprint_app.command("close")
def sprint_close(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    carry_to: Optional[str] = typer.Option(
        None, "--carry-to", metavar="WHERE", help="Where unfinished tasks go: a sprint's name, new, or backlog"
    ),
    new_name: Optional[str] = typer.Option(
        None, "--name", metavar="NAME", help="With --carry-to new: the new sprint's name (default: generated)"
    ),
    reviewed: bool = typer.Option(
        False, "--reviewed", help="Its batch tasks were reviewed with the user (the sprint review)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Close a sprint: record its report and move its unfinished tasks on."""
    if _dry(ctx, f"close the sprint {name}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    closed = close_sprint(project, name, carry_to=carry_to, new_name=new_name, reviewed=reviewed)
    show_close(closed, ctx.obj.get("json_out", False))


@sprint_app.command("delete")
def sprint_delete(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    move_to: Optional[str] = typer.Option(
        None, "--move-to", metavar="WHERE", help="Where its tasks go: a sprint's name or backlog (default)"
    ),
    force: bool = typer.Option(False, "--force", help="Also a sprint tasks finished in or moved on from"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Delete a sprint created by mistake."""
    if _dry(ctx, f"delete the sprint {name}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    show_delete(delete_sprint(project, name, move_to=move_to, force=force), ctx.obj.get("json_out", False))


@sprint_app.command("move")
def sprint_move(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    to: str = typer.Option(..., "--to", metavar="WHERE", help="A sprint's name, next, active, or backlog"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Move tasks into a sprint, the next sprint, the active one, or the backlog."""
    if _dry(ctx, f"move {', '.join(ids)} to {to}"):
        return
    moved = move_tasks(open_project(os.getcwd(), agent_identity(agent)), ids, to)
    if ctx.obj.get("json_out", False):
        out(to_json(moved))
        return
    for m in moved:
        before, after = m["from"] or "backlog", m["to"] or "backlog"
        out(f"{m['id']}: {before} -> {after}" if before != after else f"{m['id']}: already in {after}")


@sprint_app.command("backlog")
def sprint_backlog(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="Also the finished tasks with no sprint"),
) -> None:
    """The tasks with no sprint (not finished ones, unless --all)."""
    found = backlog_tasks(open_project(os.getcwd()), finished=all_)
    rows = [summary(found["vocab"], t) for t in found["tasks"]]
    if ctx.obj.get("json_out", False):
        out(to_json(rows))
    elif not rows:
        out("The backlog is empty.")
    else:
        for line in list_lines(rows):
            out(line)
