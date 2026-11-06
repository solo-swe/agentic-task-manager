"""How `atm db archive` and `db unarchive` describe the tasks they moved (docs/layered-architecture.md §9, step 12)."""


def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())
