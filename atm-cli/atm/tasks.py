"""The status commands (start, review, done, fail, release, reject) and notes."""

import os

from .claims import (
    agent_identity,
    apply_resolutions,
    check_holder,
    completion,
    expire_claims,
    locked_with_claims,
    new_claim,
    own_lease_expired,
    read_store,
    report_expired,
    waiting_record,
    write_store,
)
from .core import AtmError, now_iso, out, to_json, write_json_atomic
from .lock import locked
from .model import (
    add_note,
    archive_hint,
    archived_task,
    change_status,
    find_sprint,
    find_task,
    kind_of,
    load_tasks,
    move_out_hint,
    touch,
    vocabulary,
)
from .paths import find_paths
from .cli.render import describe_task
from .lib.query import ready_tasks


# The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing
OWN_LEASE_COMMANDS = ("done", "release", "fail", "review", "block")

# Decisions that may be made in any worktree about a task waiting in another (T-204): the command, and the kinds the
# waiting task must have there
REMOTE_DECISIONS = {"unblock": ("blocked",), "skip": ("blocked",), "done": ("review",), "reject": ("review",)}


def adopt_waiting(task, data, store, paths, command):
    """When `task` waits in another worktree (blocked or in review there, per the shared store) but not in this one,
    and `command` decides it: give this copy the waiting state, so the command applies here as it would there. Returns
    the worktree it waits in (where the decision is applied too, by apply_resolutions), or None."""
    record = store["waiting"].get(task["id"])
    if command not in REMOTE_DECISIONS or not isinstance(record, dict) or record.get("worktree") == paths["root"]:
        return None
    vocab = vocabulary(data)
    if kind_of(vocab, task.get("status")) in REMOTE_DECISIONS[command]:
        return None  # it waits here too: decided here as usual
    if kind_of(vocab, record.get("status")) not in REMOTE_DECISIONS[command]:
        return None
    task["status"] = record["status"]
    if record.get("waiting_on"):
        task["waiting_on"] = record["waiting_on"]
    if record.get("questions"):
        task["questions"] = list(record["questions"])
    return record.get("worktree")


def cmd_status_change(command):
    def run(args):
        task_id, agent = args.id, agent_identity(args.agent)
        paths = find_paths(os.getcwd())
        with locked_with_claims(paths):
            data = load_tasks(paths)
            store = read_store(paths)
            claims = store["claims"]
            expired = expire_claims(paths, data, claims)
            apply_resolutions(paths, data, store)
            task = find_task(data, task_id)
            if command in OWN_LEASE_COMMANDS:
                own = own_lease_expired(expired, data, task_id, agent, f"atm {command} {task_id}")
                if own:
                    # Write the expiry first, so `atm show` agrees with the error
                    write_json_atomic(paths["tasks"], data)
                    write_store(paths, store)
                    raise own
            check_holder(claims, task_id, agent, command)
            duplicate_of = getattr(args, "duplicate_of", None)
            if duplicate_of and not any(t.get("id") == duplicate_of for t in data["tasks"]) and not archived_task(duplicate_of):
                raise AtmError(f"{task_id} can't be a duplicate of {duplicate_of}, which doesn't exist", 2)
            waits_in = adopt_waiting(task, data, store, paths, command)
            previous = change_status(
                task,
                data,
                command,
                reason=getattr(args, "reason", None),
                agent=agent,
                status=args.status,
                note=getattr(args, "note", None),
                commit=getattr(args, "commit", None),
                waiting_on=getattr(args, "on", None),
                questions=getattr(args, "question", None) or [],
                answers=getattr(args, "answer", None) or [],
                resolution=getattr(args, "resolution", None),
                duplicate_of=duplicate_of,
            )
            if command == "start":
                claims[task_id] = new_claim(paths, data, task_id, agent)
                store["completed"].pop(task_id, None)  # starting a task again on purpose
            elif command == "reopen":
                claims.pop(task_id, None)
                store["completed"].pop(task_id, None)  # not done any more, in any worktree
            else:
                claims.pop(task_id, None)
            if command == "done":
                store["completed"][task_id] = completion(paths, task_id, agent)
            # Blocked or in review: recorded for every worktree (T-204); any other change ends the wait
            if command in ("block", "review"):
                store["waiting"][task_id] = waiting_record(paths, task)
            else:
                store["waiting"].pop(task_id, None)
                resolution = store["resolved"].pop(task_id, None)
                if waits_in:
                    store["resolved"][task_id] = {
                        "task": task_id,
                        "command": command,
                        "answers": list(getattr(args, "answer", None) or []),
                        "reason": getattr(args, "reason", None),
                        "by": agent,
                        "at": now_iso(),
                        "worktree": waits_in,
                    }
                elif resolution and resolution.get("worktree") != paths["root"]:
                    store["resolved"][task_id] = resolution  # still to be applied where it waits
            unblocked = newly_ready(data, task_id) if command == "done" else []
            write_json_atomic(paths["tasks"], data)
            # The archive hint after done, at most once a day per clone (CLI-5.2), so a loop doesn't repeat it
            hint = archive_hint(paths, data) if command == "done" else None
            today = now_iso()[:10]
            if hint and store["hints"].get("archive") == today:
                hint = None
            if hint:
                store["hints"]["archive"] = today
            write_store(paths, store)
        report_expired(expired)
        # Reopened in a closed sprint, the task is open where the loop's default selection never looks (T-330)
        stranded = None
        if command == "reopen":
            sprint = find_sprint(vocabulary(data), task.get("sprint"))
            if sprint and sprint["state"] == "closed":
                stranded = f"{task['id']} is now open in {move_out_hint(task['id'], sprint['id'])}"
        if args.json:
            extra = {"unblocked": unblocked} if command == "done" else {}
            if hint:
                extra["suggest"] = {"archive": hint}
            if stranded:
                extra["hint"] = stranded
            out(to_json({**task, "claim": claims.get(task_id), **extra}))
            return
        # New lines only after the existing one (CLI-3.2)
        out(f"{task['id']}: {previous} -> {task['status']}")
        if stranded:
            out(f"  {stranded}")
        if command == "start":
            out(describe_task(data, task))
        if unblocked:
            out("Now ready: " + ", ".join(unblocked))
        if hint:
            out(hint["text"])

    return run


def newly_ready(data, task_id):
    """The to-do tasks waiting on `task_id` whose dependencies are now all done."""
    ready = {t.get("id") for t in ready_tasks(data)}
    return [t.get("id") for t in data["tasks"] if task_id in (t.get("dependencies") or []) and t.get("id") in ready]


def cmd_note(args):
    """Add a note to a task, whatever its status (DM-4)."""
    agent = agent_identity(args.agent)
    paths = find_paths(os.getcwd())
    with locked(paths["dir"]):
        data = load_tasks(paths)
        task = find_task(data, args.id)
        stamp = now_iso()
        add_note(task, "note", args.text, agent, stamp)
        touch(task, agent, stamp)
        write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json(task["notes"][-1]))
    else:
        out(f"{task['id']}: note added ({len(task['notes'])} in all)")
