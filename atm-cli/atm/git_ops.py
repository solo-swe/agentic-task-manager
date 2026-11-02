"""The project's workflow (meta.workflow: direct or pull-request, docs/v4/skills-and-workflows.md §1) and the git
side of tasks: atm workflow, atm commit, atm pr, atm merged (docs/v4/atm-cli.md §4.5, data-model.md §4)."""

import os
import re
import subprocess

from .claims import agent_identity
from .core import AtmError, git, in_git, now_iso, out, parse_iso, to_json, write_json_atomic
from .lock import locked
from .model import (
    WORKFLOW_DEFAULTS,
    WORKFLOW_TYPES,
    effective_release,
    find_task,
    kind_of,
    load_tasks,
    touch,
    vocabulary,
    workflow_of,
)
from .paths import find_paths

# Pathspecs that stage more than the files named: another agent's work can be in the same tree
WHOLE_TREE = {".", "-A", "--all", "-u", "--update", ":/", "*"}
# git reads a path as a pattern (`*.env` is every .env file, `src/[id]` matches `src/i`); with this, a --file names
# exactly one file (docs/security-review.md F6)
LITERAL = ["--literal-pathspecs"]
HTTP_URL = re.compile(r"https?://[^\s]+\Z", re.IGNORECASE)
CO_AUTHORED = re.compile(r"^co-authored-by:", re.IGNORECASE)


def default_base(root):
    """The remote's default branch (origin/HEAD), else main."""
    ref = git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], root)
    return ref.split("/", 1)[1] if ref and "/" in ref else "main"


def describe_workflow(workflow, root):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm workflow set direct|pull-request`."
    trailers = workflow.get("trailers")
    lines = [
        f"workflow: {workflow['type']}",
        "  base: " + (workflow["base"] if workflow.get("base") else f"{default_base(root)} (the remote's default branch)"),
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
    paths = find_paths(os.getcwd())
    if args.action is None:
        workflow = workflow_of(load_tasks(paths))
        out(to_json(workflow or {"type": None}) if args.json else describe_workflow(workflow, paths["root"]))
        return
    if args.type not in WORKFLOW_TYPES:
        raise AtmError(f"atm workflow set needs a type: {' or '.join(WORKFLOW_TYPES)}", 2)
    changes = {"type": args.type}
    if args.base is not None:
        changes["base"] = args.base
    if args.verify is not None:
        changes["verify"] = args.verify == "true"
    if args.max_rounds is not None:
        changes["max_rounds"] = args.max_rounds
    if args.commit_template is not None:
        changes["commit_template"] = args.commit_template
    if args.trailers == "forbid":
        changes["trailers"] = "forbid"
    elif args.trailers == "require":
        if not args.trailer:
            raise AtmError("--trailers require needs at least one --trailer LINE", 2)
        changes["trailers"] = {"require": args.trailer}
    with locked(paths["dir"]):
        data = load_tasks(paths)
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        current = data["meta"].get("workflow") if isinstance(data["meta"].get("workflow"), dict) else {}
        workflow = {**current, **changes}
        if args.trailers == "none":
            workflow.pop("trailers", None)
        data["meta"]["workflow"] = workflow
        effective = workflow_of(data)  # validates before anything is written
        write_json_atomic(paths["tasks"], data)
    out(to_json(effective) if args.json else describe_workflow(effective, paths["root"]))


def run_git(args, root):
    """Run git; return stdout stripped, or raise AtmError with git's own message."""
    result = subprocess.run(["git"] + args, cwd=root, capture_output=True, text=True)
    if result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        command = next((a for a in args if not a.startswith("-")), "")
        raise AtmError(f"git {command} failed: {message}")
    return result.stdout.strip()


def commit_paths(root, files):
    """The files to commit, relative to the root; refuses anything that would stage more than the files named."""
    picked = []
    for name in files:
        if name in WHOLE_TREE or name.startswith(":"):
            raise AtmError(f"atm commit stages only the files you name; {name!r} would stage the whole tree", 2)
        full = os.path.realpath(os.path.join(os.getcwd(), name))
        relative = os.path.relpath(full, root)
        if relative == "." or relative.startswith(".." + os.sep) or relative == "..":
            raise AtmError(f"{name} is outside the repository", 2)
        if os.path.isdir(full):
            raise AtmError(f"{name} is a directory: name the files in it", 2)
        # A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
        if (
            not os.path.exists(full)
            and git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], root) is None
            and git(["cat-file", "-e", f"HEAD:{relative}"], root) is None
        ):
            raise AtmError(f"{name}: no such file, and git doesn't track it")
        if relative not in picked:
            picked.append(relative)
    return picked


def commit_message(text, trailers):
    """Apply the workflow's trailer rule: forbid removes Co-Authored-By lines, require adds the missing lines."""
    if trailers == "forbid":
        text = "\n".join(line for line in text.splitlines() if not CO_AUTHORED.match(line.strip())).rstrip()
    elif isinstance(trailers, dict):
        missing = [line for line in trailers["require"] if line not in text.splitlines()]
        if missing:
            text = text.rstrip() + "\n\n" + "\n".join(missing)
    return text


def record_commit(paths, args, agent):
    """atm commit --record (T-343): record an existing commit on the current branch, e.g. the one a rebase or a
    cherry-pick made of the task's commit, without committing anything."""
    root, rev = paths["root"], args.record
    if args.file or args.message is not None:
        raise AtmError("--record records an existing commit: no --file or --message", 2)
    if rev.startswith("-"):
        raise AtmError(f"--record takes a revision, not an option: {rev!r}", 2)
    sha = git(["rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"], root)
    if not sha:
        raise AtmError(f"no commit {rev} in this repository")
    if git(["merge-base", "--is-ancestor", sha, "HEAD"], root) is None:
        raise AtmError(f"{sha[:7]} is not on the current branch (an ancestor of HEAD)")
    message = run_git(["log", "-1", "--format=%B", sha], root)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        task = find_task(data, args.id)
        task["commit"] = sha
        touch(task, agent, now_iso())
        write_json_atomic(paths["tasks"], data)
    hint = None
    if not re.search(rf"(?<![\w-]){re.escape(task['id'])}(?![\w-])", message):
        hint = f"its message doesn't mention {task['id']}: is it the task's commit?"
    if args.json:
        out(to_json({"task": task["id"], "commit": sha, "recorded": True, **({"hint": hint} if hint else {})}))
        return
    out(f"{task['id']}: recorded {sha[:7]} {message.splitlines()[0] if message else ''}".rstrip())
    if hint:
        out(f"  {hint}")


def cmd_commit(args):
    """Commit exactly the files the agent names plus tasks.json, then record the commit on the task."""
    agent = agent_identity(args.agent)
    paths = find_paths(os.getcwd())
    root = paths["root"]
    if not in_git(root):
        raise AtmError("atm commit needs a git repository")
    if args.record is not None:
        return record_commit(paths, args, agent)
    if not args.file:
        raise AtmError("atm commit needs --file (once per file the task changed), or --record REV", 2)
    data = load_tasks(paths)
    task = find_task(data, args.id)
    workflow = workflow_of(data) or WORKFLOW_DEFAULTS
    template = args.message if args.message is not None else workflow["commit_template"]
    message = template
    for key in ("id", "name", "phase", "type"):
        message = message.replace("{" + key + "}", str(task.get(key, "")))
    message = commit_message(message, workflow.get("trailers"))
    if not message.strip():
        raise AtmError("the commit message is empty", 2)
    files = commit_paths(root, args.file)
    tasks_file = os.path.relpath(paths["tasks"], root)
    if tasks_file not in files:
        files.append(tasks_file)
    # A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
    to_add = [
        f
        for f in files
        if git(LITERAL + ["ls-files", "--error-unmatch", "--", f], root) is not None
        or os.path.lexists(os.path.join(root, f))
    ]
    if to_add:
        run_git(LITERAL + ["add", "-A", "--"] + to_add, root)
    # With paths, git commits only those, so whatever else is staged stays staged and out of this commit
    run_git(LITERAL + ["commit", "-q", "-m", message, "--"] + files, root)
    sha = run_git(["rev-parse", "HEAD"], root)
    # The SHA is known only once the commit exists, so tasks.json records it as a change for the next commit
    with locked(paths["dir"]):
        data = load_tasks(paths)
        task = find_task(data, args.id)
        task["commit"] = sha
        touch(task, agent, now_iso())
        write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json({"task": task["id"], "commit": sha, "message": message, "files": files}))
    else:
        out(f"{task['id']}: committed {sha[:7]} {message.splitlines()[0]}")
        out("  " + ", ".join(files))


def cmd_pr(args):
    """Record the PR that contains the tasks (DM-4.3: the tasks are then In PR)."""
    agent = agent_identity(args.agent)
    if args.number <= 0:
        raise AtmError("--number must be a positive whole number", 2)
    # The dashboard links to it: only a web address, never javascript: or data: (docs/security-review.md F9)
    if not HTTP_URL.match(args.url):
        raise AtmError(f"--url must be an http:// or https:// address, not {args.url!r}", 2)
    paths = find_paths(os.getcwd())
    with locked(paths["dir"]):
        data = load_tasks(paths)
        found = [find_task(data, task_id) for task_id in args.ids]
        stamp = now_iso()
        for task in found:
            task["pr"] = {"number": args.number, "url": args.url}
            task.pop("merged_at", None)
            touch(task, agent, stamp)
        write_json_atomic(paths["tasks"], data)
    ids = [t["id"] for t in found]
    if args.json:
        out(to_json({"tasks": ids, "pr": {"number": args.number, "url": args.url}}))
    else:
        out(f"{', '.join(ids)}: in PR #{args.number} ({args.url})")


def cmd_merged(args):
    """Record that a PR was merged, on every task that records it (DM-4.3: the tasks are then Merged)."""
    agent = agent_identity(args.agent)
    if args.at is not None:
        try:
            parse_iso(args.at)
        except ValueError:
            raise AtmError(f"--at needs a time like 2026-10-24T18:00:00Z, not {args.at!r}", 2)
    paths = find_paths(os.getcwd())
    with locked(paths["dir"]):
        data = load_tasks(paths)
        tasks = [
            t
            for t in data["tasks"]
            if isinstance(t, dict) and isinstance(t.get("pr"), dict) and t["pr"].get("number") == args.pr
        ]
        if not tasks:
            raise AtmError(f"no task records PR #{args.pr} (atm pr records it)")
        stamp = now_iso()
        changed = [t for t in tasks if not t.get("merged_at")]
        for task in changed:
            task["merged_at"] = args.at or stamp
            touch(task, agent, stamp)
        released, pending = release_with_merge(data, tasks, args.pr, args.at or stamp)
        if changed or released:
            write_json_atomic(paths["tasks"], data)
    ids = [t["id"] for t in changed]
    if args.json:
        out(
            to_json(
                {
                    "pr": args.pr,
                    "tasks": ids,
                    "already": [t["id"] for t in tasks if t not in changed],
                    "released": released,
                    "pending": pending,
                }
            )
        )
        return
    if ids:
        out(f"PR #{args.pr} merged: {', '.join(ids)}")
    else:
        out(f"PR #{args.pr}: already recorded as merged")
    for release_id in released:
        out(f"Released {release_id}: all its tasks are merged in PR #{args.pr}.")
    for release_id, waiting in pending.items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{args.pr}.")


def release_with_merge(data, merged, pr_number, at):
    """Release the planned releases the merged PR completes (SR-3.7): every task of the release in tasks.json (its own
    release, or its sprint's) is in this PR and merged; archived and skipped tasks don't hold it back. Sets its state,
    `pr` and `released_at`. Returns (the released IDs, {release: the tasks still keeping it planned})."""
    vocab = vocabulary(data)
    touched = {effective_release(vocab, t) for t in merged} - {None}
    released, pending = [], {}
    for release in (data.get("meta") or {}).get("releases") or []:
        if not isinstance(release, dict) or release.get("id") not in touched:
            continue
        if release.get("state", "planned") != "planned":
            continue
        own = [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and effective_release(vocab, t) == release["id"]
            and kind_of(vocab, t.get("status")) != "skipped"
        ]
        waiting = [
            t.get("id")
            for t in own
            if not (isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr_number and t.get("merged_at"))
        ]
        if waiting:
            pending[release["id"]] = waiting
            continue
        url = next((t["pr"].get("url") for t in merged if isinstance(t["pr"].get("url"), str)), None)
        release["state"] = "released"
        release["pr"] = {"number": pr_number, **({"url": url} if url else {})}
        release["released_at"] = at
        released.append(release["id"])
    return released, pending
