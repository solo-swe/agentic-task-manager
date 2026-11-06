"""How `atm project status` prints atm-lib's status report (docs/layered-architecture.md §9, step 3).

Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3)."""

from ..common.core import out, to_json


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
