"""The archive: finished tasks moved out of tasks.json into tasks-archive.json, so the working file stays small
(docs/v4/data-model.md §5). Both files stay readable JSON in the same format, merged by the same driver.

A move writes one file, then the other. A task found in both files keeps its tasks.json copy and loses the
archive's: an archive that crashed halfway simply hasn't happened, an unarchive that crashed halfway is complete, and
no task is ever lost.
"""

import os
from datetime import timedelta

from .core import AtmError, now, now_iso, out, parse_iso, to_json, write_json_atomic
from .edit import next_number
from .lock import locked
from .merge import ensure_archive_merge
from .model import (
    archivable,
    archive_path,
    archive_settings,
    find_sprint,
    finished_at,
    kind_of,
    load_archive,
    load_tasks,
    vocabulary,
)
from .paths import find_paths


def drop_duplicates(data, archive):
    """Tasks in both files keep their tasks.json copy (see the module docstring). Returns the dropped IDs."""
    ids = {t.get("id") for t in data["tasks"]}
    dropped = [t.get("id") for t in archive["tasks"] if t.get("id") in ids]
    archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
    return dropped


def size_kb(p):
    return os.path.getsize(p) // 1024 if os.path.isfile(p) else 0


def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def kept_finished(data, moving, before, keep, sprints):
    """The finished tasks that stay in tasks.json, and why (T-352), as archivable() decides: in an open PR, finished
    in the last keep days, not before --before, not in the --sprint chosen. [{id, reason}], in file order."""
    vocab = vocabulary(data)
    days = archive_settings(data)["keep_days"] if keep is None else keep
    recent = now() - timedelta(days=days)
    moved = {t.get("id") for t in moving}
    kept = []
    for t in data["tasks"]:
        if not isinstance(t, dict) or t.get("id") in moved:
            continue
        if kind_of(vocab, t.get("status")) not in ("done", "skipped"):
            continue
        finished = finished_at(t)
        if isinstance(t.get("pr"), dict) and not t.get("merged_at"):
            reason = f"in open PR #{t['pr'].get('number')}"
        elif finished is not None and finished > recent:
            reason = f"finished in the last {days} days"
        elif before is not None and (finished is None or finished >= before):
            reason = f"finished on or after {before.date().isoformat()}"
        elif sprints is not None:
            reason = "not in the sprints chosen"
        else:
            continue
        kept.append({"id": t.get("id"), "reason": reason})
    return kept


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())


def cmd_archive(args):
    before = None
    if args.before:
        try:
            before = parse_iso(args.before if "T" in args.before else args.before + "T00:00:00Z")
        except ValueError:
            raise AtmError(f"--before {args.before}: use a date like 2026-05-01", 2)
    paths = find_paths(os.getcwd())
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive = load_archive(paths)
        dropped = drop_duplicates(data, archive)
        moving = archivable(data, before, args.keep_days)
        names = None
        if args.sprint:
            vocab = vocabulary(data)
            names = set()
            for name in args.sprint:
                sprint = find_sprint(vocab, name)
                if sprint is None:
                    raise AtmError(f"no sprint {name!r}", 2)
                names.add(sprint["id"])
            moving = [t for t in moving if (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names]
        ids = [t.get("id") for t in moving]
        kept = kept_finished(data, moving, before, args.keep_days, names)
        if not args.dry_run and (moving or dropped):
            # New IDs must never come back to an archived ID (DM-2.3)
            if not isinstance(data.get("meta"), dict):
                data["meta"] = {}
            data["meta"]["next_id"] = next_number(data)
            archive.setdefault("meta", {})["archived_at"] = now_iso()
            archive["tasks"].extend(moving)
            write_json_atomic(archive_path(paths), archive)
            moved = set(ids)
            data["tasks"] = [t for t in data["tasks"] if t.get("id") not in moved]
            # The dashboard shows the count without loading the archive (dashboard D-5.4)
            data["meta"]["archived"] = len(archive["tasks"])
            write_json_atomic(paths["tasks"], data)
            ensure_archive_merge(paths)
    result = {
        "moved": ids,
        "dry_run": bool(args.dry_run),
        "tasks_kb": size_kb(paths["tasks"]),
        "archive_kb": size_kb(archive_path(paths)),
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if args.json:
        out(to_json(result))
        return
    if dropped and not args.dry_run:
        out(f"Kept {describe_ids(dropped)} in tasks.json and dropped the archive's copy (an interrupted move).")
    if not ids:
        out("Nothing to archive.")
    elif args.dry_run:
        out(f"Would archive {len(ids)} task(s): {describe_ids(ids)}.")
    else:
        out(
            f"Archived {len(ids)} task(s): {describe_ids(ids)}. "
            f"tasks.json is now {result['tasks_kb']} KB, tasks-archive.json {result['archive_kb']} KB."
        )
    # What stays, and why (T-352)
    if kept:
        out(f"{'Would keep' if args.dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}.")


def cmd_unarchive(args):
    paths = find_paths(os.getcwd())
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive = load_archive(paths)
        drop_duplicates(data, archive)
        by_id = {t.get("id"): t for t in archive["tasks"]}
        missing = [i for i in args.ids if i not in by_id]
        if missing:
            raise AtmError(f"not in the archive: {', '.join(missing)}")
        # tasks.json first: a crash here leaves the task in both files, and it stays in tasks.json
        data["tasks"].extend(by_id[i] for i in args.ids)
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        data["meta"]["archived"] = len(archive["tasks"]) - len(set(args.ids))
        write_json_atomic(paths["tasks"], data)
        archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in args.ids]
        write_json_atomic(archive_path(paths), archive)
    if args.json:
        out(to_json({"restored": args.ids}))
    else:
        out(f"Brought back {', '.join(args.ids)} from the archive.")
