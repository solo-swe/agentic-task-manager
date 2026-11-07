"""`atm task`: a task's lifecycle (list, add, edit, details, note, deps, state) and its review's decisions
(`atm task review`)."""

import os
from typing import Annotated, Optional

import click
import typer

from ...boot.identity import agent_identity
from ...boot.open import open_project
from ...common.components import suggest_components
from ...common.core import out, read_json, to_json
from ...common.errors import Invalid
from ...lib.archive import task_snapshot
from ...lib.edit import add_new_tasks, edit_existing_task
from ...lib.query import Filters, task_dependencies, task_views
from ...lib.status import STATUS_FUNCTIONS, add_task_note, todo_command
from ...lib.tasks import show_tasks
from ..app import app
from ..render import list_lines, render_task, report_expired, view_row


# What `atm task state` passes to atm-lib's status functions (docs/layered-architecture.md §9, step 8).

# Each status function's options, as atm-lib names them (§5.1): (parameter, the command's option)
COMMAND_OPTIONS = {
    "start": (),
    "review": (("note", "note"),),
    "done": (("note", "note"), ("commit", "commit")),
    "reject": (("reason", "reason"),),
    "drop": (),
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


# What `atm task add` and `task edit` read and print beyond atm-lib (docs/layered-architecture.md §9, step 10): the
# file --json-file names (atm-lib takes its content), and the components a task's files suggest.


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


task_app = typer.Typer(help="Task lifecycle commands")
app.add_typer(task_app, name="task")


@task_app.command("list")
def task_list(
    ctx: typer.Context,
    status: Optional[list[str]] = typer.Option(
        None, "--status", help="Only these statuses (repeatable)"
    ),
    sprint: Optional[list[str]] = typer.Option(
        None, "--sprint", help="Only this sprint's tasks (repeatable)"
    ),
    backlog: bool = typer.Option(False, "--backlog", help="Only tasks with no sprint"),
    release: Optional[str] = typer.Option(None, "--release", help="Only tasks in this release"),
    component: Optional[str] = typer.Option(
        None, "--component", help="Only tasks in this component"
    ),
    type_: Optional[str] = typer.Option(None, "--type", help="Only this task type"),
    priority: Optional[str] = typer.Option(None, "--priority", help="Only this priority"),
    label: Optional[list[str]] = typer.Option(
        None, "--label", help="Only tasks with this label (repeatable)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Only tasks started by this agent"),
    ready: bool = typer.Option(
        False, "--ready", help="Only to-do tasks whose dependencies are done"
    ),
    blocked: bool = typer.Option(
        False, "--blocked", help="Only blocked tasks and to-do tasks still waiting"
    ),
    in_pr: bool = typer.Option(
        False, "--in-pr", help="Only tasks in a PR that isn't recorded as merged"
    ),
    with_user: bool = typer.Option(
        False, "--with-user", help="Only tasks done together with the user"
    ),
    archived: bool = typer.Option(False, "--archived", help="List the archive instead"),
    derived: bool = typer.Option(False, "--derived", help="Each task in full as JSON"),
    tolerant: bool = typer.Option(
        False, "--tolerant", help="With --derived: read a malformed tasks.json as far as it can be read"
    ),
) -> None:
    """List tasks: one line per task: ID, status, priority, labels, name."""
    filters = Filters(
        sprint=sprint or None,
        backlog=backlog,
        release=release,
        component=component,
        status=status or None,
        type=type_,
        priority=priority,
        label=label or None,
        agent=agent,
        ready=ready,
        blocked=blocked,
        in_pr=in_pr,
        with_user=with_user,
        archived=archived,
    )
    if derived:
        # JSON either way: it is for tools (the dashboard's tests take atm's own values from it)
        if tolerant:
            snapshot = task_snapshot(open_project(os.getcwd(), tolerant=True), filters)
            snapshot.pop("bytes")
            out(to_json(snapshot))
        else:
            out(to_json(task_views(open_project(os.getcwd()), filters)))
        return
    rows = [view_row(v) for v in task_views(open_project(os.getcwd()), filters)]
    if ctx.obj.get("json_out", False):
        out(to_json(rows))
        return
    if not rows:
        out("No tasks match.")
        return
    for line in list_lines(rows):
        out(line)


@task_app.command("add")
def task_add(
    ctx: typer.Context,
    name: Optional[str] = typer.Option(None, "--name", help="A short name (about 60 characters)"),
    type_: Optional[str] = typer.Option(None, "--type", help="The task type"),
    sprint: Optional[str] = typer.Option(None, "--sprint", help="The sprint it goes into"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it ships in"),
    priority: Optional[str] = typer.Option(None, "--priority", help="The priority"),
    label: Optional[list[str]] = typer.Option(None, "--label", help="A label (repeatable)"),
    description: Optional[str] = typer.Option(None, "--description", help="What exactly to do"),
    description_file: Optional[str] = typer.Option(
        None, "--description-file", help="Read the description from a file"
    ),
    dep: Optional[list[str]] = typer.Option(
        None, "--dep", help="A task it depends on (repeatable)"
    ),
    file: Optional[list[str]] = typer.Option(
        None, "--file", help="A file it creates or changes (repeatable)"
    ),
    requirement: Optional[list[str]] = typer.Option(
        None, "--requirement", help="A requirement (repeatable)"
    ),
    component: Optional[list[str]] = typer.Option(
        None, "--component", help="A component (repeatable)"
    ),
    components_auto: Optional[str] = typer.Option(
        None,
        "--components",
        click_type=click.Choice(["auto"]),
        help="auto: the components its files are in (otherwise only suggested)",
    ),
    review: Optional[str] = typer.Option(None, "--review", help="How it is reviewed"),
    related: Optional[list[str]] = typer.Option(
        None, "--related", help="A related task (repeatable)"
    ),
    with_user: Optional[str] = typer.Option(
        None, "--with", help="user: done together with the user"
    ),
    json_file: Optional[str] = typer.Option(None, "--json-file", help="Add many from a JSON file"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Add a task with the next ID, or many from a JSON file."""
    if description and description_file:
        raise Invalid("--description and --description-file can't both be given")
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would add tasks from {json_file}" if json_file else f"[dry-run] would add task {name}")
        return
    if json_file:
        entries = read_entries(json_file)
    else:
        if description_file:
            with open(description_file, encoding="utf-8") as f:
                description = f.read()
        entries = [
            {
                "name": name,
                "type": type_,
                "priority": priority,
                "labels": label or [],
                "description": description or "",
                "dependencies": dep or [],
                "files": file or [],
                "requirements": requirement or [],
                "sprint": sprint,
                "release": release,
                "components": "auto" if components_auto else (component or []),
                "review": review,
                "with": with_user,
                "related": related or [],
            }
        ]
    result = add_new_tasks(open_project(os.getcwd(), agent_identity(agent)), entries)
    if ctx.obj.get("json_out", False):
        out(to_json(result["tasks"]))
        return
    for task in result["tasks"]:
        out(f"Added {task['id']} {task['name']}")
        line = suggestion_line(result["vocab"], task)
        if line:
            out(line)


@task_app.command("edit")
def task_edit(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    name: Optional[str] = typer.Option(None, "--name"),
    description: Optional[str] = typer.Option(None, "--description"),
    description_file: Optional[str] = typer.Option(None, "--description-file"),
    sprint: Optional[str] = typer.Option(None, "--sprint"),
    backlog: bool = typer.Option(False, "--backlog"),
    type_: Optional[str] = typer.Option(None, "--type"),
    priority: Optional[str] = typer.Option(None, "--priority"),
    release: Optional[str] = typer.Option(None, "--release"),
    review: Optional[str] = typer.Option(None, "--review"),
    add_file: Optional[list[str]] = typer.Option(None, "--add-file"),
    remove_file: Optional[list[str]] = typer.Option(None, "--remove-file"),
    add_dep: Optional[list[str]] = typer.Option(None, "--add-dep"),
    remove_dep: Optional[list[str]] = typer.Option(None, "--remove-dep"),
    add_label: Optional[list[str]] = typer.Option(None, "--add-label"),
    remove_label: Optional[list[str]] = typer.Option(None, "--remove-label"),
    add_component: Optional[list[str]] = typer.Option(None, "--add-component"),
    remove_component: Optional[list[str]] = typer.Option(None, "--remove-component"),
    add_related: Optional[list[str]] = typer.Option(None, "--add-related"),
    remove_related: Optional[list[str]] = typer.Option(None, "--remove-related"),
    add_requirement: Optional[list[str]] = typer.Option(None, "--add-requirement"),
    remove_requirement: Optional[list[str]] = typer.Option(None, "--remove-requirement"),
    add_use_case: Optional[list[str]] = typer.Option(None, "--add-use-case"),
    remove_use_case: Optional[list[str]] = typer.Option(None, "--remove-use-case"),
    components_auto: Optional[str] = typer.Option(
        None,
        "--components",
        click_type=click.Choice(["auto"]),
        help="auto: add the components its files are in (otherwise only suggested)",
    ),
    with_user: Optional[str] = typer.Option(None, "--with"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Change a task's definition (not its status: use `atm task state`)."""
    if description is not None and description_file:
        raise Invalid("--description and --description-file can't both be given")
    if sprint is not None and backlog:
        raise Invalid("--sprint and --backlog can't both be given")
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would edit task {task_id}")
        return
    if description_file:
        with open(description_file, encoding="utf-8") as f:
            description = f.read()
    result = edit_existing_task(
        open_project(os.getcwd(), agent_identity(agent)),
        task_id,
        name=name,
        description=description,
        sprint=sprint,
        backlog=backlog,
        task_type=type_,
        priority=priority,
        release=release,
        review=review,
        with_=with_user,
        components="auto" if components_auto else None,
        add_files=add_file or [],
        remove_files=remove_file or [],
        add_dependencies=add_dep or [],
        remove_dependencies=remove_dep or [],
        add_labels=add_label or [],
        remove_labels=remove_label or [],
        add_components=add_component or [],
        remove_components=remove_component or [],
        add_related=add_related or [],
        remove_related=remove_related or [],
        add_requirements=add_requirement or [],
        remove_requirements=remove_requirement or [],
        add_use_cases=add_use_case or [],
        remove_use_cases=remove_use_case or [],
    )
    task, changed = result["task"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(task))
        return
    out(f"{task['id']}: changed {', '.join(changed)}" if changed else f"{task['id']}: nothing changed")
    # New files may put the task in components it isn't in yet (SR-2.7): suggested, not set
    line = suggestion_line(result["vocab"], task) if add_file else None
    if line:
        out(line)


@task_app.command("details")
def task_details(
    ctx: typer.Context,
    task_ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Tasks in full (description, dependencies, files, notes)."""
    views = show_tasks(open_project(os.getcwd()), task_ids)
    if ctx.obj.get("json_out", False):
        # An archived task says so (T-353)
        out(to_json([{**v["task"], "archived": True} if v["archived"] else v["task"] for v in views]))
    else:
        out("\n\n".join(render_task(v) for v in views))


@task_app.command("note")
def task_note(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    text: Annotated[str, typer.Argument(help="The note text")],
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Add a note to a task (at most 500 characters)."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would add a note to {task_id}")
        return
    task = add_task_note(open_project(os.getcwd(), agent_identity(agent)), task_id, text)
    if ctx.obj.get("json_out", False):
        out(to_json(task["notes"][-1]))
    else:
        out(f"{task['id']}: note added ({len(task['notes'])} in all)")


@task_app.command("deps")
def task_deps(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
) -> None:
    """What a task waits for, and what waits for it."""
    deps = task_dependencies(open_project(os.getcwd()), task_id)
    if ctx.obj.get("json_out", False):
        out(to_json(deps))
        return
    line = lambda items: ", ".join(f"{i['id']} ({i['status']})" for i in items) or "nothing"
    out(f"{task_id} waits for: {line(deps['waits_for'])}")
    out(f"{task_id} is waited on by: {line(deps['waited_on_by'])}")


@task_app.command("state")
def task_state(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    next_state: Annotated[
        str, typer.Argument(help="Next state: todo, in_progress, in_review, done, blocked, skipped")
    ],
    note: Optional[str] = typer.Option(None, "--note", help="A note"),
    reason: Optional[str] = typer.Option(None, "--reason", help="A reason"),
    commit: Optional[str] = typer.Option(None, "--commit", help="The task's commit SHA"),
    resolution: Optional[str] = typer.Option(
        None, "--resolution", help="skipped: why it isn't needed (wont_do, obsolete or duplicate; default wont_do)"
    ),
    duplicate_of: Optional[str] = typer.Option(
        None, "--duplicate-of", help="The task it duplicates"
    ),
    on: Optional[str] = typer.Option(
        None, "--on", click_type=click.Choice(["user", "agent"]), help="blocked: who has to decide"
    ),
    question: Optional[list[str]] = typer.Option(
        None, "--question", help="An open question (repeatable)"
    ),
    answer: Optional[list[str]] = typer.Option(None, "--answer", help="An answer (repeatable)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who runs it"),
) -> None:
    """Move a task to another state: in_progress, in_review, done, todo, blocked or skipped. A review's decision is
    `atm task review`."""
    # The state -> atm-lib's status function
    cmd_map = {
        "in_progress": "start",
        "in_review": "review",
        "done": "done",
        "todo": "fail",  # chosen below by the kind the task is in now
        "blocked": "block",
        "skipped": "skip",
    }
    cmd = cmd_map.get(next_state)
    if cmd is None:
        raise Invalid(f"unknown state '{next_state}'. Use: {', '.join(sorted(cmd_map))}")

    project = open_project(os.getcwd(), agent_identity(agent))
    if next_state == "todo":
        # Back to todo is a different status function for each kind the task is in (a failure says why: --reason)
        cmd = todo_command(project, task_id, failing=reason is not None)

    given = {
        "note": note,
        "reason": reason,
        "commit": commit,
        "resolution": resolution,
        "duplicate_of": duplicate_of,
        "on": on,
        "question": question,
        "answer": answer,
    }
    takes = {name for _, name in COMMAND_OPTIONS[cmd]}
    extra = [name for name, value in given.items() if value and name not in takes]
    if extra:
        flags = ", ".join("--" + name.replace("_", "-") for name in extra)
        raise Invalid(f"{flags}: not for {next_state} ({cmd}) from where {task_id} is")
    required = {"reopen": "reason", "skip": "reason", "block": "on"}
    if cmd in required and not given[required[cmd]]:
        raise Invalid(f"{next_state} ({cmd}) needs --{required[cmd]}")
    run_status(ctx, project, cmd, task_id, status, given)


def run_status(ctx, project, cmd, task_id, status, given):
    """Run atm-lib's status function `cmd` with the options `given`, and print the result (task state, task review)."""
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {cmd} {task_id}")
        return
    options = {
        parameter: given.get(name) or ([] if parameter in LIST_OPTIONS else None)
        for parameter, name in COMMAND_OPTIONS[cmd]
    }
    result = STATUS_FUNCTIONS[cmd](project, task_id, status=status, **options)
    report_expired(result["expired"])
    task, hint, stranded = result["task"], result["hint"], result["stranded"]
    if ctx.obj.get("json_out", False):
        extra_fields = {"unblocked": result["unblocked"]} if cmd == "done" else {}
        if hint:
            extra_fields["suggest"] = {"archive": hint}
        if stranded:
            extra_fields["hint"] = stranded
        out(to_json({**task, "claim": result["claim"], **extra_fields}))
        return
    out(f"{task['id']}: {result['previous']} -> {task['status']}")
    if stranded:
        out(f"  {stranded}")
    if cmd == "start":
        out(render_task(result["view"]))
    if result["unblocked"]:
        out("Now ready: " + ", ".join(result["unblocked"]))
    if hint:
        out(hint["text"])


# A review's decisions (T-438): the task stays in review, and each opens one way out of it, which `task state` takes:
# approved, `done` (or `in_progress` for what is left); changes requested, `in_progress`; rejected, `todo` or
# `skipped`. The decision comes first, so a permission rule can tell them apart; no skill pre-approves them (I23)
review_app = typer.Typer(help="Decide a task's review: approve it, ask for changes, or reject it; it stays in review")
task_app.add_typer(review_app, name="review")


@review_app.command("approve")
def review_approve(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    note: Optional[str] = typer.Option(None, "--note", help="What is left to do, or a word for the agent"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who decides"),
) -> None:
    """Approve a task in review: then it can be done, or taken back for what is left."""
    run_status(ctx, open_project(os.getcwd(), agent_identity(agent)), "approve", task_id, status, {"note": note})


@review_app.command("request-changes")
def review_request_changes(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    reason: str = typer.Option(..., "--reason", help="What needs to change (a rejection note)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who decides"),
) -> None:
    """Ask for changes to a task in review: its agent takes it back first."""
    run_status(
        ctx, open_project(os.getcwd(), agent_identity(agent)), "request-changes", task_id, status, {"reason": reason}
    )


@review_app.command("reject")
def review_reject(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    reason: str = typer.Option(..., "--reason", help="Why (a rejection note)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who decides"),
) -> None:
    """Reject a task in review: then it goes back to to do (its work dropped) or is skipped."""
    run_status(ctx, open_project(os.getcwd(), agent_identity(agent)), "reject", task_id, status, {"reason": reason})
