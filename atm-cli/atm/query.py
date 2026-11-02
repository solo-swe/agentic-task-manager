"""Reading tasks: show, list, status, deps. Agents read tasks through these, never tasks.json itself (DM-1.1).

Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3). These commands
never write: they read tasks.json and the claims store without the lock (both are replaced atomically).
"""

import json
import os
import re

from .claims import Selection, loop_order, next_task, read_store
from .core import AtmError, now, out, parse_iso, to_json
from .model import (
    REWORK_STATES,
    archive_hint,
    archive_path,
    archived_task,
    effective_release,
    find_sprint,
    find_task,
    kind_of,
    known_release,
    load_archive,
    load_tasks,
    natural_key,
    open_in_closed_sprints,
    priority_rank,
    unfinished_dependencies,
    vocabulary,
)
from .paths import find_paths
from .registry import head_branch


def priority_of(vocab, task):
    """The task's priority, or the default for a missing or unknown one."""
    ids = [p["id"] for p in vocab["priorities"]]
    return task.get("priority") if task.get("priority") in ids else vocab["default_priority"]


def started_by(task):
    """started_by, or `agent` in files written before v4."""
    return task.get("started_by") or task.get("agent")


def summary(vocab, task):
    """The fields `list` shows, as a dict for --json."""
    fields = {
        "id": task.get("id"),
        "status": task.get("status"),
        "kind": kind_of(vocab, task.get("status")),
        "sprint": task.get("sprint"),
        "priority": priority_of(vocab, task),
        "labels": task.get("labels", []),
        "name": task.get("name", ""),
    }
    if isinstance(task.get("pr"), dict):
        fields["pr"] = task["pr"]
        fields["merged_at"] = task.get("merged_at")
    return fields


def scope_filter(vocab, args):
    """A test for the tasks `--sprint` (repeatable), `--backlog`, `--release` and `--component` keep (SR-3.2); a
    task's release is its effective one (its own, else its sprint's)."""
    names = []
    for name in getattr(args, "sprint", None) or []:
        sprint = find_sprint(vocab, name)
        if sprint is None:
            raise AtmError(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in vocab['sprints']) or 'none'}", 2)
        names.append(sprint["id"])
    if getattr(args, "release", None):
        known_release(vocab, args.release)

    def keep(task):
        sprint = (find_sprint(vocab, task.get("sprint")) or {}).get("id")
        if names and sprint not in names:
            return False
        if getattr(args, "backlog", False) and sprint:
            return False
        if getattr(args, "release", None) and effective_release(vocab, task) != args.release:
            return False
        return not getattr(args, "component", None) or args.component in (task.get("components") or [])

    return keep


def in_open_pr(task):
    """In PR: a recorded PR, not merged yet (DM-4.3)."""
    return isinstance(task.get("pr"), dict) and not task.get("merged_at")


def ready_tasks(data):
    """To-do tasks whose dependencies are all done, in the loop's order."""
    vocab = vocabulary(data)
    ready = [
        t
        for t in data["tasks"]
        if kind_of(vocab, t.get("status")) == "todo" and not unfinished_dependencies(data, t)
    ]
    return sorted(ready, key=lambda t: (priority_rank(vocab, t.get("priority")), natural_key(t.get("id"))))


def describe_task(data, task):
    """A task in full, as text: what an agent needs to work on it."""
    vocab = vocabulary(data)
    by_id = {t.get("id"): t for t in data["tasks"]}
    for d in task.get("dependencies") or []:
        if d not in by_id and archived_task(d):
            by_id[d] = {**archived_task(d), "status": archived_task(d).get("status") + ", archived"}
    labels = task.get("labels") or []
    # A task not in tasks.json is from the archive: say so (found in T-277, T-353)
    archived = all(t is not task for t in data["tasks"])
    lines = [
        f"{task.get('id')}  {task.get('name', '')}",
        f"  {task.get('status')} · {task.get('sprint') or 'backlog'}"
        + (" · archived" if archived else "")
        + f" · {task.get('type')} · {priority_of(vocab, task)}"
        + (f" · labels: {', '.join(labels)}" if labels else ""),
    ]
    deps = task.get("dependencies") or []
    if deps:
        lines.append(
            "  depends on: " + ", ".join(f"{d} ({by_id[d].get('status')})" if d in by_id else f"{d} (missing)" for d in deps)
        )
    # How it ended, when it isn't the default (done, or won't do for a skipped task), and its relations (SR-2.8):
    # `related` is stored on one side and shown on both
    if task.get("resolution"):
        lines.append(
            f"  resolution: {task['resolution']}" + (f" (of {task['duplicate_of']})" if task.get("duplicate_of") else "")
        )
    own = [r for r in task.get("related") or [] if isinstance(r, str)]
    others = [
        t.get("id")
        for t in data["tasks"]
        if t is not task and task.get("id") in (t.get("related") or []) and t.get("id") not in own
    ]
    if own or others:
        lines.append("  related: " + ", ".join(own + others))
    duplicates = [t.get("id") for t in data["tasks"] if t.get("duplicate_of") == task.get("id")]
    if duplicates:
        lines.append("  duplicated by: " + ", ".join(duplicates))
    for key in ("requirements", "use_cases", "files"):
        if task.get(key):
            lines.append(f"  {key.replace('_', ' ')}: {', '.join(task[key])}")
    who = []
    for what, at, by in (
        ("created", "created_at", task.get("created_by")),
        ("started", "started_at", started_by(task)),
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


def cmd_show(args):
    paths = find_paths(os.getcwd())
    data = load_tasks(paths)
    tasks = [find_task(data, task_id, archived=True) for task_id in args.ids]
    if args.json:
        # An archived task says so (T-353)
        live = {id(t) for t in data["tasks"]}
        out(to_json([t if id(t) in live else {**t, "archived": True} for t in tasks]))
    else:
        out("\n\n".join(describe_task(data, t) for t in tasks))


def cmd_list(args):
    paths = find_paths(os.getcwd())
    data = load_tasks(paths)
    vocab = vocabulary(data)
    ready = {t.get("id") for t in ready_tasks(data)}
    in_scope = scope_filter(vocab, args)
    tasks = []
    for t in load_archive(paths)["tasks"] if args.archived else data["tasks"]:
        kind = kind_of(vocab, t.get("status"))
        if not in_scope(t):
            continue
        if args.status and t.get("status") not in args.status:
            continue
        if args.type and t.get("type") != args.type:
            continue
        if args.priority and priority_of(vocab, t) != args.priority:
            continue
        if args.label and not set(args.label) <= set(t.get("labels") or []):
            continue
        if args.agent and started_by(t) != args.agent:
            continue
        if args.ready and t.get("id") not in ready:
            continue
        if args.blocked and not (kind == "blocked" or (kind == "todo" and t.get("id") not in ready)):
            continue
        if args.in_pr and not in_open_pr(t):
            continue
        if args.with_user and t.get("with") != "user":
            continue
        tasks.append(t)
    if args.json:
        out(to_json([summary(vocab, t) for t in tasks]))
        return
    if not tasks:
        out("No tasks match.")
        return
    for line in list_lines(vocab, tasks):
        out(line)


def list_lines(vocab, tasks):
    """One line per task, as `atm list` prints them: ID, status, priority, labels, name."""
    id_width = max(len(str(t.get("id"))) for t in tasks)
    status_width = max(len(str(t.get("status"))) for t in tasks)
    for t in tasks:
        labels = t.get("labels") or []
        yield (
            f"{str(t.get('id')).ljust(id_width)}  {str(t.get('status')).ljust(status_width)}  "
            f"{priority_of(vocab, t)}  "
            + (f"[{', '.join(labels)}] " if labels else "")
            + str(t.get("name", ""))
        )


def cmd_deps(args):
    paths = find_paths(os.getcwd())
    data = load_tasks(paths)
    task = find_task(data, args.id, archived=True)
    by_id = {t.get("id"): t for t in data["tasks"]}

    def dependency(d):
        found = by_id.get(d) or archived_task(d)
        if not found:
            return {"id": d, "status": "missing", "name": ""}
        return {"id": d, "status": found.get("status") + (", archived" if d not in by_id else ""), "name": found.get("name", "")}

    waits_for = [dependency(d) for d in task.get("dependencies") or []]
    waited_on_by = [
        {"id": t.get("id"), "status": t.get("status"), "name": t.get("name", "")}
        for t in data["tasks"]
        if args.id in (t.get("dependencies") or [])
    ]
    # Archived tasks that depended on it are history too, marked as the waits-for side marks them (T-358); a task in
    # both files is the tasks.json one
    waited_on_by += [
        {"id": t.get("id"), "status": f"{t.get('status')}, archived", "name": t.get("name", "")}
        for t in load_archive(paths)["tasks"]
        if isinstance(t, dict)
        and t.get("id") not in by_id
        and isinstance(t.get("dependencies"), list)
        and args.id in t["dependencies"]
    ]
    if args.json:
        out(to_json({"id": args.id, "waits_for": waits_for, "waited_on_by": waited_on_by}))
        return
    line = lambda items: ", ".join(f"{i['id']} ({i['status']})" for i in items) or "nothing"
    out(f"{args.id} waits for: {line(waits_for)}")
    out(f"{args.id} is waited on by: {line(waited_on_by)}")


ACTIVITY_FIELDS = ("updated_at", "started_at", "completed_at", "review_requested_at", "blocked_at", "skipped_at")
SUMMARY_DAYS = 14


def parse_time(text):
    try:
        return parse_iso(text) if isinstance(text, str) else None
    except ValueError:
        return None


RECENT_SPRINTS = 4
NEXT_TASKS = 2
NAME_CAP = 80  # task names in the summary, so /api/projects stays small with many projects


def capped(name):
    name = name if isinstance(name, str) else ""
    return name if len(name) <= NAME_CAP else name[: NAME_CAP - 1] + "…"


def sprint_numbers(sprint, own_kinds):
    """A sprint as the overview shows it: its name, state, due date and release, and done of its tasks but the skipped."""
    done, total = own_kinds.count("done"), len(own_kinds) - own_kinds.count("skipped")
    return {
        "id": sprint["id"],
        "state": sprint["state"],
        "due": sprint.get("due"),
        "release": sprint.get("release"),
        "done": done,
        "total": total,
        "percent": round(100 * done / total) if total else 100,
    }


def recent_sprints(vocab, tasks, kinds):
    """Up to four sprints with tasks for the overview's rings (SR-7.7), in the project's order: the active one and the
    planned ones first, then the most recently closed (by their end) to fill up. None for a project without sprints."""
    if not vocab["sprints"]:
        return None
    own = {s["id"]: [] for s in vocab["sprints"]}
    for t, k in zip(tasks, kinds):
        sprint = find_sprint(vocab, t.get("sprint"))
        if sprint:
            own[sprint["id"]].append(k)
    with_tasks = [s for s in vocab["sprints"] if own[s["id"]]]
    chosen = [s["id"] for s in with_tasks if s["state"] != "closed"][:RECENT_SPRINTS]
    closed = sorted((s for s in with_tasks if s["state"] == "closed"), key=lambda s: s.get("end") or "", reverse=True)
    chosen += [s["id"] for s in closed[: RECENT_SPRINTS - len(chosen)]]
    return [sprint_numbers(s, own[s["id"]]) for s in with_tasks if s["id"] in chosen]


def active_sprint_numbers(vocab, tasks, kinds):
    """The active sprint for the overview's cards (SR-7.7), with its progress; None when none is active."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    own = [k for t, k in zip(tasks, kinds) if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
    return {**sprint_numbers(active, own), "goal": active.get("goal")}


DESCRIPTION_CAP = 200


def description(meta):
    """meta.description (an optional line about the project, set with atm init --meta-file), capped at 200 characters
    for the overview, where it is the project name's tooltip (T-209); None without one."""
    text = meta.get("description") if isinstance(meta, dict) else None
    if not isinstance(text, str) or not text.strip():
        return None
    text = text.strip()
    return text if len(text) <= DESCRIPTION_CAP else text[: DESCRIPTION_CAP - 1] + "…"


def git_summary(paths, tasks):
    """The project's git line for its card (T-206): the main worktree's branch (read from .git/HEAD, no git process),
    and the latest PR recorded on its tasks (`atm pr`; the highest number), `merged` once its tasks have merged_at.
    No forge call and no CI state."""
    git_dir = os.path.join(paths["root"], ".git")
    branch = head_branch(git_dir) if os.path.isdir(git_dir) else None
    prs = {}
    for t in tasks:
        pr = t.get("pr")
        if isinstance(pr, dict) and isinstance(pr.get("number"), int):
            entry = prs.setdefault(pr["number"], {"number": pr["number"], "url": None, "state": "open"})
            # The dashboard links to it: only a web address (docs/security-review.md F9)
            if isinstance(pr.get("url"), str) and re.match(r"https?://\S+\Z", pr["url"], re.IGNORECASE):
                entry["url"] = pr["url"]
            if t.get("merged_at"):
                entry["state"] = "merged"
    return {"branch": branch, "pr": prs[max(prs)] if prs else None}


BOARD_CAP = 30  # cards per column of the All projects board (docs/all-projects-kanban-proposal.md, 3.1)
BOARD_READY_CAP = 10  # to-do tasks per project, so one project's new batch doesn't fill the To Do column
BOARD_DONE_HOURS = 24


def board_card(vocab, task, **extra):
    """What a project's Kanban card shows, nothing more (T-257)."""
    labels = task.get("labels")
    card = {
        "id": task.get("id"),
        "name": capped(task.get("name")),
        "type": task.get("type"),
        "sprint": task.get("sprint") if isinstance(task.get("sprint"), str) else None,
        "priority": priority_of(vocab, task),
        "labels": [l for l in labels if isinstance(l, str)] if isinstance(labels, list) else [],
    }
    card.update(extra)
    return card


def board_column(cards, cap=BOARD_CAP):
    """A column: its first `cap` cards, and how many there are in all (for "+ N more")."""
    return {"count": len(cards), "cards": cards[:cap]}


def todo_order(data, tasks, kinds, loop_ready, vocab):
    """Every to-do task, for the board's To Do column (T-349): first the ready ones, those the loop takes in its order
    before the rest (other sprints', the backlog's), then the ready ones done with the user, then those waiting on
    dependencies (each by priority, then ID). Each with why it isn't plain ready: done with the user, or the
    dependencies it waits for."""
    rank = {t.get("id"): i for i, t in enumerate(loop_ready)}

    def entry(t):
        waits = [dep for dep, _ in unfinished_dependencies(data, t)]
        key = (
            bool(waits),
            t.get("with") == "user",
            t.get("id") not in rank,
            rank.get(t.get("id"), 0),
            priority_rank(vocab, t.get("priority")),
            natural_key(t.get("id")),
        )
        return key, t, waits

    entries = sorted((entry(t) for t, k in zip(tasks, kinds) if k == "todo"), key=lambda e: e[0])
    return [(t, waits) for _, t, waits in entries]


def board(data, store, tasks, kinds, todo, vocab, current, live, recent_done=True):
    """The project's cards for the All projects board (T-257), one rule per column: every to-do task (`todo`, as
    todo_order gives it, T-349); everything in progress (longest running first), in review (longest waiting first)
    and blocked (on the user first, then longest waiting); what was finished, newest first: in the last 24 hours, or
    all of it without `recent_done` (a sprint's board shows all its tasks, T-349)."""
    of_kind = lambda kind: [t for t, k in zip(tasks, kinds) if k == kind]
    claimed_by = {c.get("task"): c.get("agent") for c in live}
    since = lambda t, field: t.get(field) or t.get("updated_at")
    active = sorted(of_kind("active"), key=lambda t: str(t.get("started_at") or ""))
    review = sorted(of_kind("review"), key=lambda t: str(since(t, "review_requested_at") or ""))
    blocked = sorted(
        of_kind("blocked"),
        key=lambda t: (t.get("waiting_on") != "user", str(since(t, "blocked_at") or "")),
    )
    cutoff = current.timestamp() - BOARD_DONE_HOURS * 3600
    finished_at = lambda t: parse_time(t.get("completed_at"))
    done = [
        t for t in of_kind("done") if not recent_done or (finished_at(t) and finished_at(t).timestamp() > cutoff)
    ]
    done.sort(key=lambda t: str(t.get("completed_at")), reverse=True)
    return {
        "todo": board_column(
            [
                board_card(
                    vocab,
                    t,
                    **({"with_user": True} if t.get("with") == "user" else {}),
                    **({"waiting_for": waits} if waits else {}),
                )
                for t, waits in todo
            ],
            BOARD_READY_CAP,
        ),
        "active": board_column(
            [
                board_card(vocab, t, by=claimed_by.get(t.get("id")) or started_by(t), since=since(t, "started_at"))
                for t in active
            ]
        ),
        "review": board_column([board_card(vocab, t, since=since(t, "review_requested_at")) for t in review]),
        "blocked": board_column(
            [
                board_card(
                    vocab,
                    t,
                    since=since(t, "blocked_at"),
                    waiting_on=t.get("waiting_on"),
                    questions=len(t.get("questions") or []),
                )
                for t in blocked
            ]
        ),
        "done": board_column([board_card(vocab, t, completed_at=t.get("completed_at")) for t in done]),
    }


def work_span(vocab, task, current):
    """A task's work as (start, end) timestamps, as the dashboard counts it (work.ts): from started_at until it asked
    for a review or was done, or until now while active; None for a task that holds none."""
    start = parse_time(task.get("started_at"))
    if not start:
        return None
    kind = kind_of(vocab, task.get("status"))
    if kind == "active":
        end = max(current, start)
    elif kind in ("done", "review"):
        end = (
            parse_time(task.get("review_requested_at"))
            or parse_time(task.get("completed_at") if kind == "done" else task.get("updated_at"))
            or start
        )
    else:
        return None
    return (start.timestamp(), max(end, start).timestamp())


def agent_minutes(vocab, tasks, current):
    """Agent time (docs/project-stats-proposal.md, T-255): the work of tasks an agent started, or nobody recorded
    (before v4), each agent's spans merged so an agent on two tasks counts once; parallel agents add up. As the
    dashboard's Project tab counts it (projectstats.ts)."""
    spans = {}
    for i, t in enumerate(tasks):
        starter = started_by(t)
        if starter and not str(starter).startswith("claude:"):
            continue
        span = work_span(vocab, t, current)
        if span:
            spans.setdefault(starter or i, []).append(span)  # unattributed spans are each their own
    total = 0.0
    for agent_spans in spans.values():
        end = None
        for s, e in sorted(agent_spans):
            if end is None or s > end:
                total += e - s
                end = e
            elif e > end:
                total += e - end
                end = e
    return round(total / 60)


def project_stats(paths, data, vocab, tasks, current):
    """The overview card's line (T-255): when the project started (its first task's created_at, else meta.created),
    how many tasks it has, and the agent time, archived tasks included; an unreadable archive is left out. (A task
    folder with a symlinked file never gets here: find_paths refuses it, docs/security-review.md F8.)"""
    try:
        archived = [t for t in load_archive(paths)["tasks"] if isinstance(t, dict)]
    except AtmError:
        archived = []
    ids = {t.get("id") for t in tasks}
    every = tasks + [t for t in archived if t.get("id") not in ids]
    created = [parse_time(t.get("created_at")) for t in every]
    created = [c for c in created if c] or [parse_time((data.get("meta") or {}).get("created"))]
    started = min((c for c in created if c), default=None)
    return {
        "started": started.strftime("%Y-%m-%dT%H:%M:%SZ") if started else None,
        "tasks": len(every),
        "agent_minutes": agent_minutes(vocab, every, current),
    }


def project_summary(paths):
    """A project's numbers for the dashboard's overview and project selector (docs/multi-project-api.md, 3.2), by
    status kind as atm counts them everywhere else: done / (total - skipped), the active and recent sprints, live
    claims, what needs the user, the latest activity and the completions of the last 14 days."""
    data = load_tasks(paths)
    vocab = vocabulary(data)
    tasks = data["tasks"]
    kinds = [kind_of(vocab, t.get("status")) for t in tasks]
    done, skipped = kinds.count("done"), kinds.count("skipped")
    counted = len(tasks) - skipped
    percent = round(100 * done / counted) if counted else (100 if skipped else 0)

    current = now()
    store = read_store(paths)
    live = sorted(
        (
            c
            for c in store["claims"].values()
            if isinstance(c, dict) and (parse_time(c.get("lease_expires_at")) or current) > current
        ),
        key=lambda c: str(c.get("claimed_at") or ""),
    )
    agents_working = len(live)
    by_id = {t.get("id"): t for t in tasks}
    # What the cards show by name (T-205): who works on what now, what the loop takes next, the last finished task
    working = [
        {
            "task": c.get("task"),
            "name": capped((by_id.get(c.get("task")) or {}).get("name")),
            "agent": c.get("agent"),
            "branch": c.get("branch"),
        }
        for c in live
    ]
    # What the loop takes next with its default selection (the active sprint's tasks in a project with sprints, SR-5.1),
    # by the very rule `atm next` follows: the dashboard's Next up shows the first (T-321)
    in_loop_order = loop_order(data, store, Selection())[0]
    next_up = [{"id": t.get("id"), "name": capped(t.get("name"))} for t in in_loop_order[:NEXT_TASKS]]
    # The board's To Do column: every to-do task, the loop's first (T-349)
    todo = todo_order(data, tasks, kinds, in_loop_order, vocab)
    finished = [(t.get("completed_at"), t) for t, k in zip(tasks, kinds) if k == "done" and parse_time(t.get("completed_at"))]
    last_task = max(finished, key=lambda f: f[0])[1] if finished else None
    last_done = (
        {"id": last_task.get("id"), "name": capped(last_task.get("name")), "completed_at": last_task.get("completed_at")}
        if last_task
        else None
    )

    times = [parse_time(t.get(f)) for t in tasks for f in ACTIVITY_FIELDS]
    times += [parse_time(n.get("at")) for t in tasks for n in t.get("notes") or [] if isinstance(n, dict)]
    times = [t for t in times if t]
    last = max(times) if times else None

    completions = [0] * SUMMARY_DAYS
    today = current.date()
    for t, k in zip(tasks, kinds):
        finished = parse_time(t.get("completed_at")) if k == "done" else None
        if finished:
            age = (today - finished.date()).days
            if 0 <= age < SUMMARY_DAYS:
                completions[SUMMARY_DAYS - 1 - age] += 1

    # The tasks that need the user, as the dashboard's needsYouReason decides it: in review and waiting for the
    # reviewer, blocked on the user, or to do together with the user (SR-6.6) now: in the active sprint (any, without
    # one) and ready, its dependencies done (T-347)
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)

    def together_now(t):
        in_scope = active is None or (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
        return in_scope and not unfinished_dependencies(data, t)

    def needs_you(t, k):
        if k == "review":
            return None if t.get("review_state") in REWORK_STATES else "review"
        if k == "blocked" and t.get("waiting_on") == "user":
            return "blocked"
        return "together" if k == "todo" and t.get("with") == "user" and together_now(t) else None

    needs_you_tasks = [
        {"id": t.get("id"), "name": capped(t.get("name")), "reason": needs_you(t, k)}
        for t, k in zip(tasks, kinds)
        if needs_you(t, k)
    ]

    return {
        "total": len(tasks),
        "done": done,
        "skipped": skipped,
        "percent": percent,
        "active_sprint": active_sprint_numbers(vocab, tasks, kinds),
        "recent_sprints": recent_sprints(vocab, tasks, kinds),
        "agents_working": agents_working,
        "working": working,
        "next": next_up,
        "last_done": last_done,
        "needs_you": {
            "review": sum(1 for item in needs_you_tasks if item["reason"] == "review"),
            "blocked_on_user": sum(1 for item in needs_you_tasks if item["reason"] == "blocked"),
            "together": sum(1 for item in needs_you_tasks if item["reason"] == "together"),
        },
        "needs_you_tasks": needs_you_tasks,
        "git": git_summary(paths, tasks),
        "description": description(data.get("meta")),
        "last_activity": last.strftime("%Y-%m-%dT%H:%M:%SZ") if last else None,
        "completions_14d": completions,
        "board": board(data, store, tasks, kinds, todo, vocab, current, live),
        "board_active_sprint": active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live),
        "stats": project_stats(paths, data, vocab, tasks, current),
    }


def active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live):
    """The All projects board's default view (T-340): the same columns, of the active sprint's tasks only, no more, no
    less (T-349: every one done too, not the last 24 hours'), with their own counts; None without an active sprint (the
    board says so)."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    in_sprint = lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
    pairs = [(t, k) for t, k in zip(tasks, kinds) if in_sprint(t)]
    return board(
        data,
        store,
        [t for t, _ in pairs],
        [k for _, k in pairs],
        [(t, waits) for t, waits in todo if in_sprint(t)],
        vocab,
        current,
        live,
        recent_done=False,
    )


def cmd_summary(args):
    """The project's summary as atm serve lists it in api/projects (3.2); the dashboard's dev server plays atm serve
    with it, so its numbers and Next up come from atm-cli too. JSON either way: it is for tools."""
    out(to_json(project_summary(find_paths(os.getcwd()))))


def cmd_status(args):
    paths = find_paths(os.getcwd())
    data = load_tasks(paths)
    vocab = vocabulary(data)
    store = read_store(paths)
    in_scope = scope_filter(vocab, args)
    tasks = [t for t in data["tasks"] if in_scope(t)]
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    sprint = None
    if active:
        in_active = [t for t in data["tasks"] if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
        sprint = {
            **{k: active[k] for k in ("id", "goal", "due", "release", "start") if active.get(k)},
            "done": sum(1 for t in in_active if kind_of(vocab, t.get("status")) == "done"),
            "tasks": sum(1 for t in in_active if kind_of(vocab, t.get("status")) != "skipped"),
            # Batch tasks waiting for the end-of-sprint review: they wait on the user (SR-4.4)
            "review_pending": [t.get("id") for t in in_active if t.get("sprint_review") == "pending"],
        }

    def counts(key):
        result = {}
        for t in tasks:
            for value in key(t):
                result[str(value)] = result.get(str(value), 0) + 1
        return result

    status_order = [s["id"] for s in vocab["statuses"]]
    by_status = counts(lambda t: [t.get("status")])
    by_status = {s: by_status[s] for s in status_order if s in by_status} | {
        s: n for s, n in by_status.items() if s not in status_order
    }
    in_progress = [
        {
            "id": t.get("id"),
            "name": t.get("name", ""),
            "by": (store["claims"].get(t.get("id")) or {}).get("agent") or started_by(t),
        }
        for t in tasks
        if kind_of(vocab, t.get("status")) == "active"
    ]
    waiting = [
        {"id": t.get("id"), "name": t.get("name", ""), "on": [d for d, _ in unfinished_dependencies(data, t)]}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "todo" and unfinished_dependencies(data, t)
    ]
    blocked = [
        {"id": t.get("id"), "name": t.get("name", ""), "waiting_on": t.get("waiting_on"), "questions": t.get("questions", [])}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "blocked"
    ]
    in_review = [
        {"id": t.get("id"), "name": t.get("name", ""), "since": t.get("review_requested_at")}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "review"
    ]
    # Blocked or in review in another worktree (T-204): this worktree's copy doesn't say so until the branches merge
    task_names = {t.get("id"): t.get("name", "") for t in tasks}
    listed = {i["id"] for i in blocked + in_review}
    for record in store["waiting"].values():
        if not isinstance(record, dict) or record.get("worktree") == paths["root"] or record.get("task") in listed:
            continue
        item = {"id": record.get("task"), "name": task_names.get(record.get("task"), ""), "branch": record.get("branch")}
        if kind_of(vocab, record.get("status")) == "review":
            in_review.append({**item, "since": record.get("at")})
        else:
            blocked.append({**item, "waiting_on": record.get("waiting_on"), "questions": record.get("questions") or []})
    nxt, reason = next_task(data, store)
    sizes = sorted(((len(json.dumps(t, ensure_ascii=False)), t.get("id")) for t in tasks), reverse=True)
    result = {
        "sprint": sprint,
        "tasks": len(tasks),
        "by_status": by_status,
        "by_sprint": counts(lambda t: [(find_sprint(vocab, t.get("sprint")) or {}).get("id") or "backlog"]),
        "by_type": counts(lambda t: [t.get("type")]),
        "by_label": counts(lambda t: t.get("labels") or []),
        "in_progress": in_progress,
        "in_review": in_review,
        "blocked": blocked,
        "waiting_on_dependencies": waiting,
        "next": {"id": nxt.get("id"), "name": nxt.get("name", "")} if nxt else None,
        "next_reason": reason,
        "file_bytes": os.path.getsize(paths["tasks"]),
        "largest_tasks": [{"id": task_id, "bytes": size} for size, task_id in sizes[:3]],
        "archived": len(load_archive(paths)["tasks"]),
        "archive_bytes": os.path.getsize(archive_path(paths)) if os.path.isfile(archive_path(paths)) else 0,
    }
    # Open tasks left in closed sprints (T-330), whatever the scope: the loop's default selection never takes them
    stranded = open_in_closed_sprints(vocab, data["tasks"])
    if stranded:
        result["open_in_closed_sprints"] = stranded
    hint = archive_hint(paths, data)
    if hint:
        result["suggest"] = {"archive": hint}
    if args.json:
        out(to_json(result))
        return
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
    elif vocab["sprints"]:
        out("sprint: none active (plan one with /atm:plan-sprint)")
    if stranded:
        n = len(stranded)
        out(
            f"warning: {n} open task{'' if n == 1 else 's'} in closed sprints, which the loop won't take: "
            + ", ".join(f"{t['id']} ({t['sprint']})" for t in stranded)
            + " (atm move <id> --to backlog or --to active)"
        )
    out(f"{len(tasks)} tasks: {pairs(by_status)}")
    if vocab["sprints"]:
        out(f"by sprint: {pairs(result['by_sprint'])}")
    out(f"by type: {pairs(result['by_type'])}")
    if result["by_label"]:
        out(f"by label: {pairs(result['by_label'])}")
    for title, items, extra in (
        ("in progress", in_progress, lambda i: f" ({i['by']})" if i["by"] else ""),
        ("in review", in_review, lambda i: (f" (since {i['since']})" if i["since"] else "") + elsewhere(i)),
        ("blocked", blocked, lambda i: (f" (on {i['waiting_on']})" if i["waiting_on"] else "") + elsewhere(i)),
        ("waiting on dependencies", waiting, lambda i: f" (on {', '.join(i['on'])})"),
    ):
        if items:
            out(f"{title}: " + "; ".join(f"{i['id']} {i['name']}{extra(i)}" for i in items))
    out(f"next: {nxt.get('id')} {nxt.get('name', '')}" if nxt else f"next: none ({reason})")
    largest = ", ".join(f"{i['id']} {i['bytes'] // 1024 or 1} KB" for i in result["largest_tasks"])
    out(f"file: {result['file_bytes'] // 1024} KB" + (f"; largest tasks: {largest}" if largest else ""))
    if result["archived"]:
        out(f"archive: {result['archived']} tasks, {result['archive_bytes'] // 1024} KB")
    if hint:
        out(hint["text"])
