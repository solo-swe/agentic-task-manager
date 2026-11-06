"""atm2: the Typer-based command tree for atm (docs/improving-atm-cli-ergonomics.md).

Nested groups (task, loop, claim, vcs, db, release, component, project, sprint)
over the same atm-lib calls the `atm` commands make, with the same output,
errors and exit codes (tests/scripts/atm2.test.mjs checks them side by side).
Where `atm` already has a helper that prints a result, atm2 uses it, so both
print the same text.
"""

from __future__ import annotations

import contextlib
import os
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from typing import Annotated, Optional

import typer

from ..boot.identity import agent_identity, session_identity
from ..boot.jsongit import (
    dashboard_id,
    hook_project,
    merge_versions,
    migrate_folder,
    register_project,
    set_up_store,
    task_paths,
)
from ..boot.open import open_project, open_registry
from ..common.core import (
    AtmExit, out, to_json, atm_home, ensure_home,
    format_duration, now, parse_iso, parse_version, pid_alive, plugin_version,
    read_json, whoami, PLUGIN_ROOT, SCRIPT_DIR,
)
from ..common.dashboard import (
    SCAN_PORTS, SERVER_LOG, SERVER_RECORD, SERVER_ENTRY, TEMPLATE,
    dashboard_head, home_id, DEFAULT_PORT,
)
from ..common.errors import AtmFailure, Invalid
from ..lib.archive import archive_finished, unarchive_tasks
from ..lib.code import (
    commit_task,
    folder_changes,
    get_workflow,
    record_commit,
    record_merge,
    record_pr,
    repository_paths,
    set_workflow,
)
from ..lib.archive import task_snapshot
from ..lib.claims import expire_claims, next_task_for, renew_due_claims, renew_lease, selection
from ..lib.components import add_component, edit_component, list_components
from ..lib.edit import add_new_tasks, check_project, edit_existing_task
from ..lib.query import Filters, project_summary, status_report, summary, task_dependencies, task_views
from ..lib.status import STATUS_FUNCTIONS, add_task_note
from ..lib.tasks import show_tasks
from ..lib.releases import add_release, archive_release, edit_release, list_releases, release_notes
from ..lib.sprints import (
    add_sprint,
    backlog_tasks,
    close_sprint,
    delete_sprint,
    edit_sprint,
    move_tasks,
    start_sprint,
)
from ..lib.settings import (
    ensure_project_id,
    ensure_repository,
    get_repository,
    init_meta,
    set_repository,
)
from .archive import describe_ids, describe_kept
from .claims import hook_input
from .code import describe_workflow
from .components import moved_text
from .edit import read_entries, suggestion_line
from .projects import projects_action
from .query import show_status
from .render import list_lines, render_task, report_expired, view_row
from .settings import logo_action
from .sprints import show_close, show_delete, show_sprints
from .status import COMMAND_OPTIONS, LIST_OPTIONS
from .store import show_paths


# ── constants ported from atm cli/serve.py ──────────────────────────────────

SERVER_LOCK = "server.lock"
SERVER_LOCK_WAIT = 15.0
STOP_WAIT = 5.0
KILL_WAIT = 2.0

# `atm2 --help` lists the commands people and agents use first (task, loop, claim, vcs, release, component, project,
# sprint), then the maintenance ones, then help and version; what git, hooks and the server call is hidden
# (db merge-driver, project summary). The panels are rich's, which isn't vendored (plugin/scripts/vendor/README.md):
# plain Click help shows the order only.
MAINTENANCE = "Maintenance"
META = "Meta"
HELP_ORDER = ["task", "loop", "claim", "vcs", "release", "component", "project", "sprint", "db", "doctor", "service",
              "help", "version"]


class OrderedGroup(typer.core.TyperGroup):
    """The top-level commands in HELP_ORDER (Typer lists commands before groups)."""

    def list_commands(self, ctx):
        names = super().list_commands(ctx)
        return sorted(names, key=lambda n: HELP_ORDER.index(n) if n in HELP_ORDER else len(HELP_ORDER))


app = typer.Typer(
    name="atm2",
    help="atm2: atm's commands, by group",
    no_args_is_help=True,
    cls=OrderedGroup,
)


@app.callback()
def _global(
    ctx: typer.Context,
    dry_run: bool = typer.Option(
        False, "--dry-run", "-n", help="Don't execute, just show what would be done"
    ),
    json_out: bool = typer.Option(False, "--json", "-j", help="Print result as JSON"),
) -> None:
    """Global options for all atm2 subcommands."""
    ctx.obj = {"dry_run": dry_run, "json_out": json_out}


# ---------------------------------------------------------------------------
# task group
# ---------------------------------------------------------------------------

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
    components_auto: bool = typer.Option(
        False, "--components", help="Auto: components its files are in"
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
    components_auto: bool = typer.Option(False, "--components"),
    with_user: Optional[str] = typer.Option(None, "--with"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Change a task's definition (not its status: use `atm2 task state`)."""
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
    resolution: Optional[str] = typer.Option(None, "--resolution", help="How it ended"),
    duplicate_of: Optional[str] = typer.Option(
        None, "--duplicate-of", help="The task it duplicates"
    ),
    on: Optional[str] = typer.Option(None, "--on", help="Who has to decide (user|agent)"),
    question: Optional[list[str]] = typer.Option(
        None, "--question", help="An open question (repeatable)"
    ),
    answer: Optional[list[str]] = typer.Option(None, "--answer", help="An answer (repeatable)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who runs it"),
) -> None:
    """Change a task's state (start, review, done, fail, release, reject, approve, request-changes, block, unblock, reopen, skip)."""
    # Map next-state to the corresponding `atm` subcommand
    cmd_map = {
        "in_progress": "start",
        "in_review": "review",
        "done": "done",
        "todo": "fail",  # default for active->todo; may be overridden below
        "blocked": "block",
        "skipped": "skip",
        # review sub-states (stay in review):
        "approved": "approve",
        "changes_requested": "request-changes",
    }
    cmd = cmd_map.get(next_state)
    if cmd is None:
        typer.echo(
            f"atm2: unknown state '{next_state}'. Use: {', '.join(sorted(cmd_map))}", err=True
        )
        raise typer.Exit(2)

    project = open_project(os.getcwd(), agent_identity(agent))
    if next_state == "todo":
        # Back to todo is a different command for each kind the task is in now
        view = next((v for v in task_views(project, Filters()) if v["task"].get("id") == task_id), None)
        if view is None:
            show_tasks(project, [task_id])  # NotFound when it doesn't exist; otherwise it is archived (finished)
            current = "done"
        else:
            current = view["derived"].get("kind")
        todo_commands = {
            "active": "release" if note is not None else "fail",
            "review": "reject",
            "blocked": "unblock",
            "done": "reopen",
            "skipped": "reopen",
        }
        if current not in todo_commands:
            raise Invalid(f"{task_id} is already {current}")
        cmd = todo_commands[current]

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
    required = {"reject": "reason", "request-changes": "reason", "reopen": "reason", "skip": "reason", "block": "on"}
    if cmd in required and not given[required[cmd]]:
        raise Invalid(f"{next_state} ({cmd}) needs --{required[cmd]}")

    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would {cmd} {task_id}")
        return
    options = {
        parameter: given[name] or ([] if parameter in LIST_OPTIONS else None)
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


# ---------------------------------------------------------------------------
# loop group
# ---------------------------------------------------------------------------

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


@loop_app.command("run")
def loop_run(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    cmd_and_args: Annotated[
        Optional[list[str]], typer.Argument(help="The command and its args, after --: -- npm test --verbose")
    ] = None,
    every: Optional[float] = typer.Option(
        None, "--every", metavar="SECONDS", help="Renew this often (default: three times per lease, at least once a minute)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Run a long command while keeping your claim on a task alive; exits with the command's exit code."""
    if not cmd_and_args:
        raise Invalid("atm run needs a command after --: atm run <id> -- <command> [args...]")
    if every is not None and every <= 0:
        raise Invalid("--every must be more than 0 seconds")
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would run {' '.join(cmd_and_args)} while keeping the claim on {task_id}")
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    renew = lambda: renew_lease(open_project(os.getcwd(), project.actor), task_id, expire=False)
    minutes = renew()["minutes"]
    interval = every if every is not None else max(60.0, minutes * 60 / 3)
    try:
        child = subprocess.Popen(cmd_and_args)
    except OSError as e:
        raise AtmFailure(f"can't run {cmd_and_args[0]}: {e.strerror}", 127)
    # Renewed from this (the main) thread between waits: the lock uses signals, which only work here
    renewing = True
    while True:
        try:
            code = child.wait(timeout=interval)
            break
        except subprocess.TimeoutExpired:
            pass
        except KeyboardInterrupt:  # the command got the same interrupt; wait for it to end
            continue
        if renewing:
            try:
                renew()
            except Exception as e:  # never stops the command; say so once and stop renewing
                sys.stderr.write(f"atm run: stopped renewing the claim on {task_id}: {e}\n")
                renewing = False
    if code:
        raise AtmExit(128 - code if code < 0 else code)  # a signal -N ends like a shell's 128+N


# ---------------------------------------------------------------------------
# claim group
# ---------------------------------------------------------------------------

claim_app = typer.Typer(help="Claim lifecycle")
app.add_typer(claim_app, name="claim")


@claim_app.command("list")
def claim_list(
    ctx: typer.Context,
) -> None:
    """List live claims, shared by all worktrees of this clone."""
    live =expire_claims(open_project(os.getcwd()))["claims"]
    if ctx.obj.get("json_out", False):
        out(to_json(live))
        return
    if not live:
        out("No live claims.")
        return
    current = now()
    for c in live:
        left = format_duration((parse_iso(c["lease_expires_at"]) - current).total_seconds())
        out(f"{c['task']}  {c.get('agent')}  {c.get('branch') or '-'}  {left} left  ({c.get('worktree')})")


def _auto_heartbeat(agent):
    """`heartbeat --auto`, run by the plugin's PostToolUse hook after every tool call (CLI-6.2): renew this agent's
    live claims once their last renewal is ten minutes old. It never prints and never fails, so a tool call is never
    disturbed."""
    try:
        payload = hook_input()
        cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) and os.path.isdir(payload["cwd"]) else os.getcwd()
        if payload.get("session_id") and not (agent or os.environ.get("ATM_AGENT")):
            # The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
            actor = session_identity(payload["session_id"], cwd)
        else:
            actor = agent_identity(agent)
        project = hook_project(cwd, actor)  # also lists the project on the dashboard
        if project is not None:
            renew_due_claims(project)
    except Exception:  # a hook must never break the tool call it follows
        return


@claim_app.command("heartbeat")
def claim_heartbeat(
    ctx: typer.Context,
    task_id: Optional[str] = typer.Argument(None, help="The task ID"),
    auto: bool = typer.Option(
        False, "--auto", help="Renew your claims if their last renewal is ten minutes old; silent (the plugin's hook)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Extend your claim's lease."""
    if ctx.obj.get("dry_run", False):
        out("[dry-run] would renew claims due for it" if auto else f"[dry-run] would extend the lease on {task_id}")
        return
    if auto:
        _auto_heartbeat(agent)
        return
    if not task_id:
        raise Invalid("atm heartbeat needs a task ID (or --auto)")
    claim = renew_lease(open_project(os.getcwd(), agent_identity(agent)), task_id)["claim"]
    if ctx.obj.get("json_out", False):
        out(to_json(claim))
    else:
        out(f"{task_id}: lease extended to {claim['lease_expires_at']}")


# ---------------------------------------------------------------------------
# vcs group
# ---------------------------------------------------------------------------

vcs_app = typer.Typer(help="Version control integration")
app.add_typer(vcs_app, name="vcs")


@vcs_app.command("commit")
def vcs_commit(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    file: Optional[list[str]] = typer.Option(
        None, "--file", help="A file the task changed (repeatable)"
    ),
    dir_: Optional[list[str]] = typer.Option(
        None, "--dir", help="Every change under a folder (repeatable)"
    ),
    message: Optional[str] = typer.Option(None, "--message", help="The commit message"),
    record: Optional[str] = typer.Option(
        None, "--record", help="Record this existing commit instead"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Commit exactly the named files plus tasks.json, and record the commit."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm2 vcs commit {task_id}"]
        if file:
            for f in file:
                parts.append(f"--file {f}")
        if dir_:
            for d in dir_:
                parts.append(f"--dir {d}")
        if message:
            parts.append(f"--message {message}")
        if record:
            parts.append(f"--record {record}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if record is not None:
        if file or dir_ or message is not None:
            raise Invalid("--record records an existing commit: no --file, --dir or --message")
        result = record_commit(project, task_id, record)
        if ctx.obj.get("json_out", False):
            shown = {k: result[k] for k in ("task", "commit", "recorded")}
            out(to_json({**shown, **({"hint": result["hint"]} if result["hint"] else {})}))
            return
        out(f"{result['task']}: recorded {result['commit'][:7]} {result['summary']}".rstrip())
        if result["hint"]:
            out(f"  {result['hint']}")
        return
    files = repository_paths(project, file or [], os.getcwd())
    if dir_:
        files += [f for f in folder_changes(project, task_id, dir_, os.getcwd()) if f not in files]
    result = commit_task(project, task_id, files, message)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"{result['task']}: committed {result['commit'][:7]} {result['message'].splitlines()[0]}")
    out("  " + ", ".join(result["files"]))


@vcs_app.command("pr")
def vcs_pr(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    number: int = typer.Option(..., "--number", help="The PR number"),
    url: str = typer.Option(..., "--url", help="The PR's URL"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record the PR that contains the tasks."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm2 vcs pr {' '.join(ids)} --number {number} --url {url}"]
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    task_ids = record_pr(project, ids, number, url)
    if ctx.obj.get("json_out", False):
        out(to_json({"tasks": task_ids, "pr": {"number": number, "url": url}}))
    else:
        out(f"{', '.join(task_ids)}: in PR #{number} ({url})")


@vcs_app.command("pushed")
def vcs_pushed(
    ctx: typer.Context,
    pr: int = typer.Option(..., "--pr", help="The PR number"),
    at: Optional[str] = typer.Option(None, "--at", help="When it was merged (ISO 8601 UTC)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record that a PR was merged, on the tasks it contains."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm2 vcs pushed --pr {pr}"]
        if at:
            parts.append(f"--at {at}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    result = record_merge(project, pr, at)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    task_ids = result["tasks"]
    out(f"PR #{pr} merged: {', '.join(task_ids)}" if task_ids else f"PR #{pr}: already recorded as merged")
    for release_id in result["released"]:
        out(f"Released {release_id}: all its tasks are merged in PR #{pr}.")
    for release_id, waiting in result["pending"].items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{pr}.")


@vcs_app.command("repository")
def vcs_repository(
    ctx: typer.Context,
    url: Optional[str] = typer.Argument(None, help="An http(s) URL or git remote"),
    old: Optional[str] = typer.Option(
        None, "--from", help="Move recorded PR links from this address"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Print or set the repository's web address."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = ["atm2 vcs repository"]
        if url:
            parts.append(url)
        if old:
            parts.append(f"--from {old}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if url is None:
        if old is not None:
            raise Invalid(f"unexpected argument '{old}' (atm repository set URL)")
        current = get_repository(project)
        if ctx.obj.get("json_out", False):
            out(to_json({"repository": current}))
        elif current:
            out(current)
        else:
            out("No repository address yet. Set it with `atm repository set <url>` (a git remote works too).")
        return
    result = set_repository(project, url, old)
    moved = result["moved_links"]
    if ctx.obj.get("json_out", False):
        out(to_json({"repository": result["repository"], "moved_links": moved}))
    else:
        links = f" ({moved} PR link{'' if moved == 1 else 's'} moved from {result['previous']})" if moved else ""
        out(f"Repository address set: {result['repository']}{links}")


# ---------------------------------------------------------------------------
# db group
# ---------------------------------------------------------------------------

db_app = typer.Typer(help="Database / file maintenance")
app.add_typer(db_app, name="db", rich_help_panel=MAINTENANCE)


@db_app.command("migrate")
def db_migrate(
    ctx: typer.Context,
) -> None:
    """Move tasks.json into .agentic-task-manager/ and set up the folder."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would move tasks.json into .agentic-task-manager/ and set up the folder")
        return
    migration = migrate_folder(os.getcwd())
    if ctx.obj.get("json_out", False):
        out(to_json(migration["result"]))
    else:
        for line in migration["lines"]:
            out(line)


@db_app.command("merge-driver", hidden=True)
def db_merge_driver(
    ctx: typer.Context,
    base: Annotated[str, typer.Argument(help="The common ancestor's tasks.json")],
    ours: Annotated[str, typer.Argument(help="This branch's tasks.json (written with the result)")],
    theirs: Annotated[str, typer.Argument(help="The other branch's tasks.json")],
) -> None:
    """git's merge driver for tasks.json (git runs it; `db init` registers it)."""
    conflicts = merge_versions(base, ours, theirs)
    if ctx.obj.get("json_out", False):
        out(to_json({"conflicts": conflicts}))
    if conflicts:
        sys.stderr.write("atm: tasks.json has conflicting edits (this branch's version was kept):\n")
        for conflict in conflicts:
            sys.stderr.write(f"  {conflict}\n")
        raise AtmExit(1)


@db_app.command("archive")
def db_archive(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    before: Optional[str] = typer.Option(None, "--before"),
    keep_days: Optional[float] = typer.Option(None, "--keep-days"),
) -> None:
    """Move finished tasks to tasks-archive.json (with --dry-run: say which, and move nothing)."""
    # atm-lib's own dry run says which tasks would move, as `atm archive --dry-run` does
    dry_run = ctx.obj.get("dry_run", False)
    result = archive_finished(
        open_project(os.getcwd()), sprint, before, keep_days, dry_run
    )
    ids, dropped, kept, sizes = (
        result["moved"],
        result["duplicates_dropped"],
        result["kept"],
        result["sizes"],
    )
    shown = {
        "moved": ids,
        "dry_run": bool(dry_run),
        "tasks_kb": sizes["tasks"] // 1024,
        "archive_kb": sizes["archive"] // 1024,
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if ctx.obj.get("json_out", False):
        out(to_json(shown))
        return
    if dropped and not dry_run:
        out(
            f"Kept {describe_ids(dropped)} in tasks.json and dropped the archive's copy (an interrupted move)."
        )
    if not ids:
        out("Nothing to archive.")
    elif dry_run:
        out(f"Would archive {len(ids)} task(s): {describe_ids(ids)}.")
    else:
        out(
            f"Archived {len(ids)} task(s): {describe_ids(ids)}. "
            f"tasks.json is now {shown['tasks_kb']} KB, tasks-archive.json {shown['archive_kb']} KB."
        )
    if kept:
        out(
            f"{'Would keep' if dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}."
        )


@db_app.command("unarchive")
def db_unarchive(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Bring archived tasks back into tasks.json."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would unarchive {', '.join(ids)}")
        return
    result = unarchive_tasks(open_project(os.getcwd()), ids)
    again = result["moves_back"]
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"Brought back {', '.join(ids)} from the archive.")
    if again:
        out(
            f"{describe_ids(again)} {'is' if len(again) == 1 else 'are'} finished: the next `atm archive` moves "
            f"{'it' if len(again) == 1 else 'them'} back, unless reopened (`atm reopen <id> --reason …`)."
        )


@db_app.command("init")
def db_init(
    ctx: typer.Context,
    meta_file: Optional[str] = typer.Option(
        None, "--meta-file", help="Set tasks.json meta from a JSON object"
    ),
) -> None:
    """Give tasks.json a meta.id and register the merge driver."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would give tasks.json a meta.id and register the merge driver")
        return
    project = open_project(os.getcwd())
    if meta_file:
        new = read_json(meta_file)
        if not isinstance(new, dict):
            raise Invalid(f"{meta_file} must hold a JSON object: the meta keys to set")
        created, keys = init_meta(project, new)
        project = open_project(os.getcwd())  # the task data as it is now
        if not ctx.obj.get("json_out", False):
            out(("Created tasks.json; meta: " if created else "Updated tasks.json meta: ") + (", ".join(keys) or "-"))
    project_id = ensure_project_id(project)
    repository = ensure_repository(project)
    register_project(project)  # the dashboard lists every project atm runs in
    setup = set_up_store(project)
    if ctx.obj.get("json_out", False):
        out(
            to_json(
                {
                    "project_id": project_id,
                    "repository": repository,
                    "git": setup["git"],
                    "gitattributes_added": setup["gitattributes_added"],
                }
            )
        )
        return
    if project_id:
        out(f"Gave this project an id ({project_id}) in tasks.json meta, for the dashboard's saved view.")
    if repository:
        out(f"Recorded the repository address ({repository}), for commit and PR links on the dashboard.")
    if not setup["git"]:
        out("Not in a git repository: there is no merge driver to set up.")
        return
    out(
        "Registered the tasks.json merge driver (git config merge.atm)"
        + (
            " and added .agentic-task-manager/.gitattributes (staged)."
            if setup["gitattributes_added"]
            else "; .gitattributes already set."
        )
    )


# ---------------------------------------------------------------------------
# release group
# ---------------------------------------------------------------------------

release_app = typer.Typer(help="Release management")
app.add_typer(release_app, name="release")


@release_app.command("list")
def release_list(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="List archived releases too"),
) -> None:
    """List releases with their tasks by kind and stage."""
    project = open_project(os.getcwd())
    listed = list_releases(project, all_)
    if ctx.obj.get("json_out", False):
        out(to_json(listed["releases"]))
        return
    if not listed["releases"]:
        out(
            "No open releases (--all lists the archived ones)."
            if listed["has_releases"]
            else "No releases yet: atm releases add <R>."
        )
        return
    for r in listed["releases"]:
        details = [r["state"]] + ([f"target {r['target']}"] if r.get("target") else [])
        if r.get("released_at"):
            details.append(f"released {r['released_at'][:10]}")
        if isinstance(r.get("pr"), dict) and r["pr"].get("number"):
            details.append(f"PR #{r['pr']['number']}")
        title = r["id"] + (f" “{r['name']}”" if r.get("name") else "")
        kinds = ", ".join(f"{k} {n}" for k, n in r["by_kind"].items())
        stages = ", ".join(f"{s.replace('_', ' ')} {n}" for s, n in r["by_stage"].items())
        line = f"{title} · {' · '.join(details)}: {r['tasks']} task{'' if r['tasks'] == 1 else 's'}"
        line = line + (f" ({kinds})" if kinds else "") + (f"; {stages}" if stages else "")
        out(line)


@release_app.command("add")
def release_add(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
    pr: Optional[list[int]] = typer.Option(None, "--pr", metavar="N", help="Add: give it the tasks of pull request N (repeatable)"),
    released_at: Optional[str] = typer.Option(None, "--released-at", metavar="TIME", help="Add: it shipped already, at this time (e.g. its PR's merge)"),
) -> None:
    """Add a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name:
            parts.append(f"--name {name}")
        if target:
            parts.append(f"--target {target}")
        if pr:
            for n in pr:
                parts.append(f"--pr {n}")
        if released_at:
            parts.append(f"--released-at {released_at}")
        out(f"[dry-run] would add release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_release(
        project, release_id, name=name, target=target, prs=pr or [], released_at=released_at
    )
    if ctx.obj.get("json_out", False):
        out(to_json(result["release"]))
        return
    release, given, prs = result["release"], result["tasks"], pr or []
    line = f'Added the release “{release_id}” ({release["state"]})'
    if given:
        numbers = ", ".join(f"#{n}" for n in prs)
        line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
    out(line + ".")


@release_app.command("edit")
def release_edit(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
) -> None:
    """Edit a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name is not None:
            parts.append(f"--name {name}")
        if target is not None:
            parts.append(f"--target {target}")
        out(f"[dry-run] would edit release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_release(project, release_id, name=name, target=target)
    release, changed = result["release"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")


@release_app.command("archive")
def release_archive(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Archive a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would archive release {release_id}")
        return
    project = open_project(os.getcwd())
    release = archive_release(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"Archived the release “{release['id']}”.")


@release_app.command("notes")
def release_notes_cmd(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Print a Markdown draft of a release's changes."""
    project = open_project(os.getcwd())
    notes = release_notes(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(notes))
        return
    release = notes["release"]
    lines = [f"# {release['id']}" + (f" “{release['name']}”" if release.get("name") else "")]
    if not notes["groups"]:
        lines += ["", "Nothing done in it yet."]
    for group in notes["groups"]:
        lines += ["", f"## {group['label']}", ""]
        for t in group["tasks"]:
            lines.append(f"- **{t['id']}** {t['name']}" + (f": {t['outcome']}" if t["outcome"] else ""))
    if notes["open"]:
        lines += ["", "## Not done yet", ""]
        lines += [f"- {t['id']} {t['name']} ({t['status']})" for t in notes["open"]]
    for line in lines:
        out(line)


# ---------------------------------------------------------------------------
# component group
# ---------------------------------------------------------------------------

component_app = typer.Typer(help="Component management")
app.add_typer(component_app, name="component")


@component_app.command("list")
def component_list(
    ctx: typer.Context,
) -> None:
    """List components with their task counts."""
    project = open_project(os.getcwd())
    listed = list_components(project)
    if ctx.obj.get("json_out", False):
        out(to_json(listed))
        return
    if not listed:
        out("No components yet: atm components add <id> --path GLOB.")
        return
    for c in listed:
        details = [f"“{c['label']}”"] if c.get("label") else []
        details.append(", ".join(c["paths"]) if c["paths"] else "no paths")
        if c.get("review"):
            details.append(f"review {c['review']}")
        out(f"{c['id']} · {' · '.join(details)}: {c['tasks']} task{'' if c['tasks'] == 1 else 's'}")


@component_app.command("add")
def component_add(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown"),
    path: Optional[list[str]] = typer.Option(None, "--path", metavar="GLOB", help="Add: a glob over its files, e.g. 'dashboard/**' (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Add a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label:
            parts.append(f"--label {label}")
        if path:
            for p in path:
                parts.append(f"--path {p}")
        if review:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would add component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_component(project, component_id, label, path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    text = moved_text(from_label, result["from_label"], assign, result["by_files"])
    out(f"Added the component {component_id}" + (f": {text}." if text else "."))


@component_app.command("edit")
def component_edit(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown (add: default the label's own text, with --from-label; edit: empty removes it)"),
    add_path: Optional[list[str]] = typer.Option(None, "--add-path", metavar="GLOB", help="Edit: add a glob over its files (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed (blocker, async, batch, none; edit: empty removes it)"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Edit a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label is not None:
            parts.append(f"--label {label}")
        if add_path:
            for p in add_path:
                parts.append(f"--add-path {p}")
        if review is not None:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would edit component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_component(project, component_id, label, add_path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    moved = moved_text(from_label, result["from_label"], assign, result["by_files"])
    text = ", ".join(([f"changed {', '.join(result['changed'])}"] if result["changed"] else []) + [t for t in [moved] if t])
    out(f"{component_id}: {text or 'nothing changed'}")


# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------

project_app = typer.Typer(help="Project-level config")
app.add_typer(project_app, name="project")


def _choice(name, value, choices):
    """A value that must be one of `choices` (or not given), refused as atm's argparse refuses it (exit code 2)."""
    if value is not None and value not in choices:
        raise Invalid(f"{name}: invalid choice '{value}' (choose from {', '.join(choices)})")
    return value


def _dry(ctx, what):
    """With --dry-run, say what would be done and stop; otherwise go on."""
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {what}")
        return True
    return False


@project_app.command("config")
def project_config(
    ctx: typer.Context,
) -> None:
    """Print where this project's task files are."""
    show_paths(task_paths(os.getcwd()), ctx.obj.get("json_out", False))


@project_app.command("summary", hidden=True)
def project_summary_cmd(ctx: typer.Context) -> None:
    """The project's numbers as atm serve gives them to the dashboard (JSON; the dashboard's dev server reads it)."""
    out(to_json(project_summary(open_project(os.getcwd()))))


@project_app.command("status")
def project_status(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint", help="Only this sprint's tasks (repeatable)"),
    backlog: bool = typer.Option(False, "--backlog", help="Only the tasks with no sprint"),
    release: Optional[str] = typer.Option(None, "--release", help="Only the tasks in this release"),
    component: Optional[str] = typer.Option(None, "--component", help="Only the tasks in this component"),
) -> None:
    """The overview: counts, what is in progress, in review or blocked, the next task, the file's size."""
    status = status_report(
        open_project(os.getcwd()),
        Filters(sprint=sprint or None, backlog=backlog, release=release, component=component),
    )
    show_status(status, ctx.obj.get("json_out", False))


@project_app.command("workflow")
def project_workflow(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set: change the workflow"),
    wf_type: Optional[str] = typer.Argument(None, help="direct or pull-request"),
    base: Optional[str] = typer.Option(None, "--base", help="The branch PRs target, or that direct pushes to"),
    verify: Optional[str] = typer.Option(None, "--verify", help="Run the project's verify before pushing (true|false)"),
    max_rounds: Optional[int] = typer.Option(None, "--max-rounds", help="CI fix rounds before /atm:push stops"),
    commit_template: Optional[str] = typer.Option(
        None, "--commit-template", metavar="TEXT", help='Commit messages, e.g. "feat({id}): {name}"'
    ),
    trailers: Optional[str] = typer.Option(
        None, "--trailers", help="forbid Co-Authored-By lines, require the --trailer lines, or none (the agent's)"
    ),
    trailer: Optional[list[str]] = typer.Option(None, "--trailer", metavar="LINE", help="A required trailer line"),
) -> None:
    """Print the project's workflow, or set it (atm2 project workflow set TYPE)."""
    _choice("action", action, ["set"])
    _choice("--verify", verify, ["true", "false"])
    _choice("--trailers", trailers, ["forbid", "require", "none"])
    project = open_project(os.getcwd())
    if action is None:
        workflow = get_workflow(project)
    else:
        if _dry(ctx, f"set the workflow to {wf_type}"):
            return
        workflow = set_workflow(
            project,
            wf_type,
            base=base,
            verify=None if verify is None else verify == "true",
            max_rounds=max_rounds,
            commit_template=commit_template,
            trailers=trailers,
            trailer=trailer or [],
        )
    if ctx.obj.get("json_out", False):
        out(to_json(workflow or {"type": None}))
    else:
        out(describe_workflow(workflow, project.vcs.default_base))


@project_app.command("logo")
def project_logo(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set or unset it"),
    path: Optional[str] = typer.Argument(None, help="set: the image (SVG, PNG, WebP or ICO), relative to here"),
) -> None:
    """Print, set or unset the project's logo for the dashboard (meta.logo)."""
    _choice("action", action, ["set", "unset"])
    if action is not None and _dry(ctx, f"{action} the logo"):
        return
    logo_action(open_project(os.getcwd()), action, path, ctx.obj.get("json_out", False))


@project_app.command("list")
def project_list(ctx: typer.Context) -> None:
    """List the projects the dashboard shows."""
    projects_action(open_registry(), "list", None, ctx.obj.get("json_out", False))


@project_app.command("add")
def project_add(
    ctx: typer.Context,
    target: Optional[str] = typer.Argument(None, help="The project's folder (default: here)"),
) -> None:
    """Add a project to the dashboard's list."""
    if _dry(ctx, f"add {target or 'this folder'} to the dashboard's projects"):
        return
    projects_action(open_registry(), "add", target, ctx.obj.get("json_out", False))


@project_app.command("remove")
def project_remove(
    ctx: typer.Context,
    target: Annotated[str, typer.Argument(help="The project's ID or path")],
) -> None:
    """Remove a project from the dashboard's list."""
    if _dry(ctx, f"remove {target} from the dashboard's projects"):
        return
    projects_action(open_registry(), "remove", target, ctx.obj.get("json_out", False))


@project_app.command("prune")
def project_prune(ctx: typer.Context) -> None:
    """Drop the projects whose folder is gone from the dashboard's list."""
    if _dry(ctx, "drop the projects whose folder is gone"):
        return
    projects_action(open_registry(), "prune", None, ctx.obj.get("json_out", False))


# ---------------------------------------------------------------------------
# sprint group
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# release group
# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------


@app.command("doctor", rich_help_panel=MAINTENANCE)
def doctor(
    ctx: typer.Context,
    files: bool = typer.Option(False, "--files", help="Also warn about missing files"),
) -> None:
    """Validate tasks.json: schema, vocabularies, dependencies."""
    result = check_project(open_project(os.getcwd()), files=files)
    errors, warnings = result["errors"], result["warnings"]
    if ctx.obj.get("json_out", False):
        out(to_json({"errors": errors, "warnings": warnings}))
    else:
        for e in errors:
            out(f"error: {e}")
        for w in warnings:
            out(f"warning: {w}")
        if not errors:
            count = result["count"]
            msg = f"tasks.json is fine ({count} tasks"
            if warnings:
                msg += f", {len(warnings)} warnings"
            msg += ")"
            out(msg)
    if errors:
        raise AtmExit(1)


# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------


# ── helpers ported from atm cli/serve.py ──────────────────────────────────


def server_url(port, project=None):
    return f"http://localhost:{port}/" + (f"#project={project}" if project else "")


def live_server(home):
    """The running server's record, or None. A stale record is removed."""
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        return None
    info = whoami(record.get("port"))
    if info and info.get("pid") == record.get("pid"):
        return record
    os.remove(record_path)
    return None


def open_browser(url):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.Popen([opener, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        out(f"Could not open a browser; open {url} yourself.")


@contextlib.contextmanager
def server_lock(home):
    """Only one `atm serve` at a time looks for a running server and starts one: a directory, which mkdir creates for
    exactly one process. A lock whose holder is gone (its pid isn't running) or that outlives the wait is broken."""
    ensure_home(home)
    lock = os.path.join(home, SERVER_LOCK)
    deadline = time.time() + SERVER_LOCK_WAIT
    while True:
        try:
            os.mkdir(lock)
            break
        except FileExistsError:
            holder = read_json(os.path.join(lock, "pid.json")) or {}
            gone = isinstance(holder.get("pid"), int) and not pid_alive(holder["pid"])
            if gone or time.time() >= deadline:
                shutil.rmtree(lock, ignore_errors=True)
                continue
            time.sleep(0.05)
    try:
        with open(os.path.join(lock, "pid.json"), "w", encoding="utf-8") as f:
            f.write(to_json({"pid": os.getpid()}))
        yield
    finally:
        shutil.rmtree(lock, ignore_errors=True)


def ensure_server(home, port):
    """The running server's record, starting the server process if none runs. Returns (record, started)."""
    with server_lock(home):
        record = live_server(home)
        if record:
            return record, False
        return start_server(home, port), True


def start_server(home, port):
    """Start the server process and wait for its record."""
    # Find atm.py: works when running from built atm.py (SCRIPT_DIR/atm.py exists)
    # and from source (plugin/scripts/atm.py relative to repo root).
    atm_py = os.path.join(SCRIPT_DIR, "atm.py")
    if not os.path.exists(atm_py):
        atm_py = os.path.join(
            os.path.dirname(os.path.dirname(os.path.dirname(SCRIPT_DIR))),
            "plugin", "scripts", "atm.py",
        )
    with open(os.path.join(home, SERVER_LOG), "ab") as log:
        child = subprocess.Popen(
            [sys.executable, atm_py, SERVER_ENTRY, home, str(port)],
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
    deadline = time.time() + 10
    while time.time() < deadline:
        record = read_json(os.path.join(home, SERVER_RECORD))
        if record and record.get("pid") == child.pid and whoami(record["port"]):
            return record
        if child.poll() is not None:
            break
        time.sleep(0.05)
    raise AtmFailure(f"the dashboard server did not start; see {os.path.join(home, SERVER_LOG)}")


def serve_start(cwd, port, overview):
    """Start (or reuse) the server; returns the URL to show."""
    project = None
    if not overview:
        migration = migrate_folder(cwd, register_it=False)
        for line in migration["lines"]:
            if line != "Nothing to migrate.":
                out(line)
        if not migration["tasks_exists"]:
            raise AtmFailure("no tasks.json to serve. Run /atm:create-tasks first.")
        project, shown = dashboard_id(open_project(cwd))
        if not shown:
            out("This project isn't listed on the dashboard (meta.dashboard.listed is false).")
            project = None
    home = atm_home()
    record, started = ensure_server(home, port)
    url = server_url(record["port"], project)
    out(f"Dashboard server {'started' if started else 'already running'}: {url}")
    return url


def wait_gone(pid, seconds):
    deadline = time.time() + seconds
    while time.time() < deadline and pid_alive(pid):
        time.sleep(0.05)
    return not pid_alive(pid)


def stop_server(home, pid):
    """Stop a server of the user's (whose pid whoami confirmed)."""
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    if not wait_gone(pid, STOP_WAIT):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if not wait_gone(pid, KILL_WAIT):
            raise AtmFailure(f"the dashboard server (pid {pid}) didn't stop; its record is kept")
    record_path = os.path.join(home, SERVER_RECORD)
    if (read_json(record_path) or {}).get("pid") == pid:
        os.remove(record_path)


def unrecorded_servers(home, first_port, record=None):
    """Servers of this atm folder answering on the usual ports that the record doesn't name."""
    mine = home_id(home)
    found = []
    for port in range(first_port, first_port + SCAN_PORTS):
        info = whoami(port, timeout=0.5)
        if info and info.get("home") == mine and info.get("pid") != (record or {}).get("pid"):
            found.append(info)
    return found


def serve_stop(home, port):
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    live = bool(record) and (whoami(record.get("port")) or {}).get("pid") == record.get("pid")
    strays = unrecorded_servers(home, port, record if live else None)
    if record is None and not strays:
        out("Dashboard server is not running.")
        return
    if record is not None and not live:
        os.remove(record_path)
        if not strays:
            out(f"Dashboard server is not running (removed a stale record for pid {record.get('pid')}).")
            return
    if live:
        stop_server(home, record["pid"])
        out(f"Dashboard server stopped (pid {record['pid']}).")
    for info in strays:
        stop_server(home, info["pid"])
        out(f"Stopped a dashboard server the record didn't name (pid {info['pid']}, port {info['port']}).")


def outdated(record):
    """True when the server runs an older dashboard than this plugin's."""
    mine, mine_build = dashboard_head(TEMPLATE) or ("", "")
    theirs, theirs_build = str(record.get("version") or ""), str(record.get("build") or "")
    if parse_version(mine) != parse_version(theirs):
        return parse_version(mine) > parse_version(theirs)
    theirs_root = record.get("plugin_root")
    if theirs_root and os.path.realpath(str(theirs_root)) != os.path.realpath(PLUGIN_ROOT):
        return False
    return bool(mine_build) and mine_build != theirs_build


def refresh_server(home=None):
    """Restart the server on the same port if it runs an older dashboard."""
    home = home or atm_home()
    record = read_json(os.path.join(home, SERVER_RECORD))
    if not isinstance(record, dict) or not outdated(record):
        return None
    with server_lock(home):
        live = live_server(home)
        if not live:
            return None
        stop_server(home, live["pid"])
        return start_server(home, live["port"])


def serve_refresh(home):
    record = refresh_server(home)
    if record:
        out(f"Dashboard server restarted with dashboard {record.get('version')}: {server_url(record['port'])}")


def serve_status(home, as_json, port):
    record = live_server(home)
    strays = unrecorded_servers(home, port, record)
    if as_json:
        url = server_url(record["port"]) if record else None
        unrecorded = [{"pid": s["pid"], "port": s["port"], "version": s.get("version")} for s in strays]
        out(to_json({"running": bool(record), **(record or {}), "url": url, "unrecorded": unrecorded}))
        return
    if not record:
        out("Dashboard server is not running.")
    else:
        up = format_duration((datetime.now(timezone.utc) - parse_iso(record["started_at"])).total_seconds())
        out(
            f"Dashboard server running: {server_url(record['port'])} "
            f"(pid {record['pid']}, dashboard {record.get('version')}, up {up})"
        )
    for info in strays:
        out(
            f"Also running, without a record: pid {info['pid']} on port {info['port']} "
            f"(dashboard {info.get('version')}); `atm serve stop` stops it."
        )


@app.command("service", rich_help_panel=MAINTENANCE)
def service(
    ctx: typer.Context,
    action: str = typer.Argument("start", help="start|stop|status"),
    open_: bool = typer.Option(False, "--open", help="Also open the dashboard in the browser"),
    all_projects: bool = typer.Option(False, "--all", help="Show all projects overview"),
    port: int = typer.Option(DEFAULT_PORT, "--port", help="The port to try first"),
    dir_: Optional[str] = typer.Option(None, "--dir", help="The project to show"),
    refresh: bool = typer.Option(False, "--refresh", help="Only restart if dashboard is older"),
) -> None:
    """Run the dashboard server in the background."""
    cwd = os.path.abspath(dir_) if dir_ else os.getcwd()
    if not os.path.isdir(cwd):
        raise AtmFailure(f"--dir {dir_}: not a directory", 2)
    home = atm_home()
    dry_run = ctx.obj.get("dry_run", False)
    if refresh:
        if dry_run:
            out("[dry-run] would refresh the dashboard server if outdated")
        else:
            serve_refresh(home)
    elif action == "start":
        if dry_run:
            out("[dry-run] would start the dashboard server")
        else:
            refresh_server(home)
            url = serve_start(cwd, port, all_projects)
            if open_:
                open_browser(url)
    elif action == "stop":
        if dry_run:
            out("[dry-run] would stop the dashboard server")
        else:
            serve_stop(home, port)
    else:
        serve_status(home, ctx.obj.get("json_out", False), port)


@app.command("version", rich_help_panel=META)
def version(ctx: typer.Context) -> None:
    """Print the plugin version."""
    out(to_json({"version": plugin_version()}) if ctx.obj.get("json_out", False) else plugin_version())


@app.command("help", rich_help_panel=META)
def help_(ctx: typer.Context) -> None:
    """Show the list of commands."""
    usage = ctx.parent.get_help()
    out(to_json({"usage": usage}) if ctx.obj.get("json_out", False) else usage)


# ---------------------------------------------------------------------------
# shell completion (T-423)
# ---------------------------------------------------------------------------
# Typer installs the scripts (--install-completion, --show-completion; zsh, bash, fish); these complete atm's own
# values, read through atm-lib in the folder the shell is in. Completion never fails loudly: outside a project, or
# with a task file atm can't read, it completes nothing.

# The targets of `atm2 task state` (task_state's cmd_map), with what each runs
STATE_TARGETS = [
    ("in_progress", "start"),
    ("in_review", "review"),
    ("done", "done"),
    ("todo", "back to to do: fail, release, reject, unblock or reopen"),
    ("blocked", "block"),
    ("skipped", "skip"),
    ("approved", "approve"),
    ("changes_requested", "request-changes"),
]


def _vocabulary():
    return backlog_tasks(open_project(os.getcwd()))["vocab"]


def _task_ids():
    return [(v["task"]["id"], v["task"].get("name") or "") for v in task_views(open_project(os.getcwd()), Filters())]


def _sprints():
    return [(s["id"], s.get("goal") or s.get("state") or "") for s in _vocabulary()["sprints"] if s.get("state") != "closed"]


def _releases():
    return [(r["id"], r.get("name") or r.get("state") or "") for r in _vocabulary()["releases"]]


def _components():
    return [(c["id"], c.get("label") or "") for c in _vocabulary()["components"]]


def _terms(key):
    return lambda: [(t["id"], t.get("label") or "") for t in _vocabulary()[key]]


def _completer(values):
    """A Click completer for `values()` ((value, help) pairs), matching what has been typed; nothing on any error."""

    def complete(ctx, param, incomplete):
        from click.shell_completion import CompletionItem

        try:
            pairs = values()
        except Exception:  # completion must never break the shell, whatever the project holds
            return []
        return [CompletionItem(value, help=help or None) for value, help in pairs if value.startswith(incomplete)]

    return complete


# (command path, parameter name) -> completer; a None path is any command with that parameter
COMPLETIONS = {
    (None, "task_id"): _task_ids,
    (None, "task_ids"): _task_ids,
    (None, "ids"): _task_ids,
    (None, "sprint"): _sprints,
    (None, "release"): _releases,
    (None, "component"): _components,
    (None, "status"): _terms("statuses"),
    (None, "type_"): _terms("types"),
    (None, "priority"): _terms("priorities"),
    ("task state", "next_state"): lambda: STATE_TARGETS,
    ("db unarchive", "ids"): None,  # archived tasks, which the task list doesn't hold
    **{(f"sprint {c}", "name"): _sprints for c in ("edit", "start", "close", "delete")},
    ("sprint close", "carry_to"): _sprints,
    ("sprint delete", "move_to"): _sprints,
    ("sprint move", "to"): _sprints,
    **{(f"release {c}", "release_id"): _releases for c in ("edit", "archive", "notes")},
    ("component edit", "component_id"): _components,
}


def _attach(cmd, path):
    for param in getattr(cmd, "params", []):
        key = (path, param.name) if (path, param.name) in COMPLETIONS else (None, param.name)
        values = COMPLETIONS.get(key)
        if values is not None:
            param._custom_shell_complete = _completer(values)
    for name, sub in getattr(cmd, "commands", {}).items():
        _attach(sub, f"{path} {name}".strip())


def command():
    """atm2's Click command, with atm's values to complete (what atm2_main runs)."""
    cmd = typer.main.get_command(app)
    _attach(cmd, "")
    return cmd


if __name__ == "__main__":
    command()(prog_name="atm2")
