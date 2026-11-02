"""`atm workflow`, `atm commit`, `atm pr` and `atm merged`, through atm-lib (docs/layered-architecture.md §9, step 13)."""

import os

from ..boot.open import open_project
from ..claims import agent_identity
from ..common.errors import Invalid
from ..core import out, to_json
from ..lib.code import commit_task, record_commit, record_merge, record_pr, set_workflow, workflow_settings


def describe_workflow(workflow, base_default):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm workflow set direct|pull-request`."
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


def cmd_workflow(args):
    project = open_project(os.getcwd())
    if args.action is None:
        workflow = workflow_settings(project)
    else:
        workflow = set_workflow(
            project,
            args.type,
            base=args.base,
            verify=None if args.verify is None else args.verify == "true",
            max_rounds=args.max_rounds,
            commit_template=args.commit_template,
            trailers=args.trailers,
            trailer=args.trailer,
        )
    if args.json:
        out(to_json(workflow or {"type": None}))
    else:
        out(describe_workflow(workflow, project.vcs.default_base))


def cmd_commit(args):
    """Commit exactly the files the agent names plus tasks.json, then record the commit on the task."""
    project = open_project(os.getcwd(), agent_identity(args.agent))
    if args.record is not None:
        if args.file or args.message is not None:
            raise Invalid("--record records an existing commit: no --file or --message")
        result = record_commit(project, args.id, args.record)
        if args.json:
            shown = {k: result[k] for k in ("task", "commit", "recorded")}
            out(to_json({**shown, **({"hint": result["hint"]} if result["hint"] else {})}))
            return
        out(f"{result['task']}: recorded {result['commit'][:7]} {result['summary']}".rstrip())
        if result["hint"]:
            out(f"  {result['hint']}")
        return
    result = commit_task(project, args.id, args.file, os.getcwd(), args.message)
    if args.json:
        out(to_json(result))
    else:
        out(f"{result['task']}: committed {result['commit'][:7]} {result['message'].splitlines()[0]}")
        out("  " + ", ".join(result["files"]))


def cmd_pr(args):
    """Record the PR that contains the tasks (DM-4.3: the tasks are then In PR)."""
    ids = record_pr(open_project(os.getcwd(), agent_identity(args.agent)), args.ids, args.number, args.url)
    if args.json:
        out(to_json({"tasks": ids, "pr": {"number": args.number, "url": args.url}}))
    else:
        out(f"{', '.join(ids)}: in PR #{args.number} ({args.url})")


def cmd_merged(args):
    """Record that a PR was merged, on every task that records it (DM-4.3: the tasks are then Merged)."""
    result = record_merge(open_project(os.getcwd(), agent_identity(args.agent)), args.pr, args.at)
    if args.json:
        out(to_json(result))
        return
    ids = result["tasks"]
    out(f"PR #{args.pr} merged: {', '.join(ids)}" if ids else f"PR #{args.pr}: already recorded as merged")
    for release_id in result["released"]:
        out(f"Released {release_id}: all its tasks are merged in PR #{args.pr}.")
    for release_id, waiting in result["pending"].items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{args.pr}.")
