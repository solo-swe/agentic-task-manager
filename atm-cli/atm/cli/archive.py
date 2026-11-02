"""`atm archive` and `atm unarchive`, through atm-lib (docs/layered-architecture.md §9, step 12)."""

import os

from ..boot.open import open_project
from ..common.core import out, to_json
from ..lib.archive import archive_finished, unarchive_tasks


def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())


def cmd_archive(args):
    result = archive_finished(open_project(os.getcwd()), args.sprint, args.before, args.keep_days, args.dry_run)
    ids, dropped, kept, sizes = result["moved"], result["duplicates_dropped"], result["kept"], result["sizes"]
    shown = {
        "moved": ids,
        "dry_run": bool(args.dry_run),
        "tasks_kb": sizes["tasks"] // 1024,
        "archive_kb": sizes["archive"] // 1024,
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if args.json:
        out(to_json(shown))
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
            f"tasks.json is now {shown['tasks_kb']} KB, tasks-archive.json {shown['archive_kb']} KB."
        )
    # What stays, and why (T-352)
    if kept:
        out(f"{'Would keep' if args.dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}.")


def cmd_unarchive(args):
    result = unarchive_tasks(open_project(os.getcwd()), args.ids)
    again = result["moves_back"]
    if args.json:
        out(to_json(result))
        return
    out(f"Brought back {', '.join(args.ids)} from the archive.")
    if again:
        out(
            f"{describe_ids(again)} {'is' if len(again) == 1 else 'are'} finished: the next `atm archive` moves "
            f"{'it' if len(again) == 1 else 'them'} back, unless reopened (`atm reopen <id> --reason …`)."
        )
