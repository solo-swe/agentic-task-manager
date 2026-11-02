"""atm-lib: the project's settings in meta (docs/layered-architecture.md §3.1, §5.1): the keys `init --meta-file`
sets, the project's id, the repository's web address and the logo. Each change is one change of the store (§3.3)."""

import os

from ..common.core import now
from ..common.errors import Invalid
from ..common.model import vocabulary
from ..common.repository import move_pr_links, repository_url


def _meta(data):
    if not isinstance(data.get("meta"), dict):
        data["meta"] = {}
    return data["meta"]


def init_meta(project, new):
    """`init --meta-file`: merge `new`'s keys into meta (project, description, vocabularies, ...), creating the task
    data when there is none, so /atm:create-tasks never writes the file itself. The vocabularies are checked before
    anything is written; meta.id is kept. Returns (created, the keys set)."""
    new = {k: v for k, v in new.items() if k != "id"}
    store = project.store
    if not store.exists():
        data = {"meta": {"created": now().strftime("%Y-%m-%d"), **new}, "tasks": []}
        vocabulary(data)  # raises, naming the problem, before anything is written
        store.create(data)
        return True, sorted(new)

    def change(work):
        _meta(work.document).update(new)
        vocabulary(work.document)

    store.change(change)
    return False, sorted(new)


def ensure_project_id(project):
    """Give the project a meta.id if it has none: the dashboard keeps each project's saved view under it (projects
    served from the same port share browser storage). Returns the new id, or None if there was one already."""
    if not project.store.exists():
        return None

    def change(work):
        meta = _meta(work.document)
        if isinstance(meta.get("id"), str) and meta["id"]:
            return None
        meta["id"] = os.urandom(4).hex()
        return meta["id"]

    return project.store.change(change)


def ensure_repository(project):
    """Fill meta.repository from the version control's remote when there is none. Returns the address set, or None."""
    if not project.store.exists() or project.vcs is None:
        return None
    url = repository_url(project.vcs.remote_url() or "")
    if not url:
        return None

    def change(work):
        meta = _meta(work.document)
        if meta.get("repository"):
            return None
        meta["repository"] = url
        return url

    return project.store.change(change)


def get_repository(project):
    """meta.repository, or None."""
    return (project.store.document().get("meta") or {}).get("repository") or None


def set_repository(project, url, previous=None):
    """Set meta.repository to `url` (an http(s) URL or a git remote). When the repository moved (from `previous`, else
    the address set before), the tasks' PR links move with it, in the working tasks and the archive (T-273). Returns
    {repository, moved_links, previous}."""
    address = repository_url(url)
    if not address:
        raise Invalid(f"{url!r} is not a repository address: give an http(s) URL or a git remote")
    old = None
    if previous is not None:
        old = repository_url(previous)
        if not old:
            raise Invalid(f"--from {previous!r} is not a repository address: give an http(s) URL or a git remote")

    def change(work):
        meta = _meta(work.document)
        before = old or repository_url(meta.get("repository") or "")
        meta["repository"] = address
        moved = 0
        if before and before != address:
            moved = move_pr_links(work.document.get("tasks") or [], before, address)
            moved += move_pr_links(work.archive()["tasks"], before, address)
        return {"repository": address, "moved_links": moved, "previous": before}

    return project.store.change(change)


def get_logo(project):
    """The project's logo: {logo: its path in the project, source: "set" or "found", file, type} or None (store's
    `logo`), and `set`, what meta.logo says."""
    meta = project.store.document().get("meta") or {}
    return {"found": project.store.logo(meta), "set": meta.get("logo")}


def set_logo(project, relative):
    """Set meta.logo to the image at `relative` (a path in the project); the store refuses one it can't serve."""
    project.store.check_logo(relative)

    def change(work):
        _meta(work.document)["logo"] = relative

    project.store.change(change)
    return relative


def unset_logo(project):
    """Remove meta.logo. Returns what it was, or None when none was set."""
    return project.store.change(lambda work: _meta(work.document).pop("logo", None))
