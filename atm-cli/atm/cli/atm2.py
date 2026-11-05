"""atm2: a Typer-based CLI entry point for `atm`.

This is an experimental CLI that delegates to the existing `atm` commands via
subprocess. It evaluates Typer's ergonomics (nested groups, type-hinted
options, auto-generated help) without touching the `atm` command surface.

Each subcommand builds an argument list for `atm <cmd>` and runs it as a
subprocess, passing through stdout, stderr, and the exit code.
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

from ..boot.jsongit import dashboard_id, migrate_folder
from ..boot.open import open_project
from ..common.core import (
    AtmError, AtmExit, out, to_json, atm_home, ensure_home,
    format_duration, parse_iso, parse_version, pid_alive,
    read_json, whoami, PLUGIN_ROOT, SCRIPT_DIR,
)
from ..common.dashboard import (
    SCAN_PORTS, SERVER_LOG, SERVER_RECORD, SERVER_ENTRY, TEMPLATE,
    dashboard_head, home_id, server_command, DEFAULT_PORT,
)
from ..common.errors import AtmFailure
from ..lib.edit import check_project
from ._delegate import run_atm, run_atm_capture


# ── constants ported from atm cli/serve.py ──────────────────────────────────

SERVER_LOCK = "server.lock"
SERVER_LOCK_WAIT = 15.0
STOP_WAIT = 5.0
KILL_WAIT = 2.0

app = typer.Typer(
    name="atm2",
    help="atm2: Typer-based ATM CLI (delegates to `atm` commands)",
    no_args_is_help=True,
    rich_markup_mode="rich",
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
) -> None:
    """List tasks: one line per task: ID, status, priority, labels, name."""
    args: list[str] = ["list"]
    if status:
        for s in status:
            args += ["--status", s]
    if sprint:
        for s in sprint:
            args += ["--sprint", s]
    if backlog:
        args.append("--backlog")
    if release:
        args += ["--release", release]
    if component:
        args += ["--component", component]
    if type_:
        args += ["--type", type_]
    if priority:
        args += ["--priority", priority]
    if label:
        for l in label:
            args += ["--label", l]
    if agent:
        args += ["--agent", agent]
    if ready:
        args.append("--ready")
    if blocked:
        args.append("--blocked")
    if in_pr:
        args.append("--in-pr")
    if with_user:
        args.append("--with-user")
    if archived:
        args.append("--archived")
    if derived:
        args.append("--derived")
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("add")
def task_add(
    ctx: typer.Context,
    name: str = typer.Option(..., "--name", help="A short name (about 60 characters)"),
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
) -> None:
    """Add a task with the next ID, or many from a JSON file."""
    args = ["add"]
    args += ["--name", name]
    if type_:
        args += ["--type", type_]
    if sprint:
        args += ["--sprint", sprint]
    if release:
        args += ["--release", release]
    if priority:
        args += ["--priority", priority]
    if label:
        for l in label:
            args += ["--label", l]
    if description:
        args += ["--description", description]
    if description_file:
        args += ["--description-file", description_file]
    if dep:
        for d in dep:
            args += ["--dep", d]
    if file:
        for f in file:
            args += ["--file", f]
    if requirement:
        for r in requirement:
            args += ["--requirement", r]
    if component:
        for c in component:
            args += ["--component", c]
    if components_auto:
        args += ["--components", "auto"]
    if review:
        args += ["--review", review]
    if related:
        for r in related:
            args += ["--related", r]
    if with_user:
        args += ["--with", with_user]
    if json_file:
        args += ["--json-file", json_file]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


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
) -> None:
    """Change a task's definition (not its status: use `atm2 task state`)."""
    args = ["edit", task_id]
    if name:
        args += ["--name", name]
    if description:
        args += ["--description", description]
    if description_file:
        args += ["--description-file", description_file]
    if sprint:
        args += ["--sprint", sprint]
    if backlog:
        args.append("--backlog")
    if type_:
        args += ["--type", type_]
    if priority:
        args += ["--priority", priority]
    if release:
        args += ["--release", release]
    if review:
        args += ["--review", review]
    if add_file:
        for f in add_file:
            args += ["--add-file", f]
    if remove_file:
        for f in remove_file:
            args += ["--remove-file", f]
    if add_dep:
        for d in add_dep:
            args += ["--add-dep", d]
    if remove_dep:
        for d in remove_dep:
            args += ["--remove-dep", d]
    if add_label:
        for l in add_label:
            args += ["--add-label", l]
    if remove_label:
        for l in remove_label:
            args += ["--remove-label", l]
    if add_component:
        for c in add_component:
            args += ["--add-component", c]
    if remove_component:
        for c in remove_component:
            args += ["--remove-component", c]
    if add_related:
        for r in add_related:
            args += ["--add-related", r]
    if remove_related:
        for r in remove_related:
            args += ["--remove-related", r]
    if add_requirement:
        for r in add_requirement:
            args += ["--add-requirement", r]
    if remove_requirement:
        for r in remove_requirement:
            args += ["--remove-requirement", r]
    if add_use_case:
        for u in add_use_case:
            args += ["--add-use-case", u]
    if remove_use_case:
        for u in remove_use_case:
            args += ["--remove-use-case", u]
    if components_auto:
        args += ["--components", "auto"]
    if with_user:
        args += ["--with", with_user]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("details")
def task_details(
    ctx: typer.Context,
    task_ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Tasks in full (description, dependencies, files, notes)."""
    args = ["show"] + task_ids
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("note")
def task_note(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    text: Annotated[str, typer.Argument(help="The note text")],
) -> None:
    """Add a note to a task (at most 500 characters)."""
    args = ["note", task_id, text]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("deps")
def task_deps(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
) -> None:
    """What a task waits for, and what waits for it."""
    sys.exit(run_atm(["deps", task_id]))


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

    # `todo` from review means reject; `todo` from blocked means unblock; `todo` from done/skipped means reopen
    # We can't know the current state without asking `atm show`, so default `todo` -> reject (from review).
    # The user can override with --on/--answer for unblock, or use the specific atm command directly.
    if next_state == "todo":
        cmd = "reject"  # safest default: task in review back to todo

    args = [cmd, task_id]
    if note:
        args += ["--note", note]
    if reason:
        args += ["--reason", reason]
    if commit:
        args += ["--commit", commit]
    if resolution:
        args += ["--resolution", resolution]
    if duplicate_of:
        args += ["--duplicate-of", duplicate_of]
    if on:
        args += ["--on", on]
    if question:
        for q in question:
            args += ["--question", q]
    if answer:
        for a in answer:
            args += ["--answer", a]
    if status:
        args += ["--status", status]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# loop group
# ---------------------------------------------------------------------------

loop_app = typer.Typer(help="The work loop")
app.add_typer(loop_app, name="loop")


@loop_app.command("next")
def loop_next(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    tasks: Optional[str] = typer.Option(None, "--tasks"),
    all_tasks: bool = typer.Option(False, "--all"),
    backlog: bool = typer.Option(False, "--backlog"),
    type_: Optional[str] = typer.Option(None, "--type"),
    with_user: bool = typer.Option(False, "--with-user"),
    claim: bool = typer.Option(False, "--claim", help="Also start and claim it, atomically"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """The task /atm:loop-tasks works on next (exit code 3 if there is none)."""
    args = ["next"]
    if sprint:
        for s in sprint:
            args += ["--sprint", s]
    if tasks:
        args += ["--tasks", tasks]
    if all_tasks:
        args.append("--all")
    if backlog:
        args.append("--backlog")
    if type_:
        args += ["--type", type_]
    if with_user:
        args.append("--with-user")
    if claim:
        args.append("--claim")
    if ctx.obj.get("json_out", False):
        args.append("--json")
    if agent:
        args += ["--agent", agent]
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@loop_app.command("run")
def loop_run(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    cmd_and_args: Annotated[
        list[str], typer.Argument(help="The command and its args, e.g. -- npm test --verbose")
    ],
    every: Optional[float] = typer.Option(None, "--every", help="Renew claim this often (seconds)"),
) -> None:
    """Run a long command while keeping your claim on a task alive."""
    args = ["run", task_id]
    if every:
        args += ["--every", str(every)]
    args += ["--"] + cmd_and_args
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# claim group
# ---------------------------------------------------------------------------

claim_app = typer.Typer(help="Claim lifecycle")
app.add_typer(claim_app, name="claim")


@claim_app.command("list")
def claim_list(
    ctx: typer.Context,
) -> None:
    """List live claims."""
    args = ["claims"]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@claim_app.command("heartbeat")
def claim_heartbeat(
    ctx: typer.Context,
    task_id: Optional[str] = typer.Argument(None, help="The task ID"),
    auto: bool = typer.Option(False, "--auto", help="Renew claims silently (plugin's hook)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Extend your claim's lease."""
    args = ["heartbeat"]
    if task_id:
        args.append(task_id)
    if auto:
        args.append("--auto")
    if agent:
        args += ["--agent", agent]
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


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
    args = ["commit", task_id]
    if file:
        for f in file:
            args += ["--file", f]
    if dir_:
        for d in dir_:
            args += ["--dir", d]
    if message:
        args += ["--message", message]
    if record:
        args += ["--record", record]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@vcs_app.command("pr")
def vcs_pr(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    number: int = typer.Option(..., "--number", help="The PR number"),
    url: str = typer.Option(..., "--url", help="The PR's URL"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record the PR that contains the tasks."""
    args = ["pr"] + ids + ["--number", str(number), "--url", url]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@vcs_app.command("pushed")
def vcs_pushed(
    ctx: typer.Context,
    pr: int = typer.Option(..., "--pr", help="The PR number"),
    at: Optional[str] = typer.Option(None, "--at", help="When it was merged (ISO 8601 UTC)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record that a PR was merged, on the tasks it contains."""
    args = ["merged", "--pr", str(pr)]
    if at:
        args += ["--at", at]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


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
    args = ["repository"]
    if url:
        args.append(url)
    if old:
        args += ["--from", old]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# db group
# ---------------------------------------------------------------------------

db_app = typer.Typer(help="Database / file maintenance")
app.add_typer(db_app, name="db")


@db_app.command("migrate")
def db_migrate(
    ctx: typer.Context,
) -> None:
    """Move tasks.json into .agentic-task-manager/ and set up the folder."""
    args = ["migrate"]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("archive")
def db_archive(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    before: Optional[str] = typer.Option(None, "--before"),
    keep_days: Optional[float] = typer.Option(None, "--keep-days"),
) -> None:
    """Move finished tasks to tasks-archive.json."""
    args = ["archive"]
    if sprint:
        for s in sprint:
            args += ["--sprint", s]
    if before:
        args += ["--before", before]
    if keep_days:
        args += ["--keep-days", str(keep_days)]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("unarchive")
def db_unarchive(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Bring archived tasks back into tasks.json."""
    args = ["unarchive"] + ids
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("init")
def db_init(
    ctx: typer.Context,
    meta_file: Optional[str] = typer.Option(
        None, "--meta-file", help="Set tasks.json meta from a JSON object"
    ),
) -> None:
    """Give tasks.json a meta.id and register the merge driver."""
    args = ["init"]
    if meta_file:
        args += ["--meta-file", meta_file]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# project group
# ---------------------------------------------------------------------------

project_app = typer.Typer(help="Project-level config")
app.add_typer(project_app, name="project")


@project_app.command("config")
def project_config(
    ctx: typer.Context,
) -> None:
    """Print where this project's task files are."""
    args = ["paths"]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("status")
def project_status(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    backlog: bool = typer.Option(False, "--backlog"),
    release: Optional[str] = typer.Option(None, "--release"),
    component: Optional[str] = typer.Option(None, "--component"),
) -> None:
    """The project overview: counts, what is in progress, in review or blocked."""
    args = ["status"]
    if sprint:
        for s in sprint:
            args += ["--sprint", s]
    if backlog:
        args.append("--backlog")
    if release:
        args += ["--release", release]
    if component:
        args += ["--component", component]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("workflow")
def project_workflow(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set: change the workflow"),
    wf_type: Optional[str] = typer.Argument(None, help="direct or pull-request"),
    base: Optional[str] = typer.Option(None, "--base"),
    verify: Optional[str] = typer.Option(None, "--verify"),
    max_rounds: Optional[int] = typer.Option(None, "--max-rounds"),
    commit_template: Optional[str] = typer.Option(None, "--commit-template"),
    trailers: Optional[str] = typer.Option(None, "--trailers"),
    trailer: Optional[list[str]] = typer.Option(None, "--trailer"),
) -> None:
    """Print the project's workflow, or set it."""
    args = ["workflow"]
    if action:
        args.append(action)
    if wf_type:
        args.append(wf_type)
    if base:
        args += ["--base", base]
    if verify:
        args += ["--verify", verify]
    if max_rounds:
        args += ["--max-rounds", str(max_rounds)]
    if commit_template:
        args += ["--commit-template", commit_template]
    if trailers:
        args += ["--trailers", trailers]
    if trailer:
        for t in trailer:
            args += ["--trailer", t]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("logo")
def project_logo(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set or unset"),
    path: Optional[str] = typer.Argument(None, help="The image (set)"),
) -> None:
    """Print, set or unset the project's logo."""
    args = ["logo"]
    if action:
        args.append(action)
    if path:
        args.append(path)
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("list")
def project_list(
    ctx: typer.Context,
    target: Optional[str] = typer.Argument(None, help="Add: project folder; Remove: ID or path"),
) -> None:
    """List the projects the dashboard shows."""
    args = ["projects", "list"]
    if target:
        args.append(target)
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


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
    """List sprints with task counts."""
    args = ["sprints", "list"]
    if all_:
        args.append("--all")
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@sprint_app.command("add")
def sprint_add(
    ctx: typer.Context,
    name: Optional[str] = typer.Argument(None, help="The sprint's name"),
    goal: Optional[str] = typer.Option(None, "--goal"),
    release: Optional[str] = typer.Option(None, "--release"),
    due: Optional[str] = typer.Option(None, "--due"),
    after: Optional[str] = typer.Option(None, "--after"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Add a sprint."""
    args = ["sprints", "add"]
    if name:
        args.append(name)
    if goal:
        args += ["--goal", goal]
    if release:
        args += ["--release", release]
    if due:
        args += ["--due", due]
    if after:
        args += ["--after", after]
    if agent:
        args += ["--agent", agent]
    if ctx.obj.get("json_out", False):
        args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# release group
# ---------------------------------------------------------------------------

release_app = typer.Typer(help="Release management")
app.add_typer(release_app, name="release")


# ---------------------------------------------------------------------------
# component group
# ---------------------------------------------------------------------------

component_app = typer.Typer(help="Component management")
app.add_typer(component_app, name="component")


# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------


@app.command("doctor")
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


@app.command("service")
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


if __name__ == "__main__":
    app()
