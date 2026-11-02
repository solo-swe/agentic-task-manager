"""atm-lib: the archive (docs/v4/data-model.md §5; docs/layered-architecture.md §5.1): finished tasks moved out of the
working tasks into the archive, so the working file stays small, and brought back.

A move is one store change that tells the store it moves tasks into the archive (`work.moving_into_archive()`): the
JSON-in-git store writes the archive first, then tasks.json; a restore the other way round. A task found in both
keeps its working copy and loses the archive's: an archive that crashed halfway simply hasn't happened, an unarchive
that crashed halfway is complete, and no task is ever lost.
"""

from datetime import timedelta

from ..common.errors import Invalid, NotFound
from ..core import now, now_iso, parse_iso
from ..model import archivable, archive_hint_for, archive_settings, find_sprint, finished_at, kind_of, vocabulary
from .edit import next_number


def drop_duplicates(data, archive):
    """Tasks in both files keep their tasks.json copy (see the module docstring). Returns the dropped IDs."""
    ids = {t.get("id") for t in data["tasks"]}
    dropped = [t.get("id") for t in archive["tasks"] if t.get("id") in ids]
    archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
    return dropped


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


def parse_before(before):
    """--before as a time: a date (2026-05-01) or a full ISO time; None without one."""
    if not before:
        return None
    try:
        return parse_iso(before if "T" in before else before + "T00:00:00Z")
    except ValueError:
        raise Invalid(f"--before {before}: use a date like 2026-05-01")


def plan_archive(data, archive, sprints, before, keep_days):
    """What an archive would do: (moving, dropped, kept, names). Changes `archive` (drops its duplicates)."""
    dropped = drop_duplicates(data, archive)
    moving = archivable(data, before, keep_days)
    names = None
    if sprints:
        vocab = vocabulary(data)
        names = set()
        for name in sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise NotFound(f"no sprint {name!r}", 2)
            names.add(sprint["id"])
        moving = [t for t in moving if (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names]
    return moving, dropped, kept_finished(data, moving, before, keep_days, names)


def archive_finished(project, sprints=None, before=None, keep_days=None, dry_run=False):
    """`atm archive`: move the finished tasks (DM-5.1) to the archive, those of `sprints` only and finished before
    `before` if given, keeping those finished in the last `keep_days`. With `dry_run`, only what it would do. Returns
    {moved, dry_run, duplicates_dropped, kept, sizes}."""
    before = parse_before(before)
    if dry_run:
        # Nothing changes: worked out on copies of what the store reads
        data = {"meta": project.store.document().get("meta"), "tasks": list(project.store.list_tasks())}
        archive = {"meta": {}, "tasks": list(project.store.list_archived())}
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        return {"moved": ids, "dry_run": True, "duplicates_dropped": dropped, "kept": kept, "sizes": project.store.sizes()}

    def change(work):
        data, archive = work.document, work.archive()
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        if moving or dropped:
            work.moving_into_archive()
            # New IDs must never come back to an archived ID (DM-2.3)
            if not isinstance(data.get("meta"), dict):
                data["meta"] = {}
            data["meta"]["next_id"] = next_number(data)
            archive.setdefault("meta", {})["archived_at"] = now_iso()
            archive["tasks"].extend(moving)
            moved = set(ids)
            data["tasks"] = [t for t in data["tasks"] if t.get("id") not in moved]
            # The dashboard shows the count without loading the archive (dashboard D-5.4)
            data["meta"]["archived"] = len(archive["tasks"])
        return {"moved": ids, "dry_run": False, "duplicates_dropped": dropped, "kept": kept}

    result = project.store.change(change)
    return {**result, "sizes": project.store.sizes()}


def unarchive_tasks(project, ids):
    """`atm unarchive`: bring archived tasks back. Returns {restored, moves_back}: those the next archive would move
    straight back (finished and old, T-357)."""

    def change(work):
        data, archive = work.document, work.archive()
        drop_duplicates(data, archive)
        by_id = {t.get("id"): t for t in archive["tasks"]}
        missing = [i for i in ids if i not in by_id]
        if missing:
            # Already back, or never there at all: two different mistakes (T-357)
            live = {t.get("id") for t in data["tasks"]}
            here = [i for i in missing if i in live]
            unknown = [i for i in missing if i not in live]
            parts = ([f"{', '.join(here)} (already in tasks.json)"] if here else []) + (
                [f"{', '.join(unknown)} (no such task)"] if unknown else []
            )
            raise NotFound(f"not in the archive: {'; '.join(parts)}")
        data["tasks"].extend(by_id[i] for i in ids)
        # Still finished and old: the next atm archive would move it straight back (T-357)
        again = [t.get("id") for t in archivable(data, None, None) if t.get("id") in ids]
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        data["meta"]["archived"] = len(archive["tasks"]) - len(set(ids))
        archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
        return {"restored": list(ids), "moves_back": again}

    return project.store.change(change)


def archive_hint_now(project):
    """The suggestion to archive (CLI-5.1), or None: when the working tasks are over the threshold."""
    return archive_hint_for(project.store.document(), project.store.sizes()["tasks"])
