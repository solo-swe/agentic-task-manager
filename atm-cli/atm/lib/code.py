"""atm-lib: the project's workflow (meta.workflow: direct or pull-request, docs/v4/skills-and-workflows.md §1) and the
code side of tasks: committing a task's files and recording the commit, the PR that contains tasks, its merge
(docs/v4/atm-cli.md §4.5, data-model.md §4; docs/layered-architecture.md §2, §5.1).

The commit goes through the project's version control (`project.vcs`); the files that travel with a code commit
(tasks.json, for the JSON-in-git store) are the store's to say (`store.files_for_commit()`)."""

import re

from ..common.errors import AtmFailure, Invalid, NotFound
from ..common.core import now_iso, parse_iso
from .loop import archived_in
from ..common.model import WORKFLOW_DEFAULTS, WORKFLOW_TYPES, effective_release, find_task, kind_of, touch, vocabulary, workflow_of

HTTP_URL = re.compile(r"https?://[^\s]+\Z", re.IGNORECASE)
CO_AUTHORED = re.compile(r"^co-authored-by:", re.IGNORECASE)


def workflow_settings(project):
    """The project's workflow (meta.workflow with its defaults), or None when none is set."""
    return workflow_of(project.store.document())


def set_workflow(project, workflow_type, base=None, verify=None, max_rounds=None, commit_template=None, trailers=None, trailer=()):
    """Set the workflow: its type, and what is given of the rest; `trailers` forbid, require (with `trailer` lines) or
    none (removes the rule). Returns the workflow with its defaults."""
    if workflow_type not in WORKFLOW_TYPES:
        raise Invalid(f"atm workflow set needs a type: {' or '.join(WORKFLOW_TYPES)}")
    changes = {"type": workflow_type}
    if base is not None:
        changes["base"] = base
    if verify is not None:
        changes["verify"] = verify
    if max_rounds is not None:
        changes["max_rounds"] = max_rounds
    if commit_template is not None:
        changes["commit_template"] = commit_template
    if trailers == "forbid":
        changes["trailers"] = "forbid"
    elif trailers == "require":
        if not trailer:
            raise Invalid("--trailers require needs at least one --trailer LINE")
        changes["trailers"] = {"require": list(trailer)}

    def change(work):
        data = work.document
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        current = data["meta"].get("workflow") if isinstance(data["meta"].get("workflow"), dict) else {}
        workflow = {**current, **changes}
        if trailers == "none":
            workflow.pop("trailers", None)
        data["meta"]["workflow"] = workflow
        return workflow_of(data)  # validates before anything is written

    return project.store.change(change)


def commit_message(text, trailers):
    """Apply the workflow's trailer rule: forbid removes Co-Authored-By lines, require adds the missing lines."""
    if trailers == "forbid":
        text = "\n".join(line for line in text.splitlines() if not CO_AUTHORED.match(line.strip())).rstrip()
    elif isinstance(trailers, dict):
        missing = [line for line in trailers["require"] if line not in text.splitlines()]
        if missing:
            text = text.rstrip() + "\n\n" + "\n".join(missing)
    return text


def record_on_task(project, task_id, sha):
    """The task's commit is `sha` (the SHA is known only once the commit exists: a change for the next commit)."""

    def change(work):
        task = find_task(work.document, task_id, archived_in(work))
        task["commit"] = sha
        touch(task, project.actor, now_iso())
        return task

    return project.store.change(change)


def commit_task(project, task_id, files, cwd, message=None):
    """`atm commit`: commit exactly `files` (named relative to `cwd`) plus the task data that travels with code commits,
    with the workflow's message, then record the commit on the task. Returns {task, commit, message, files}."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm commit needs a git repository")
    if not files:
        raise Invalid("atm commit needs --file (once per file the task changed), or --record REV")
    data = project.store.document()
    task = find_task(data, task_id, project.store.get_archived_task)
    workflow = workflow_of(data) or WORKFLOW_DEFAULTS
    text = message if message is not None else workflow["commit_template"]
    for key in ("id", "name", "phase", "type"):
        text = text.replace("{" + key + "}", str(task.get(key, "")))
    text = commit_message(text, workflow.get("trailers"))
    if not text.strip():
        raise Invalid("the commit message is empty")
    picked = vcs.paths(files, cwd)
    picked += [f for f in project.store.files_for_commit() if f not in picked]
    sha = vcs.commit(picked, text)
    task = record_on_task(project, task_id, sha)
    return {"task": task["id"], "commit": sha, "message": text, "files": picked}


def record_commit(project, task_id, rev):
    """`atm commit --record` (T-343): record an existing commit on the current branch, e.g. the one a rebase or a
    cherry-pick made of the task's commit, without committing anything. Returns {task, commit, recorded, summary,
    hint}: `hint` when the commit's message doesn't mention the task."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm commit needs a git repository")
    if rev.startswith("-"):
        raise Invalid(f"--record takes a revision, not an option: {rev!r}")
    sha = vcs.resolve(rev)
    if not sha:
        raise NotFound(f"no commit {rev} in this repository")
    if not vcs.in_history(sha):
        raise AtmFailure(f"{sha[:7]} is not on the current branch (an ancestor of HEAD)")
    text = vcs.message(sha)
    task = record_on_task(project, task_id, sha)
    hint = None
    if not re.search(rf"(?<![\w-]){re.escape(task['id'])}(?![\w-])", text):
        hint = f"its message doesn't mention {task['id']}: is it the task's commit?"
    return {"task": task["id"], "commit": sha, "recorded": True, "summary": text.splitlines()[0] if text else "", "hint": hint}


def record_pr(project, ids, number, url):
    """`atm pr`: the PR that contains the tasks (DM-4.3: the tasks are then In PR). Returns the task IDs."""
    if number <= 0:
        raise Invalid("--number must be a positive whole number")
    # The dashboard links to it: only a web address, never javascript: or data: (docs/security-review.md F9)
    if not HTTP_URL.match(url):
        raise Invalid(f"--url must be an http:// or https:// address, not {url!r}")

    def change(work):
        archived = archived_in(work)
        found = [find_task(work.document, task_id, archived) for task_id in ids]
        stamp = now_iso()
        for task in found:
            task["pr"] = {"number": number, "url": url}
            task.pop("merged_at", None)
            touch(task, project.actor, stamp)
        return [t["id"] for t in found]

    return project.store.change(change)


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


def record_merge(project, pr, at=None):
    """`atm merged`: PR `pr` was merged (at `at`, default now), on every task that records it (DM-4.3: the tasks are
    then Merged); the releases it completes are released. Returns {pr, tasks, already, released, pending}."""
    if at is not None:
        try:
            parse_iso(at)
        except ValueError:
            raise Invalid(f"--at needs a time like 2026-10-24T18:00:00Z, not {at!r}")

    def change(work):
        data = work.document
        tasks = [
            t for t in data["tasks"] if isinstance(t, dict) and isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr
        ]
        if not tasks:
            raise NotFound(f"no task records PR #{pr} (atm pr records it)")
        stamp = now_iso()
        changed = [t for t in tasks if not t.get("merged_at")]
        for task in changed:
            task["merged_at"] = at or stamp
            touch(task, project.actor, stamp)
        released, pending = release_with_merge(data, tasks, pr, at or stamp)
        return {
            "pr": pr,
            "tasks": [t["id"] for t in changed],
            "already": [t["id"] for t in tasks if t not in changed],
            "released": released,
            "pending": pending,
        }

    return project.store.change(change)
