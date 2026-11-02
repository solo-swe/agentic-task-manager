"""The read commands, through atm-lib (docs/layered-architecture.md §9, step 3): list, deps, status, summary.

Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3)."""

import os

from ..boot.open import open_project
from ..lib.archive import archive_hint_now
from ..common.core import out, to_json
from ..lib.query import (
    Filters,
    list_matching,
    project_summary,
    read_problems,
    status_report,
    summary,
    task_dependencies,
    task_views,
)
from .render import list_lines


def cmd_list(args):
    if args.derived:
        # JSON either way: it is for tools (the dashboard's tests take atm's own values from it)
        if args.tolerant:
            project = open_project(os.getcwd(), tolerant=True)
            views = task_views(project, args)
            meta = project.store.document().get("meta") or {}  # without what doesn't work, as atm serve sends it
            read = {"meta": meta, "tasks": views, "problems": read_problems(project)}
            hint = archive_hint_now(project) if not args.archived else None
            if hint:
                read["archive_hint"] = hint
            out(to_json(read))
        else:
            out(to_json(task_views(open_project(os.getcwd()), args)))
        return
    found = list_matching(open_project(os.getcwd()), args)
    vocab, tasks = found["vocab"], found["tasks"]
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
        return
    if not tasks:
        out("No tasks match.")
        return
    for line in list_lines(vocab, tasks):
        out(line)


def cmd_deps(args):
    deps = task_dependencies(open_project(os.getcwd()), args.id)
    if args.json:
        out(to_json(deps))
        return
    line = lambda items: ", ".join(f"{i['id']} ({i['status']})" for i in items) or "nothing"
    out(f"{args.id} waits for: {line(deps['waits_for'])}")
    out(f"{args.id} is waited on by: {line(deps['waited_on_by'])}")


def cmd_summary(args):
    """The project's summary as atm serve lists it in api/projects (3.2); the dashboard's dev server plays atm serve
    with it, so its numbers and Next up come from atm-cli too. JSON either way: it is for tools."""
    out(to_json(project_summary(open_project(os.getcwd()))))


def cmd_status(args):
    status = status_report(
        open_project(os.getcwd()),
        Filters(sprint=args.sprint, backlog=args.backlog, release=args.release, component=args.component),
    )
    result = status["report"]
    if args.json:
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
            + " (atm move <id> --to backlog or --to active)"
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
