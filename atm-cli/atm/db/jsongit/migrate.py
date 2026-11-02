"""`atm migrate`, the JSON-in-git store's migrations of old layouts: move a legacy tasks.json into the task folder and
set the folder up (.gitignore, merge driver), clean up after per-project dashboards (0.10.0-0.14.0: the dashboard now
comes from the plugin, served by `atm serve` for all projects), and bring old task data up to date (finish times,
phases to sprints, `with: user`)."""

import os
import signal

from ...common.core import DASHBOARD_FILE, AtmError, now_iso, read_json, rel, whoami, write_json_atomic
from ...common.model import MAX_SPRINT_NAME, kind_of, vocabulary
from .claims import CLAIMS_SNAPSHOT, read_store, write_claims_snapshot
from .files import archive_hint, archive_path, load_archive, load_tasks
from .gitrun import git, in_git, is_tracked
from .lock import locked
from .merge import ensure_merge_driver

# Runtime files in the task folder that must not be committed (the dashboard and server files are from before
# 0.15.0; they stay listed so leftovers stay out of git)
IGNORED = ["tasks-dashboard.html", ".server.json", ".server.log", "claims.json", "claims-store.json", ".lock/"]
OLD_SERVER_RECORD = ".server.json"
OLD_SERVER_LOG = ".server.log"


def written_by_atm(path):
    """Whether an old server record is one atm wrote here: owned by this user and not tracked by git (atm always kept
    it out of git). A record that came with the repository is only deleted: atm never contacts its port or signals its
    pid (docs/security-review.md F14)."""
    try:
        if os.stat(path).st_uid != os.getuid():
            return False
    except (OSError, AttributeError):  # missing, or no uids (Windows)
        return False
    return not is_tracked(os.path.dirname(path), path)


def clean_up_old_dashboard(task_dir):
    """Per-project dashboards (0.10.0-0.14.0): delete the project's copy of the dashboard, stop its server if one still
    runs (only when whoami confirms it serves this folder), and remove the server's record and log. Returns what was
    done, or None when there was nothing left."""
    removed = False
    copy = os.path.join(task_dir, DASHBOARD_FILE)
    if os.path.isfile(copy):
        os.remove(copy)
        removed = True
    stopped = None
    record_path = os.path.join(task_dir, OLD_SERVER_RECORD)
    record = read_json(record_path) if written_by_atm(record_path) else None
    if record is not None:
        info = whoami(record.get("port")) if isinstance(record, dict) else None
        if info and info.get("folder") == task_dir and info.get("pid") == record.get("pid"):
            try:
                os.kill(record["pid"], signal.SIGTERM)
                stopped = record["pid"]
            except OSError:
                pass
    had_files = False
    for name in (OLD_SERVER_RECORD, OLD_SERVER_LOG):
        p = os.path.join(task_dir, name)
        if os.path.exists(p):
            os.remove(p)
            had_files = True
    if not (removed or stopped or had_files):
        return None
    return {"dashboard": removed, "server": stopped}


FINISH_FIELDS = {"done": "completed_at", "skipped": "skipped_at"}


def missing_finish_times(tasks, vocab):
    """The finished tasks without their finish time (done ones from before completed_at existed; skipped ones from
    before skipped_at existed), with the field each one needs."""
    result = []
    for task in tasks:
        field = FINISH_FIELDS.get(kind_of(vocab, task.get("status"))) if isinstance(task, dict) else None
        if field and not task.get(field) and task.get("updated_at"):
            result.append((task, field))
    return result


def backfill_finish_times(paths):
    """Give finished tasks without a finish time their updated_at, which is what the dashboard showed for them anyway,
    so a later edit (which moves updated_at) doesn't move them in time. In tasks.json and the archive; once per task;
    updated_at and updated_by stay. Returns how many tasks were filled in."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        if not missing_finish_times(data["tasks"], vocab) and not missing_finish_times(load_archive(paths)["tasks"], vocab):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    filled = 0
    with locked(paths["dir"]):
        data = load_tasks(paths)
        files = [(paths["tasks"], data)] + ([(archive, load_archive(paths))] if os.path.isfile(archive) else [])
        for path, contents in files:
            missing = missing_finish_times(contents["tasks"], vocab)
            for task, field in missing:
                task[field] = task["updated_at"]
            if missing:
                write_json_atomic(path, contents)
                filled += len(missing)
    return filled


FINISHED_KINDS = ("done", "skipped")


def task_phase(task):
    """The task's phase (a number), or None: a task without one goes to the backlog."""
    phase = task.get("phase") if isinstance(task, dict) else None
    if not isinstance(phase, (int, float)) or isinstance(phase, bool):
        return None
    return int(phase) if float(phase).is_integer() else phase


def phase_sprint_names(meta, tasks):
    """{phase: sprint name} for every phase with tasks or a name, in phase order: its name, or "Phase n" when
    unnamed, with " (phase n)" added to a name already taken (the names the dashboard shows for unmigrated projects,
    dashboard/src/lib/sprints.ts)."""
    named = {}
    for p in meta.get("phases") or []:
        if isinstance(p, dict) and task_phase({"phase": p.get("id")}) is not None:
            name = p.get("name").strip()[:MAX_SPRINT_NAME].rstrip() if isinstance(p.get("name"), str) else ""
            if name:
                named[task_phase({"phase": p["id"]})] = name
    phases = set(named) | {task_phase(t) for t in tasks} - {None}
    names, taken = {}, set()
    for phase in sorted(phases):
        name = named.get(phase) or f"Phase {phase}"
        if name.casefold() in taken:
            suffix = f" (phase {phase})"
            name = name[: MAX_SPRINT_NAME - len(suffix)].rstrip() + suffix
        taken.add(name.casefold())
        names[phase] = name
    return names


def sprint_from_phase(name, tasks, vocab):
    """The sprint a phase becomes: closed when all its tasks are finished (start, end and report from them), else
    planned (SR-8.1)."""
    finished = bool(tasks) and all(kind_of(vocab, t.get("status")) in FINISHED_KINDS for t in tasks)
    if not finished:
        return {"id": name, "state": "planned"}
    sprint = {"id": name, "state": "closed"}
    starts = [t["started_at"] for t in tasks if isinstance(t.get("started_at"), str)]
    ends = [
        t.get("completed_at") or t.get("skipped_at") or t.get("updated_at")
        for t in tasks
        if isinstance(t.get("completed_at") or t.get("skipped_at") or t.get("updated_at"), str)
    ]
    if starts:
        sprint["start"] = min(starts)
    if ends:
        sprint["end"] = max(ends)
    sprint["report"] = {
        "done": [t["id"] for t in tasks if kind_of(vocab, t.get("status")) == "done" and "id" in t],
        "carried": [],
    }
    return sprint


def phases_to_sprints(data, archived, vocab, stamp):
    """Turn phases into sprints in tasks.json's data and the archive's tasks, in place (SR-8.1). A project that already
    has meta.sprints only gets its leftover phases converted (a task merged in from an unmigrated branch, SR-8.5).
    Returns the sprints it added, or [] when there was nothing to convert."""
    meta = data.setdefault("meta", {})
    tasks = [t for t in list(data["tasks"]) + list(archived) if isinstance(t, dict)]
    if not meta.get("phases") and not any("phase" in t for t in tasks):
        return []
    names = phase_sprint_names(meta, tasks)
    sprints = meta.get("sprints") if isinstance(meta.get("sprints"), list) else []
    existing = {s["id"].casefold() for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    added = []
    for phase, name in names.items():
        if name.casefold() in existing:
            continue
        sprint = sprint_from_phase(name, [t for t in tasks if task_phase(t) == phase], vocab)
        sprints.append(sprint)
        added.append(sprint)
    meta["sprints"] = sprints
    meta.pop("phases", None)
    by_name = {s["id"].casefold(): s["id"] for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    for task in tasks:
        phase = task_phase(task)
        task.pop("phase", None)
        if phase is None or task.get("sprint"):
            continue
        task["sprint"] = by_name[names[phase].casefold()]
        task.setdefault("sprint_history", []).append(
            {"sprint": task["sprint"], "from": task.get("created_at") if isinstance(task.get("created_at"), str) else stamp}
        )
    return added


def migrate_phases(paths):
    """phases_to_sprints on the files under the lock, staging them in git (not committing). Returns the sprints it
    added, or None when there was nothing to convert."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        archive_data = load_archive(paths)
        if not data.get("meta", {}).get("phases") and not any(
            isinstance(t, dict) and "phase" in t for t in list(data["tasks"]) + list(archive_data["tasks"])
        ):
            return None
    except AtmError:
        return None  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive_data = load_archive(paths)
        added = phases_to_sprints(data, archive_data["tasks"], vocab, now_iso())
        if isinstance(archive_data.get("meta"), dict):
            archive_data["meta"].pop("phases", None)
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
        written = [paths["tasks"]]
        if os.path.isfile(archive):
            write_json_atomic(archive, archive_data)
            written.append(archive)
    if in_git(paths["root"]):
        git(["add", "--"] + [rel(paths["root"], p) for p in written], paths["root"])
    return added


WITH_USER_MIGRATION = "interactive-label"


def migrate_interactive(paths):
    """Tasks labelled `interactive` (work done with the user, T-240) get `with: user` (SR-6.5), once: the change is
    recorded in meta.migrations, so a `with` taken off later isn't put back. The label stays. Only unfinished tasks in
    tasks.json; nothing is written when none has the label. Returns how many tasks it changed."""
    def todo(data):
        meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
        if WITH_USER_MIGRATION in (meta.get("migrations") or []):
            return []
        vocab = vocabulary(data)
        return [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and "interactive" in (t.get("labels") or [])
            and not t.get("with")
            and kind_of(vocab, t.get("status")) not in ("done", "skipped")
        ]

    try:
        if not todo(load_tasks(paths)):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        tasks = todo(data)
        for task in tasks:
            task["with"] = "user"
        meta = data.setdefault("meta", {})
        meta["migrations"] = list(meta.get("migrations") or []) + [WITH_USER_MIGRATION]
        write_json_atomic(paths["tasks"], data)
    if in_git(paths["root"]):
        git(["add", "--", rel(paths["root"], paths["tasks"])], paths["root"])
    return len(tasks)


def ensure_gitignore(task_dir):
    """Add any missing runtime entries to the folder's .gitignore, keeping the user's lines. Returns True if changed."""
    p = os.path.join(task_dir, ".gitignore")
    try:
        with open(p, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    lines = text.splitlines()
    missing = [entry for entry in IGNORED if entry not in lines]
    if not missing:
        return False
    if not text:
        text = "# atm runtime files (managed by atm)\n"
    elif not text.endswith("\n"):
        text += "\n"
    with open(p, "w", encoding="utf-8") as f:
        f.write(text + "".join(entry + "\n" for entry in missing))
    return True


def migrate(paths):
    """Move a legacy tasks.json into the task folder, set the folder up, and turn phases into sprints. Never commits."""
    root, task_dir, legacy = paths["root"], paths["dir"], paths["legacy"]
    result = {
        "moved": None,
        "removed_dashboard": None,
        "warning": None,
        "gitignore_updated": False,
        "merge_driver_updated": False,
        "cleaned_up": None,
        "finish_times_filled": 0,
        "sprints_from_phases": None,
        "with_user_from_label": 0,
    }
    if legacy and paths["tasks_exists"]:
        result["warning"] = (
            f"both {rel(root, legacy['tasks'])} and {rel(root, paths['tasks'])} exist; "
            f"using {rel(root, paths['tasks'])}. Remove the old file when you no longer need it."
        )
    elif legacy:
        os.makedirs(task_dir, exist_ok=True)
        tracked = is_tracked(root, legacy["tasks"])
        if tracked:
            if git(["mv", "--", rel(root, legacy["tasks"]), rel(root, paths["tasks"])], root) is None:
                raise AtmError(f"git mv {rel(root, legacy['tasks'])} failed")
        else:
            os.rename(legacy["tasks"], paths["tasks"])
        result["moved"] = {"from": rel(root, legacy["tasks"]), "to": rel(root, paths["tasks"]), "git": tracked}
        if legacy["dashboard"]:
            if is_tracked(root, legacy["dashboard"]):
                git(["rm", "--cached", "-q", "--", rel(root, legacy["dashboard"])], root)
            os.remove(legacy["dashboard"])
            result["removed_dashboard"] = rel(root, legacy["dashboard"])
    if not os.path.isfile(paths["tasks"]):
        return result
    if ensure_gitignore(task_dir):
        result["gitignore_updated"] = True
        if in_git(root):
            git(["add", "--", rel(root, os.path.join(task_dir, ".gitignore"))], root)
    result["merge_driver_updated"] = ensure_merge_driver(paths)
    if not os.path.exists(os.path.join(task_dir, CLAIMS_SNAPSHOT)):
        # So the dashboard finds the file from the start instead of logging a 404 on every refresh
        write_claims_snapshot(paths, read_store(paths)["claims"])
    result["cleaned_up"] = clean_up_old_dashboard(task_dir)
    result["finish_times_filled"] = backfill_finish_times(paths)
    result["sprints_from_phases"] = migrate_phases(paths)
    result["with_user_from_label"] = migrate_interactive(paths)
    try:
        hint = archive_hint(paths, load_tasks(paths))
    except AtmError:
        hint = None  # the command the skill runs next reports the problem
    if hint:
        result["suggest"] = {"archive": hint}
    return result


def describe_sprints_from_phases(sprints):
    """What the phase migration did, said once (it doesn't run again)."""
    if not sprints:
        return ["Moved the tasks' phases to their sprints (staged, not committed)."]
    closed = [s["id"] for s in sprints if s["state"] == "closed"]
    planned = [s["id"] for s in sprints if s["state"] == "planned"]
    count = "1 phase" if len(sprints) == 1 else f"{len(sprints)} phases"
    lines = [f"Phases are now sprints: turned {count} into sprints (staged, not committed)."]
    if closed:
        lines.append(f"Closed (all their tasks finished): {', '.join(closed)}.")
    if planned:
        lines.append(f"Planned: {', '.join(planned)}.")
        lines.append("None is active yet: plan one with /atm:plan-sprint.")
    return lines


def describe_migration(r, paths):
    lines = []
    if r["warning"]:
        lines.append("warning: " + r["warning"])
    if r["moved"]:
        line = f"Moved {r['moved']['from']} to {r['moved']['to']}"
        if r["moved"]["git"]:
            line += " (staged with git mv, not committed)"
        if r["removed_dashboard"]:
            line += f"; removed {r['removed_dashboard']} (the dashboard now comes with the plugin: /atm:serve)"
        lines.append(line + ".")
    elif not os.path.isfile(paths["tasks"]):
        lines.append("No tasks.json found. Run /atm:create-tasks to create one.")
    cleaned = r["cleaned_up"]
    if cleaned:
        line = "Removed this project's old dashboard files"
        if cleaned["server"]:
            line += f" and stopped its old dashboard server (pid {cleaned['server']})"
        lines.append(line + ": one dashboard for all your projects now comes with the plugin (/atm:serve).")
    filled = r.get("finish_times_filled")
    if filled:
        tasks = "1 finished task" if filled == 1 else f"{filled} finished tasks"
        lines.append(f"Filled in the finish time of {tasks} that had none, from the last update (completed_at/skipped_at).")
    sprints = r.get("sprints_from_phases")
    if sprints is not None:
        lines.extend(describe_sprints_from_phases(sprints))
    together = r.get("with_user_from_label")
    if together:
        tasks = "1 task" if together == 1 else f"{together} tasks"
        lines.append(
            f"Gave {tasks} labelled interactive `with: user` (done together with you; unattended loops skip them)."
        )
    if not lines:
        lines.append("Nothing to migrate.")
    if r.get("suggest"):
        lines.append(r["suggest"]["archive"]["text"])
    return lines
