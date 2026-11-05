"""atm2: a Typer-based CLI entry point for `atm`.

This is an experimental CLI that delegates to the existing `atm` commands via
subprocess. It evaluates Typer's ergonomics (nested groups, type-hinted
options, auto-generated help) without touching the `atm` command surface.

Each subcommand builds an argument list for `atm <cmd>` and runs it as a
subprocess, passing through stdout, stderr, and the exit code.
"""

from __future__ import annotations

import sys
from typing import Annotated, Optional

import typer

from ._delegate import run_atm, run_atm_capture

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
) -> None:
    """Global options for all atm2 subcommands."""
    ctx.obj = {"dry_run": dry_run}

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
    component: Optional[str] = typer.Option(None, "--component", help="Only tasks in this component"),
    type_: Optional[str] = typer.Option(None, "--type", help="Only this task type"),
    priority: Optional[str] = typer.Option(None, "--priority", help="Only this priority"),
    label: Optional[list[str]] = typer.Option(None, "--label", help="Only tasks with this label (repeatable)"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Only tasks started by this agent"),
    ready: bool = typer.Option(False, "--ready", help="Only to-do tasks whose dependencies are done"),
    blocked: bool = typer.Option(False, "--blocked", help="Only blocked tasks and to-do tasks still waiting"),
    in_pr: bool = typer.Option(False, "--in-pr", help="Only tasks in a PR that isn't recorded as merged"),
    with_user: bool = typer.Option(False, "--with-user", help="Only tasks done together with the user"),
    archived: bool = typer.Option(False, "--archived", help="List the archive instead"),
    derived: bool = typer.Option(False, "--derived", help="Each task in full as JSON"),
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
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
    if json_out:
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
    dep: Optional[list[str]] = typer.Option(None, "--dep", help="A task it depends on (repeatable)"),
    file: Optional[list[str]] = typer.Option(None, "--file", help="A file it creates or changes (repeatable)"),
    requirement: Optional[list[str]] = typer.Option(None, "--requirement", help="A requirement (repeatable)"),
    component: Optional[list[str]] = typer.Option(None, "--component", help="A component (repeatable)"),
    components_auto: bool = typer.Option(False, "--components", help="Auto: components its files are in"),
    review: Optional[str] = typer.Option(None, "--review", help="How it is reviewed"),
    related: Optional[list[str]] = typer.Option(None, "--related", help="A related task (repeatable)"),
    with_user: Optional[str] = typer.Option(None, "--with", help="user: done together with the user"),
    json_file: Optional[str] = typer.Option(None, "--json-file", help="Add many from a JSON file"),
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
) -> None:
    """Add a task with the next ID, or many from a JSON file."""
    args = ["add"]
    args += ["--name", name]
    if type_: args += ["--type", type_]
    if sprint: args += ["--sprint", sprint]
    if release: args += ["--release", release]
    if priority: args += ["--priority", priority]
    if label:
        for l in label: args += ["--label", l]
    if description: args += ["--description", description]
    if description_file: args += ["--description-file", description_file]
    if dep:
        for d in dep: args += ["--dep", d]
    if file:
        for f in file: args += ["--file", f]
    if requirement:
        for r in requirement: args += ["--requirement", r]
    if component:
        for c in component: args += ["--component", c]
    if components_auto: args += ["--components", "auto"]
    if review: args += ["--review", review]
    if related:
        for r in related: args += ["--related", r]
    if with_user: args += ["--with", with_user]
    if json_file: args += ["--json-file", json_file]
    if json_out: args.append("--json")
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
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Change a task's definition (not its status: use `atm2 task state`)."""
    args = ["edit", task_id]
    if name: args += ["--name", name]
    if description: args += ["--description", description]
    if description_file: args += ["--description-file", description_file]
    if sprint: args += ["--sprint", sprint]
    if backlog: args.append("--backlog")
    if type_: args += ["--type", type_]
    if priority: args += ["--priority", priority]
    if release: args += ["--release", release]
    if review: args += ["--review", review]
    if add_file:
        for f in add_file: args += ["--add-file", f]
    if remove_file:
        for f in remove_file: args += ["--remove-file", f]
    if add_dep:
        for d in add_dep: args += ["--add-dep", d]
    if remove_dep:
        for d in remove_dep: args += ["--remove-dep", d]
    if add_label:
        for l in add_label: args += ["--add-label", l]
    if remove_label:
        for l in remove_label: args += ["--remove-label", l]
    if add_component:
        for c in add_component: args += ["--add-component", c]
    if remove_component:
        for c in remove_component: args += ["--remove-component", c]
    if add_related:
        for r in add_related: args += ["--add-related", r]
    if remove_related:
        for r in remove_related: args += ["--remove-related", r]
    if add_requirement:
        for r in add_requirement: args += ["--add-requirement", r]
    if remove_requirement:
        for r in remove_requirement: args += ["--remove-requirement", r]
    if add_use_case:
        for u in add_use_case: args += ["--add-use-case", u]
    if remove_use_case:
        for u in remove_use_case: args += ["--remove-use-case", u]
    if components_auto: args += ["--components", "auto"]
    if with_user: args += ["--with", with_user]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("details")
def task_details(
    ctx: typer.Context,
    task_ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
) -> None:
    """Tasks in full (description, dependencies, files, notes)."""
    args = ["show"] + task_ids
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@task_app.command("note")
def task_note(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    text: Annotated[str, typer.Argument(help="The note text")],
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
) -> None:
    """Add a note to a task (at most 500 characters)."""
    args = ["note", task_id, text]
    if json_out: args.append("--json")
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
    next_state: Annotated[str, typer.Argument(help="Next state: todo, in_progress, in_review, done, blocked, skipped")],
    note: Optional[str] = typer.Option(None, "--note", help="A note"),
    reason: Optional[str] = typer.Option(None, "--reason", help="A reason"),
    commit: Optional[str] = typer.Option(None, "--commit", help="The task's commit SHA"),
    resolution: Optional[str] = typer.Option(None, "--resolution", help="How it ended"),
    duplicate_of: Optional[str] = typer.Option(None, "--duplicate-of", help="The task it duplicates"),
    on: Optional[str] = typer.Option(None, "--on", help="Who has to decide (user|agent)"),
    question: Optional[list[str]] = typer.Option(None, "--question", help="An open question (repeatable)"),
    answer: Optional[list[str]] = typer.Option(None, "--answer", help="An answer (repeatable)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who runs it"),
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
) -> None:
    """Change a task's state (start, review, done, fail, release, reject, approve, request-changes, block, unblock, reopen, skip)."""
    # Map next-state to the corresponding `atm` subcommand
    cmd_map = {
        "in_progress": "start",
        "in_review": "review",
        "done": "done",
        "todo": "fail",        # default for active->todo; may be overridden below
        "blocked": "block",
        "skipped": "skip",
        # review sub-states (stay in review):
        "approved": "approve",
        "changes_requested": "request-changes",
    }
    cmd = cmd_map.get(next_state)
    if cmd is None:
        typer.echo(f"atm2: unknown state '{next_state}'. Use: {', '.join(sorted(cmd_map))}", err=True)
        raise typer.Exit(2)

    # `todo` from review means reject; `todo` from blocked means unblock; `todo` from done/skipped means reopen
    # We can't know the current state without asking `atm show`, so default `todo` -> reject (from review).
    # The user can override with --on/--answer for unblock, or use the specific atm command directly.
    if next_state == "todo":
        cmd = "reject"  # safest default: task in review back to todo

    args = [cmd, task_id]
    if note: args += ["--note", note]
    if reason: args += ["--reason", reason]
    if commit: args += ["--commit", commit]
    if resolution: args += ["--resolution", resolution]
    if duplicate_of: args += ["--duplicate-of", duplicate_of]
    if on: args += ["--on", on]
    if question:
        for q in question: args += ["--question", q]
    if answer:
        for a in answer: args += ["--answer", a]
    if status: args += ["--status", status]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
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
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """The task /atm:loop-tasks works on next (exit code 3 if there is none)."""
    args = ["next"]
    if sprint:
        for s in sprint: args += ["--sprint", s]
    if tasks: args += ["--tasks", tasks]
    if all_tasks: args.append("--all")
    if backlog: args.append("--backlog")
    if type_: args += ["--type", type_]
    if with_user: args.append("--with-user")
    if claim: args.append("--claim")
    if json_out: args.append("--json")
    if agent: args += ["--agent", agent]
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
    if every: args += ["--every", str(every)]
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
    json_out: bool = typer.Option(False, "--json", help="Print result as JSON"),
) -> None:
    """List live claims."""
    args = ["claims"]
    if json_out: args.append("--json")
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
    if task_id: args.append(task_id)
    if auto: args.append("--auto")
    if agent: args += ["--agent", agent]
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
    file: Optional[list[str]] = typer.Option(None, "--file", help="A file the task changed (repeatable)"),
    dir_: Optional[list[str]] = typer.Option(None, "--dir", help="Every change under a folder (repeatable)"),
    message: Optional[str] = typer.Option(None, "--message", help="The commit message"),
    record: Optional[str] = typer.Option(None, "--record", help="Record this existing commit instead"),
    agent: Optional[str] = typer.Option(None, "--agent"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Commit exactly the named files plus tasks.json, and record the commit."""
    args = ["commit", task_id]
    if file:
        for f in file: args += ["--file", f]
    if dir_:
        for d in dir_: args += ["--dir", d]
    if message: args += ["--message", message]
    if record: args += ["--record", record]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@vcs_app.command("pr")
def vcs_pr(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    number: int = typer.Option(..., "--number", help="The PR number"),
    url: str = typer.Option(..., "--url", help="The PR's URL"),
    agent: Optional[str] = typer.Option(None, "--agent"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Record the PR that contains the tasks."""
    args = ["pr"] + ids + ["--number", str(number), "--url", url]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@vcs_app.command("pushed")
def vcs_pushed(
    ctx: typer.Context,
    pr: int = typer.Option(..., "--pr", help="The PR number"),
    at: Optional[str] = typer.Option(None, "--at", help="When it was merged (ISO 8601 UTC)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Record that a PR was merged, on the tasks it contains."""
    args = ["merged", "--pr", str(pr)]
    if at: args += ["--at", at]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@vcs_app.command("repository")
def vcs_repository(
    ctx: typer.Context,
    url: Optional[str] = typer.Argument(None, help="An http(s) URL or git remote"),
    old: Optional[str] = typer.Option(None, "--from", help="Move recorded PR links from this address"),
    agent: Optional[str] = typer.Option(None, "--agent"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Print or set the repository's web address."""
    args = ["repository"]
    if url: args.append(url)
    if old: args += ["--from", old]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# db group
# ---------------------------------------------------------------------------

db_app = typer.Typer(help="Database / file maintenance")
app.add_typer(db_app, name="db")


@db_app.command("migrate")
def db_migrate(
    ctx: typer.Context,
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Move tasks.json into .agentic-task-manager/ and set up the folder."""
    args = ["migrate"]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("archive")
def db_archive(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    before: Optional[str] = typer.Option(None, "--before"),
    keep_days: Optional[float] = typer.Option(None, "--keep-days"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Move finished tasks to tasks-archive.json."""
    args = ["archive"]
    if sprint:
        for s in sprint: args += ["--sprint", s]
    if before: args += ["--before", before]
    if keep_days: args += ["--keep-days", str(keep_days)]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("unarchive")
def db_unarchive(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Bring archived tasks back into tasks.json."""
    args = ["unarchive"] + ids
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@db_app.command("init")
def db_init(
    ctx: typer.Context,
    meta_file: Optional[str] = typer.Option(None, "--meta-file", help="Set tasks.json meta from a JSON object"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Give tasks.json a meta.id and register the merge driver."""
    args = ["init"]
    if meta_file: args += ["--meta-file", meta_file]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


# ---------------------------------------------------------------------------
# project group
# ---------------------------------------------------------------------------

project_app = typer.Typer(help="Project-level config")
app.add_typer(project_app, name="project")


@project_app.command("config")
def project_config(
    ctx: typer.Context,
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Print where this project's task files are."""
    args = ["paths"]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("status")
def project_status(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    backlog: bool = typer.Option(False, "--backlog"),
    release: Optional[str] = typer.Option(None, "--release"),
    component: Optional[str] = typer.Option(None, "--component"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """The project overview: counts, what is in progress, in review or blocked."""
    args = ["status"]
    if sprint:
        for s in sprint: args += ["--sprint", s]
    if backlog: args.append("--backlog")
    if release: args += ["--release", release]
    if component: args += ["--component", component]
    if json_out: args.append("--json")
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
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Print the project's workflow, or set it."""
    args = ["workflow"]
    if action: args.append(action)
    if wf_type: args.append(wf_type)
    if base: args += ["--base", base]
    if verify: args += ["--verify", verify]
    if max_rounds: args += ["--max-rounds", str(max_rounds)]
    if commit_template: args += ["--commit-template", commit_template]
    if trailers: args += ["--trailers", trailers]
    if trailer:
        for t in trailer: args += ["--trailer", t]
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("logo")
def project_logo(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set or unset"),
    path: Optional[str] = typer.Argument(None, help="The image (set)"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Print, set or unset the project's logo."""
    args = ["logo"]
    if action: args.append(action)
    if path: args.append(path)
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@project_app.command("list")
def project_list(
    ctx: typer.Context,
    target: Optional[str] = typer.Argument(None, help="Add: project folder; Remove: ID or path"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """List the projects the dashboard shows."""
    args = ["projects", "list"]
    if target: args.append(target)
    if json_out: args.append("--json")
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
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """List sprints with task counts."""
    args = ["sprints", "list"]
    if all_: args.append("--all")
    if json_out: args.append("--json")
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
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Add a sprint."""
    args = ["sprints", "add"]
    if name: args.append(name)
    if goal: args += ["--goal", goal]
    if release: args += ["--release", release]
    if due: args += ["--due", due]
    if after: args += ["--after", after]
    if agent: args += ["--agent", agent]
    if json_out: args.append("--json")
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
    ctx: typer.Context,files: bool = typer.Option(False, "--files", help="Also warn about missing files")) -> None:
    """Validate tasks.json: schema, vocabularies, dependencies."""
    args = ["check"]
    if files: args.append("--files")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


@app.command("service")
def service(
    ctx: typer.Context,
    action: str = typer.Argument("start", help="start|stop|status"),
    open_: bool = typer.Option(False, "--open", help="Also open the dashboard in the browser"),
    all_projects: bool = typer.Option(False, "--all", help="Show all projects overview"),
    port: int = typer.Option(8765, "--port", help="The port to try first"),
    dir_: Optional[str] = typer.Option(None, "--dir", help="The project to show"),
    refresh: bool = typer.Option(False, "--refresh", help="Only restart if dashboard is older"),
    json_out: bool = typer.Option(False, "--json"),
) -> None:
    """Run the dashboard server in the background."""
    args = ["serve", action]
    if open_: args.append("--open")
    if all_projects: args.append("--all")
    if port != 8765: args += ["--port", str(port)]
    if dir_: args += ["--dir", dir_]
    if refresh: args.append("--refresh")
    if json_out: args.append("--json")
    sys.exit(run_atm(args, dry_run=ctx.obj.get("dry_run", False)))


if __name__ == "__main__":
    app()
