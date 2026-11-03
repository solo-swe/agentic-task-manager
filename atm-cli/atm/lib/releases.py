"""atm-lib: releases (docs/sprints-and-releases-design.md §3.3, SR-2.2; docs/layered-architecture.md §5.1): list them
with their tasks by kind and stage, add one (with PRs, giving it their tasks, archived ones too), edit and archive one,
and draft its notes. A task's release is its own, else its sprint's. Each write is one store change; the functions
take keyword parameters (T-383)."""

from ..common.errors import Invalid, NotFound
from ..common.vocabularies import KINDS
from ..common.core import parse_iso
from ..common.model import DATE, RELEASE_ID, effective_release, find_release, kind_of, vocabulary

STAGES = ("done", "in_pr", "merged")


def stage_of(vocab, task):
    """A done task's stage (DM-4.3): merged once its PR merged, in_pr while its PR is open, else done; None when it
    isn't done."""
    if kind_of(vocab, task.get("status")) != "done":
        return None
    pr = task.get("pr") if isinstance(task.get("pr"), dict) else None
    if pr and task.get("merged_at"):
        return "merged"
    return "in_pr" if pr else "done"


def release_overview(data, archived, include_archived):
    """The releases in order, each with its tasks (by effective release, both files) by kind and by stage."""
    vocab = vocabulary(data)
    counts = {r["id"]: ({k: 0 for k in KINDS}, {s: 0 for s in STAGES}) for r in vocab["releases"]}
    for task in list(data["tasks"]) + list(archived):
        if not isinstance(task, dict):
            continue
        release = effective_release(vocab, task)
        if release not in counts:
            continue
        by_kind, by_stage = counts[release]
        kind = kind_of(vocab, task.get("status"))
        if kind in by_kind:
            by_kind[kind] += 1
        stage = stage_of(vocab, task)
        if stage:
            by_stage[stage] += 1
    return [
        {
            **r,
            "tasks": sum(counts[r["id"]][0].values()),
            "by_kind": {k: n for k, n in counts[r["id"]][0].items() if n},
            "by_stage": {s: n for s, n in counts[r["id"]][1].items() if n},
        }
        for r in vocab["releases"]
        if include_archived or r["state"] != "archived"
    ]


def list_releases(project, include_archived=False):
    """The releases in order with their tasks by kind and stage. Returns {releases, has_releases}."""
    data = project.store.document()
    return {
        "releases": release_overview(data, project.store.list_archived(), include_archived),
        "has_releases": bool(vocabulary(data)["releases"]),
    }


def check_target(target):
    if target and not DATE.fullmatch(target):
        raise Invalid(f"--target {target!r}: use a date, YYYY-MM-DD")
    return target


def release_fields(release, name, target):
    """name and target on a release (None leaves one as it is); an empty value removes the field. Returns the names
    changed."""
    changed = []
    for key, value in (("name", name), ("target", check_target(target))):
        if value is None:
            continue
        if value == "":
            if key in release:
                del release[key]
                changed.append(key)
        elif release.get(key) != value:
            release[key] = value
            changed.append(key)
    return changed


def unknown_release(vocab, release_id):
    known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm releases add <R>"
    return NotFound(f"no release {release_id!r}; the releases: {known}", 2)


def stored_release(data, release_id):
    """The release as stored in meta.releases (to change it), or an error naming the releases."""
    vocab = vocabulary(data)
    if find_release(vocab, release_id) is None:
        raise unknown_release(vocab, release_id)
    return next(r for r in data["meta"]["releases"] if r.get("id") == release_id)


def add_release(project, release_id, name=None, target=None, prs=(), released_at=None):
    """A planned release; with `prs` (pull request numbers) every task of those PRs, in both files, goes into it;
    with `released_at` it shipped already. Returns {release, tasks}: the tasks given to it."""
    if not RELEASE_ID.fullmatch(release_id):
        raise Invalid(f"{release_id!r}: a release ID is letters, digits, '.', '-' and '_' (at most 64)")
    prs = list(prs or [])
    if any(n <= 0 for n in prs):
        raise Invalid("--pr needs a pull request number")
    if released_at is not None:
        try:
            parse_iso(released_at)
        except ValueError:
            raise Invalid(f"--released-at needs a time like 2026-10-24T18:00:00Z, not {released_at!r}")

    def change(work):
        data, archive_data = work.document, work.archive()
        vocab = vocabulary(data)
        if find_release(vocab, release_id):
            raise Invalid(f"there is already a release {release_id!r}")
        release = {"id": release_id, "state": "planned"}
        release_fields(release, name, target)
        given = []
        # A release recovered from pull requests (SR-8.2): their tasks, in both files, go into it; the release records
        # the last PR given (the one that shipped it)
        for number in prs:
            own = []
            for task in list(data["tasks"]) + list(archive_data["tasks"]):
                pr = task.get("pr") if isinstance(task, dict) and isinstance(task.get("pr"), dict) else None
                if pr and pr.get("number") == number:
                    task["release"] = release_id
                    own.append(task["id"])
                    if isinstance(pr.get("url"), str):
                        release["pr"] = {"number": number, "url": pr["url"]}
            if not own:
                raise Invalid(f"no task is in pull request #{number}")
            given += own
        if released_at is not None:
            # One that shipped already: released at that time (its PR's merge, for one recovered from PRs)
            release["state"] = "released"
            release["released_at"] = released_at
        meta = data.setdefault("meta", {})
        meta["releases"] = list(meta.get("releases") or []) + [release]
        vocabulary(data)
        return {"release": release, "tasks": given}

    return project.store.change(change)


def edit_release(project, release_id, name=None, target=None):
    """Change a release's name or target (None leaves it, an empty value removes it). Returns {release, changed}."""

    def change(work):
        release = stored_release(work.document, release_id)
        changed = release_fields(release, name, target)
        if changed:
            vocabulary(work.document)
        return {"release": release, "changed": changed}

    return project.store.change(change)


def archive_release(project, release_id):
    """Archive a release: it leaves the open releases (`atm releases --all` shows it). Returns the release."""

    def change(work):
        release = stored_release(work.document, release_id)
        if release.get("state") == "archived":
            raise Invalid(f"the release {release['id']!r} is already archived")
        release["state"] = "archived"
        return release

    return project.store.change(change)


def outcome_of(task):
    """The task's last outcome note (what `atm done --note` said), or None."""
    notes = [n for n in task.get("notes") or [] if isinstance(n, dict) and n.get("kind") == "outcome"]
    return notes[-1].get("text") if notes else None


def release_notes(project, release_id):
    """A draft of the release's changes (§3.3): its done tasks (by effective release, both files) grouped by type in
    the project's order, each with its ID, name and outcome note; and the tasks in it not done yet. Returns
    {release, groups, open}."""
    data = project.store.document()
    vocab = vocabulary(data)
    release = find_release(vocab, release_id)
    if release is None:
        raise unknown_release(vocab, release_id)
    tasks = [t for t in list(data["tasks"]) + project.store.list_archived() if isinstance(t, dict)]
    own = [t for t in tasks if effective_release(vocab, t) == release_id]
    done = [t for t in own if kind_of(vocab, t.get("status")) == "done"]
    types = [(t["id"], t["label"]) for t in vocab["types"]]
    types += [(t, t) for t in dict.fromkeys(x.get("type") for x in done) if t not in dict(types)]
    groups = []
    for type_id, label in types:
        items = [{"id": t.get("id"), "name": t.get("name"), "outcome": outcome_of(t)} for t in done if t.get("type") == type_id]
        if items:
            groups.append({"type": type_id, "label": label, "tasks": items})
    open_tasks = [
        {"id": t.get("id"), "name": t.get("name"), "status": t.get("status")}
        for t in own
        if kind_of(vocab, t.get("status")) not in ("done", "skipped")
    ]
    return {"release": release, "groups": groups, "open": open_tasks}
