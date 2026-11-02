"""A store's working copy for one change (docs/layered-architecture.md §3.3): what atm-lib reads and changes inside a
store's `change(fn)`, and the record of what it changed.

A store runs `fn(work)` under its own isolation (the JSON-in-git store: its lock), writes what changed and only that,
and raises what `fn` deferred after writing. `changes()` lists each task and claim `fn` changed, before and after: a
store that keeps entities apart (a database) can turn the list into conditional writes, without atm-lib's rules
changing. Until the last of the code from before the layers has moved, the copy is the whole task document.
"""

import json


def _stamp(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False)


class Working:
    def __init__(self, document, claims_store, here, size, before, archive=None):
        """`here` and `size` may be callables (worked out when asked); `before()` gives the document and the claims
        store as they were, from wherever the store keeps them (the JSON-in-git store: its files, not written until
        the change ends), so recording what changed costs nothing unless `changes()` is asked for. `archive()` loads
        the archive document, only when a change asks for it."""
        self.document = document
        self.claims_store = claims_store
        self._here = here
        self._size = size
        self._before = before
        self._load_archive = archive
        self._archive = None
        self._before_archive = None
        self._deferred = None
        self._before_store = _stamp(claims_store)

    def archive(self):
        """The archive document ({"meta", "tasks"}), to read and change in place; loaded on first use."""
        if self._archive is None:
            self._archive = self._load_archive() if self._load_archive else {"meta": {}, "tasks": []}
            self._before_archive = _stamp(self._archive)
        return self._archive

    def archive_changed(self):
        """Whether the archive was asked for and changed: the store writes it then, and only then."""
        return self._archive is not None and _stamp(self._archive) != self._before_archive

    @staticmethod
    def _entities(document, claims_store):
        tasks = {t.get("id"): t for t in document["tasks"] if isinstance(t, dict)}
        return {"task": tasks, "claim": dict(claims_store.get("claims") or {})}

    def tasks(self):
        """The working tasks, to read and change in place."""
        return self.document["tasks"]

    def claims(self):
        """The claims store: `claims`, `completed`, `waiting`, `resolved`, `hints`, to read and change in place."""
        return self.claims_store

    def here(self):
        """The working copy this change runs in: `worktree` (its folder, None for a store without them) and `branch`."""
        if callable(self._here):
            self._here = self._here()
        return self._here

    def size(self):
        """How big the task data is, in bytes, as it was read for this change (the archive hint)."""
        if callable(self._size):
            self._size = self._size()
        return self._size

    def defer(self, error):
        """Raise `error` once the change is written: a lease that expired is written as expired, then reported."""
        self._deferred = error

    def raise_deferred(self):
        if self._deferred is not None:
            raise self._deferred

    def changes(self):
        """What changed: [{"entity": "task" | "claim", "id", "before", "after"}], `before` or `after` None for an entity
        created or removed. The rest of the claims store (completed, waiting, resolved, hints) is in `store_changed`."""
        before = self._entities(*self._before())
        after = self._entities(self.document, self.claims_store)
        found = []
        for entity in ("task", "claim"):
            old, new = before[entity], after[entity]
            for key in list(old) + [k for k in new if k not in old]:
                if old.get(key) != new.get(key):
                    found.append({"entity": entity, "id": key, "before": old.get(key), "after": new.get(key)})
        return found

    def store_changed(self):
        return _stamp(self.claims_store) != self._before_store
