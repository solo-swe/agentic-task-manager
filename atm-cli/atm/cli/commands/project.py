"""`atm project`: the project (config, status, workflow, logo) and the dashboard's list of projects (list, add,
remove, prune)."""

import os
from typing import Annotated, Optional

import typer

from ...boot.jsongit import task_paths
from ...boot.open import open_project, open_registry
from ...common.core import out, to_json
from ...common.errors import Invalid
from ...lib.code import get_workflow, set_workflow
from ...lib.projects import add_project, list_projects, prune_projects, remove_project
from ...lib.query import Filters, project_summary, status_report
from ...lib.settings import get_logo, set_logo, unset_logo
from ..app import _choice, _dry, app
from ..store import show_paths


# How `atm project status` prints atm-lib's status report (docs/layered-architecture.md §9, step 3).
#
# Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3).


def show_status(status, as_json):
    """Print atm-lib's status report (`atm project status`)."""
    result = status["report"]
    if as_json:
        out(to_json(result))
        return
    sprint, stranded = result["sprint"], result.get("open_in_closed_sprints")
    pairs = lambda d: ", ".join(f"{k} {v}" for k, v in d.items())
    elsewhere = lambda i: f" [in another worktree, {i['branch'] or 'detached'}]" if "branch" in i else ""
    if sprint:
        details = [f"{sprint['done']} of {sprint['tasks']} done"] + [
            f"{k} {sprint[k]}" for k in ("due", "release") if sprint.get(k)
        ]
        out(f"sprint: {sprint['id']} (active): {' · '.join(details)}")
        if sprint.get("goal"):
            out(f"  goal: {sprint['goal']}")
        if sprint["review_pending"]:
            out(f"  waiting for the sprint review: {', '.join(sprint['review_pending'])} (/atm:close-sprint)")
    elif status["has_sprints"]:
        out("sprint: none active (plan one with /atm:plan-sprint)")
    if stranded:
        n = len(stranded)
        out(
            f"warning: {n} open task{'' if n == 1 else 's'} in closed sprints, which the loop won't take: "
            + ", ".join(f"{t['id']} ({t['sprint']})" for t in stranded)
            + " (atm sprint move <id> --to backlog or --to active)"
        )
    out(f"{result['tasks']} tasks: {pairs(result['by_status'])}")
    if status["has_sprints"]:
        out(f"by sprint: {pairs(result['by_sprint'])}")
    out(f"by type: {pairs(result['by_type'])}")
    if result["by_label"]:
        out(f"by label: {pairs(result['by_label'])}")
    for title, items, extra in (
        ("in progress", result["in_progress"], lambda i: f" ({i['by']})" if i["by"] else ""),
        ("in review", result["in_review"], lambda i: (f" (since {i['since']})" if i["since"] else "") + elsewhere(i)),
        ("blocked", result["blocked"], lambda i: (f" (on {i['waiting_on']})" if i["waiting_on"] else "") + elsewhere(i)),
        ("waiting on dependencies", result["waiting_on_dependencies"], lambda i: f" (on {', '.join(i['on'])})"),
    ):
        if items:
            out(f"{title}: " + "; ".join(f"{i['id']} {i['name']}{extra(i)}" for i in items))
    nxt = result["next"]
    out(f"next: {nxt['id']} {nxt['name']}" if nxt else f"next: none ({result['next_reason']})")
    largest = ", ".join(f"{i['id']} {i['bytes'] // 1024 or 1} KB" for i in result["largest_tasks"])
    out(f"file: {result['file_bytes'] // 1024} KB" + (f"; largest tasks: {largest}" if largest else ""))
    if result["archived"]:
        out(f"archive: {result['archived']} tasks, {result['archive_bytes'] // 1024} KB")
    hint = (result.get("suggest") or {}).get("archive")
    if hint:
        out(hint["text"])


# How `atm project workflow` describes the project's workflow (docs/layered-architecture.md §9, step 13).


def describe_workflow(workflow, base_default):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm project workflow set direct|pull-request`."
    trailers = workflow.get("trailers")
    lines = [
        f"workflow: {workflow['type']}",
        "  base: " + (workflow["base"] if workflow.get("base") else f"{base_default()} (the remote's default branch)"),
        f"  verify before pushing: {'yes' if workflow['verify'] else 'no'}",
        f"  CI fix rounds: {workflow['max_rounds']}",
        f"  commit message: {workflow['commit_template']}",
        "  trailers: "
        + (
            "none (Co-Authored-By lines are removed)"
            if trailers == "forbid"
            else ("required: " + "; ".join(trailers["require"]) if isinstance(trailers, dict) else "as the agent writes them")
        ),
    ]
    return "\n".join(lines)


# `atm project logo`: the project's logo, through atm-lib. Setting the store itself up (`atm db init`: the merge
# driver, the dashboard's list) is boot's, the one place that knows the store.


def logo_action(project, action, path, as_json):
    """Print, set or unset the logo (`atm project logo`)."""
    if action is None:
        if path is not None:
            raise Invalid(f"unexpected argument '{path}' (atm project logo set PATH)")
        info = get_logo(project)
        logo, chosen = info["found"], info["set"]
        if as_json:
            found = {"logo": logo["path"], "source": logo["source"]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": chosen}))
        elif logo:
            out(logo["path"] + (" (found; set one with `atm project logo set <path>`)" if logo["source"] == "found" else ""))
        elif chosen:
            out(f"meta.logo is {chosen}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm project logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if action == "unset":
        removed = unset_logo(project)
        out(to_json({"logo": None}) if as_json else "Logo unset." if removed else "No logo was set.")
        return
    if path is None:
        raise Invalid("atm project logo set needs a path")
    # The path is relative to where atm runs, like every path atm takes; atm-lib keeps it relative to the project
    relative = set_logo(project, path)
    out(to_json({"logo": relative}) if as_json else f"Logo set: {relative}")


# `atm project list|add|remove|prune`: the projects the dashboard shows, through atm-lib.


def describe_entry(entry):
    return f"{entry['id']}  {entry['name']}  {entry['path']}"


def damaged_notice(damaged):
    return f"projects.json was unreadable: moved it to {damaged} and started an empty registry." if damaged else None


def projects_action(registry, action, target, as_json):
    """List, add, remove or prune the dashboard's projects (`atm project list|add|remove|prune`)."""
    if action == "add":
        result = add_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        out(f"{'Added' if result['result'] == 'added' else 'Already registered'}: {describe_entry(result['project'])}")
    elif action == "remove":
        result = remove_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        out(f"Removed: {describe_entry(result['removed'])}")
    elif action == "prune":
        result = prune_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["removed"]:
            out("Nothing to prune: every registered project is there.")
        for e in result["removed"]:
            out(f"Removed: {describe_entry(e)}")
    else:
        if target:
            raise Invalid(f"unexpected argument '{target}'")
        result = list_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["projects"]:
            out("No projects registered. `atm project add [PATH]` adds one; atm adds a project when it runs there.")
        for e in result["projects"]:
            out(describe_entry(e) + ("  (not found)" if e["missing"] else ""))


project_app = typer.Typer(help="Project-level config")
app.add_typer(project_app, name="project")


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
    """Print the project's workflow, or set it (atm project workflow set TYPE)."""
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
def project_logo_cmd(
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
