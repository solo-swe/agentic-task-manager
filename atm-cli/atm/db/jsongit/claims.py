"""The JSON-in-git store's claims store: which agent works on which task until when, what was finished, waits or was
decided in another worktree, and when hints were last shown (docs/parallel-agents-design.md). It lives in the git
common directory, so all worktrees of a clone see it; tasks.json itself is per worktree. Lock order: the task folder,
then the claims store, so two commands can never wait on each other. The rules on it are atm-lib's (lib/loop.py)."""

import contextlib
import os

from ...common.core import now_iso, read_json, write_json_atomic
from ...host.git import git
from .lock import locked

CLAIMS_STORE = "claims.json"  # in <git common dir>/atm/, shared by all worktrees of a clone
CLAIMS_STORE_NO_GIT = "claims-store.json"  # in the task folder, outside git
CLAIMS_SNAPSHOT = "claims.json"  # in the task folder (gitignored): what the dashboard reads


# The git common directory of each working copy, found once per process (it runs git; atm serve asks on every poll)
_common_dirs = {}


def claims_dir(paths):
    root = paths["root"]
    if root not in _common_dirs:
        _common_dirs[root] = git(["rev-parse", "--git-common-dir"], root)
    common = _common_dirs[root]
    if common:
        return os.path.join(os.path.normpath(os.path.join(root, common)), "atm")
    return paths["dir"]


def claims_store_path(paths):
    directory = claims_dir(paths)
    return os.path.join(directory, CLAIMS_STORE if directory != paths["dir"] else CLAIMS_STORE_NO_GIT)


@contextlib.contextmanager
def locked_with_claims(paths):
    """Hold the task folder's lock and, if it is elsewhere, the claims store's lock (always in this order)."""
    with contextlib.ExitStack() as stack:
        stack.enter_context(locked(paths["dir"]))
        if claims_dir(paths) != paths["dir"]:
            stack.enter_context(locked(claims_dir(paths)))
        yield


def read_store(paths):
    """The shared store: live `claims`, and `completed` tasks (done in some worktree; its tasks.json may be the only
    one that says so until the branches merge, so `next` must not hand those tasks out again elsewhere); `waiting`
    tasks (blocked or sent for review in some worktree, likewise); `resolved` ones (answered, passed, rejected or
    skipped in another worktree than the one they wait in, to be applied there); and `hints`, when each hint was last
    shown (so `done` repeats one at most once a day)."""
    data = read_json(claims_store_path(paths))
    data = data if isinstance(data, dict) else {}

    def section(key):
        return data.get(key) if isinstance(data.get(key), dict) else {}

    return {
        "claims": data.get("claims") or {},
        "completed": data.get("completed") or {},
        "waiting": section("waiting"),
        "resolved": section("resolved"),
        "hints": section("hints"),
    }


def write_store(paths, store):
    os.makedirs(claims_dir(paths), exist_ok=True)
    write_json_atomic(claims_store_path(paths), store)
    write_claims_snapshot(paths, store["claims"])


def write_claims_snapshot(paths, claims):
    """The gitignored copy the dashboard polls: live claims, as a list."""
    if os.path.isdir(paths["dir"]):
        snapshot = {"generated_at": now_iso(), "claims": sorted(claims.values(), key=lambda c: c["task"])}
        write_json_atomic(os.path.join(paths["dir"], CLAIMS_SNAPSHOT), snapshot)
