"""The JSON-in-git store's merge driver: merging the task files task by task.

Registered as a git merge driver (`tasks.json merge=atm`), so branches that changed tasks.json merge task by task
instead of line by line. Design: docs/parallel-agents-design.md, option E and layer L3.
"""

import os

from ...common.core import SCRIPT_DIR, AtmError, read_json, rel, write_json_atomic
from ...common.model import kind_of, split_id, vocabulary
from ...host.git import git, in_git

# What `atm` means is not in this file: git config can't be committed, so each clone registers the driver in its own
# .git/config (merge.atm.driver), which the atm skills do on every run. A clone without it merges tasks.json normally.
GITATTRIBUTES_LINES = (
    "# 'atm' is defined in each clone's git config (merge.atm.driver), registered by the atm plugin on every run.\n"
    "# Without it, git merges tasks.json line by line as usual.\n"
    "tasks.json merge=atm\n"
)
# Fields that move with the status: they come from the side whose progress wins
PROGRESS_FIELDS = (
    "status",
    "updated_at",
    "started_at",
    "completed_at",
    "skipped_at",
    "last_failure",
    "agent",
    "started_by",
    "completed_by",
    "updated_by",
    "commit",
    "review_requested_at",
    "review_state",
    "reviewed_at",
    "sprint_review",
    "waiting_on",
    "questions",
    "blocked_at",
)
# How far along a status's kind is (DM-3.3); a status without a kind ranks like todo
KIND_RANK = {"todo": 0, "active": 1, "blocked": 1, "review": 1, "done": 2, "skipped": 2}
MISSING = object()


def merge_vocabulary(*versions):
    """The vocabulary to rank statuses by: this branch's, else the other's, else the defaults. A merge never fails
    over a vocabulary; the next atm command reports it."""
    for data in versions:
        try:
            return vocabulary(data)
        except AtmError:
            continue
    return vocabulary({})


def merge_value(base, ours, theirs):
    """Three-way merge of one value: (value, conflict). On conflict, ours is kept."""
    if ours == theirs:
        return ours, False
    if ours == base:
        return theirs, False
    if theirs == base:
        return ours, False
    return ours, True


def progress_winner(ours, theirs, vocab):
    """Both sides moved a task along: the one further along wins, then the newer update, then ours."""
    rank = lambda task: KIND_RANK.get(kind_of(vocab, task.get("status")), 0)
    rank_o, rank_t = rank(ours), rank(theirs)
    if rank_o != rank_t:
        return ours if rank_o > rank_t else theirs
    return theirs if str(theirs.get("updated_at", "")) > str(ours.get("updated_at", "")) else ours


def merge_notes(ours, theirs):
    """Notes only grow: both sides' notes, each once, oldest first."""
    merged = list(ours) + [n for n in theirs if n not in ours]
    return sorted(merged, key=lambda n: str(n.get("at", "")) if isinstance(n, dict) else "")


def merge_intervals(ours, theirs):
    """Past blocks and reviews only grow: both sides', one per start, oldest first."""
    starts = {i.get("from") for i in ours if isinstance(i, dict)}
    merged = list(ours) + [i for i in theirs if isinstance(i, dict) and i.get("from") not in starts]
    return sorted(merged, key=lambda i: str(i.get("from", "")) if isinstance(i, dict) else "")


def merge_history(ours, theirs):
    """A task's sprint_history only grows: one entry per start (`from`); an entry ended on either side (`to`) is kept
    ended, so a sprint change on one branch closes the entry the other branch still has open."""
    by_start = {}
    for entry in list(ours) + list(theirs):
        if not isinstance(entry, dict):
            continue
        known = by_start.get(entry.get("from"))
        if known is None or ("to" in entry and "to" not in known):
            by_start[entry.get("from")] = entry
    return sorted(by_start.values(), key=lambda e: str(e.get("from", "")))


# meta lists merged item by item, by ID, like tasks (docs/sprints-and-releases-design.md SR-2.4), with how far along
# each state is: changed on both sides, the further state wins
META_LISTS = {
    "sprints": {"planned": 0, "active": 1, "closed": 2},
    "releases": {"planned": 0, "released": 1, "archived": 2},
    "components": None,
}
# Sprints are named, and two names that differ only in case are the same sprint (SR-2.3)
item_key = lambda name, item: (
    item.get("id").casefold() if name == "sprints" and isinstance(item.get("id"), str) else item.get("id")
)


def merge_items(name, base, ours, theirs):
    """Merge three versions of a meta list of {id, ...} items. Returns (items, conflict descriptions)."""
    rank = META_LISTS[name]
    keyed = lambda items: {item_key(name, i): i for i in items if isinstance(i, dict)} if isinstance(items, list) else {}
    b, o, t = keyed(base), keyed(ours), keyed(theirs)
    items, conflicts = [], []
    for key, item in o.items():
        if key not in b:
            other = t.get(key)
            if other is not None and other != item:
                conflicts.append(f"meta.{name}: {item.get('id')!r} was added on both branches, differently")
            items.append(item)
        elif key not in t:
            if item != b[key]:
                conflicts.append(f"meta.{name}: {item.get('id')!r} changed on this branch, deleted on the other")
                items.append(item)
        else:
            merged, fields = merge_item(b[key], item, t[key], rank)
            items.append(merged)
            if fields:
                conflicts.append(f"meta.{name}: both branches changed {item.get('id')!r} ({', '.join(fields)})")
    for key, item in t.items():
        if key not in o and (key not in b or item != b[key]):
            if key in b:
                conflicts.append(f"meta.{name}: {item.get('id')!r} deleted on this branch, changed on the other")
            items.append(item)
    return items, conflicts


def merge_item(base, ours, theirs, rank):
    """One sprint, release or component changed on both sides: field by field; where both changed a field and both
    changed the state, the side further along wins it (a sprint closed on one branch stays closed with its report);
    otherwise ours, as a conflict. Returns (item, names of conflicting fields)."""
    winner = None
    if rank and ours.get("state") != base.get("state") and theirs.get("state") != base.get("state"):
        ours_rank, theirs_rank = rank.get(ours.get("state"), 0), rank.get(theirs.get("state"), 0)
        if ours_rank != theirs_rank:
            winner = ours if ours_rank > theirs_rank else theirs
    merged, conflicts = {}, []
    for key in list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]:
        value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
        if conflict and winner is not None:
            value, conflict = winner.get(key, MISSING), False
        if conflict:
            conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def merge_set(base, ours, theirs):
    """A list merged as a set (`related`, SR-2.8): what either side added stays, what either side removed goes, in
    ours' order, then theirs' additions. MISSING when nothing is left."""
    as_list = lambda v: v if isinstance(v, list) else []
    base, ours, theirs = as_list(base), as_list(ours), as_list(theirs)
    removed = (set(base) - set(ours)) | (set(base) - set(theirs))
    merged = [v for v in ours if v not in removed]
    merged += [v for v in theirs if v not in removed and v not in merged]
    return merged or MISSING


def merge_task(base, ours, theirs, vocab):
    """Merge one task changed on both sides. Returns (task, names of conflicting fields)."""
    changed = lambda side: any(side.get(f, MISSING) != base.get(f, MISSING) for f in PROGRESS_FIELDS)
    if not changed(theirs):
        progress = ours
    elif not changed(ours):
        progress = theirs
    else:
        progress = progress_winner(ours, theirs, vocab)
    keys = list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]
    merged, conflicts = {}, []
    for key in keys:
        if key in PROGRESS_FIELDS:
            value = progress.get(key, MISSING)
        elif key == "notes":
            value = merge_notes(ours.get(key, []), theirs.get(key, []))
        elif key in ("blocks", "reviews"):
            value = merge_intervals(ours.get(key, []), theirs.get(key, []))
        elif key == "sprint_history":
            value = merge_history(ours.get(key, []), theirs.get(key, []))
        elif key == "related":
            value = merge_set(base.get(key, []), ours.get(key, []), theirs.get(key, []))
        else:
            value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
            if conflict and key == "phase" and ("sprint" in ours or "sprint" in theirs):
                # One side has been migrated to sprints (SR-8.5): its sprint stands for the phase
                value, conflict = MISSING, False
            if conflict:
                conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def next_free_id(task_id, used):
    parts = split_id(task_id)
    if not parts:
        n = 2
        while f"{task_id}-{n}" in used:
            n += 1
        return f"{task_id}-{n}"
    prefix, digits = parts
    numbers = [int(x[len(prefix) :]) for x in used if x.startswith(prefix) and x[len(prefix) :].isdigit()]
    return prefix + str(max(numbers) + 1).zfill(len(digits))


def merge_task_files(base, ours, theirs):
    """Merge three versions of tasks.json. Returns (merged data, list of conflict descriptions)."""
    by_id = lambda data: {t.get("id"): t for t in data.get("tasks", [])}
    vocab = merge_vocabulary(ours, theirs)
    base_tasks, our_tasks, their_tasks = by_id(base), by_id(ours), by_id(theirs)
    used = set(base_tasks) | set(our_tasks) | set(their_tasks)
    conflicts, merged_tasks, renamed, theirs_new = [], [], {}, []

    for task in ours.get("tasks", []):
        task_id = task.get("id")
        b, t = base_tasks.get(task_id), their_tasks.get(task_id)
        if b is None:
            merged_tasks.append(task)  # added here (if added there too, theirs is renumbered below)
        elif t is None:
            if task != b:
                conflicts.append(f"{task_id}: changed on this branch, deleted on the other")
                merged_tasks.append(task)
        else:
            merged, fields = merge_task(b, task, t, vocab)
            merged_tasks.append(merged)
            if fields:
                conflicts.append(f"{task_id}: both branches changed {', '.join(fields)}")

    for task in theirs.get("tasks", []):
        task_id = task.get("id")
        if task_id in base_tasks:
            if task_id not in our_tasks and task != base_tasks[task_id]:
                conflicts.append(f"{task_id}: deleted on this branch, changed on the other")
                merged_tasks.append(task)
            continue
        if task_id in our_tasks:
            if our_tasks[task_id] == task:
                continue  # the same task added on both sides
            new_id = next_free_id(task_id, used)
            used.add(new_id)
            renamed[task_id] = new_id
            task = {**task, "id": new_id}
        theirs_new.append(task)
    for task in theirs_new:
        if renamed and task.get("dependencies"):
            task = {**task, "dependencies": [renamed.get(d, d) for d in task["dependencies"]]}
        merged_tasks.append(task)

    result = {}
    for key in list(ours) + [k for k in theirs if k not in ours]:
        if key == "tasks":
            result[key] = merged_tasks
        elif key == "meta" and isinstance(ours.get(key), dict) and isinstance(theirs.get(key), dict):
            b, o, t = base.get(key) or {}, ours[key], theirs[key]
            meta = {}
            for k in list(o) + [k for k in t if k not in o]:
                if k in META_LISTS:
                    value, item_conflicts = merge_items(k, b.get(k, []), o.get(k, []), t.get(k, []))
                    conflicts.extend(item_conflicts)
                else:
                    value, _ = merge_value(b.get(k, MISSING), o.get(k, MISSING), t.get(k, MISSING))
                if value is not MISSING:
                    meta[k] = value
            if "sprints" in meta:
                meta.pop("phases", None)  # a project migrated on either side has no phases left (SR-8.5)
            result[key] = meta
        else:
            result[key] = ours.get(key, theirs.get(key))
    return result, conflicts


def merge_files(base_path, ours_path, theirs_path):
    """What git's merge driver does (`atm merge-driver %O %A %B`): merge the three versions task by task and write the
    result over `ours_path`. Returns the conflicts (this branch's version was kept for each)."""
    versions = [read_json(p) for p in (base_path, ours_path, theirs_path)]
    if versions[0] is None and os.path.exists(base_path) and os.path.getsize(base_path) == 0:
        versions[0] = {"tasks": []}  # the file was added on both branches
    if not all(isinstance(v, dict) and isinstance(v.get("tasks", []), list) for v in versions):
        raise AtmError("tasks.json merge: one side is not valid JSON; resolve the conflict by hand")
    merged, conflicts = merge_task_files(*versions)
    write_json_atomic(ours_path, merged)
    return conflicts


def launcher_command():
    launcher = os.path.join(SCRIPT_DIR, "atm").replace("'", "'\\''")
    return f"'{launcher}' merge-driver %O %A %B"


def ensure_merge_driver(paths):
    """In git: `tasks.json merge=atm` in the task folder's .gitattributes (staged when added), and the driver
    registered in the clone's config, pointing at this plugin's launcher. Returns True if .gitattributes changed."""
    root = paths["root"]
    if not in_git(root):
        return False
    for key, value in (("merge.atm.name", "atm: merge tasks.json task by task"), ("merge.atm.driver", launcher_command())):
        if git(["config", "--get", key], root) != value:
            git(["config", key, value], root)
    attributes = os.path.join(paths["dir"], ".gitattributes")
    try:
        with open(attributes, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    if "tasks.json merge=atm" in text.splitlines():
        return False
    os.makedirs(paths["dir"], exist_ok=True)
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if not text or text.endswith("\n") else "\n") + GITATTRIBUTES_LINES)
    git(["add", "--", rel(root, attributes)], root)
    return True


ARCHIVE_ATTRIBUTES_LINE = "tasks-archive.json merge=atm\n"


def ensure_archive_merge(paths):
    """In git: the archive merges task by task too. Added when a project first archives, so projects that never do
    see no change. Returns True if .gitattributes changed (staged)."""
    root = paths["root"]
    if not in_git(root):
        return False
    ensure_merge_driver(paths)  # the driver itself, and the tasks.json line
    attributes = os.path.join(paths["dir"], ".gitattributes")
    with open(attributes, encoding="utf-8") as f:
        text = f.read()
    if ARCHIVE_ATTRIBUTES_LINE.strip() in text.splitlines():
        return False
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if text.endswith("\n") else "\n") + ARCHIVE_ATTRIBUTES_LINE)
    git(["add", "--", rel(root, attributes)], root)
    return True
