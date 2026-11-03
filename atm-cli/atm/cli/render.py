"""Text for people and agents: how the CLI shows atm-lib's results. Short and stable, for agents to read (CLI-3.3)."""

import sys


def report_expired(expired):
    """On stderr, the claims a command found expired (their tasks went back to todo)."""
    for claim in expired:
        sys.stderr.write(f"atm: lease of {claim['task']} by {claim.get('agent')} expired; it is available again\n")


def view_row(view):
    """The fields `atm list` shows of a task, from atm-lib's view of it (task_views), as a dict for --json."""
    task, derived = view["task"], view["derived"]
    fields = {
        "id": task.get("id"),
        "status": task.get("status"),
        "kind": derived.get("kind"),
        "sprint": task.get("sprint"),
        "priority": derived.get("priority"),
        "labels": task.get("labels", []),
        "name": task.get("name", ""),
    }
    if isinstance(task.get("pr"), dict):
        fields["pr"] = task["pr"]
        fields["merged_at"] = task.get("merged_at")
    return fields


def list_lines(rows):
    """One line per task, as `atm list` prints them: ID, status, priority, labels, name (rows as view_row gives
    them, or atm-lib's `summary`)."""
    id_width = max(len(str(t.get("id"))) for t in rows)
    status_width = max(len(str(t.get("status"))) for t in rows)
    for t in rows:
        labels = t.get("labels") or []
        yield (
            f"{str(t.get('id')).ljust(id_width)}  {str(t.get('status')).ljust(status_width)}  "
            f"{t.get('priority')}  "
            + (f"[{', '.join(labels)}] " if labels else "")
            + str(t.get("name", ""))
        )


def render_task(view):
    """A task in full, as text (atm-lib's view of it: `show_tasks`, or a status change's `view`): what an agent needs
    to work on it. `atm start` prints the same text as `atm show`."""
    task = view["task"]
    labels = task.get("labels") or []
    lines = [
        f"{task.get('id')}  {task.get('name', '')}",
        f"  {task.get('status')} · {task.get('sprint') or 'backlog'}"
        # A task from the archive says so (found in T-277, T-353)
        + (" · archived" if view["archived"] else "")
        + f" · {task.get('type')} · {view['priority']}"
        + (f" · labels: {', '.join(labels)}" if labels else ""),
    ]
    if view["dependencies"]:
        lines.append(
            "  depends on: "
            + ", ".join(
                f"{d['id']} (missing)"
                if d["status"] is None
                else f"{d['id']} ({d['status']}" + (", archived" if d["archived"] else "") + ")"
                for d in view["dependencies"]
            )
        )
    # How it ended, when it isn't the default (done, or won't do for a skipped task), and its relations (SR-2.8)
    if task.get("resolution"):
        lines.append(
            f"  resolution: {task['resolution']}" + (f" (of {task['duplicate_of']})" if task.get("duplicate_of") else "")
        )
    if view["related"]:
        lines.append("  related: " + ", ".join(view["related"]))
    if view["duplicated_by"]:
        lines.append("  duplicated by: " + ", ".join(view["duplicated_by"]))
    for key in ("requirements", "use_cases", "files"):
        if task.get(key):
            lines.append(f"  {key.replace('_', ' ')}: {', '.join(task[key])}")
    who = []
    for what, at, by in (
        ("created", "created_at", task.get("created_by")),
        ("started", "started_at", view["started_by"]),
        ("review requested", "review_requested_at", None),
        ("completed", "completed_at", task.get("completed_by")),
    ):
        if task.get(at):
            who.append(f"{what} {task[at]}" + (f" by {by}" if by else ""))
    if who:
        lines.append("  " + " · ".join(who))
    if task.get("commit"):
        lines.append(f"  commit: {task['commit']}")
    description = (task.get("description") or "").strip()
    if description:
        lines.append("  description:")
        lines.extend("    " + line for line in description.splitlines())
    notes = task.get("notes") or []
    if notes:
        lines.append("  notes:")
        for n in notes:
            lines.append(f"    {n.get('at')} {n.get('kind')}" + (f" ({n['by']})" if n.get("by") else "") + f": {n.get('text')}")
    elif task.get("last_failure"):
        failure = task["last_failure"]
        lines.append(f"  last failure: {failure.get('at')}: {failure.get('reason')}")
    return "\n".join(lines)
