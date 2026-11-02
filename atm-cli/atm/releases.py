"""Releases (docs/sprints-and-releases-design.md §3.3, SR-2.2): `atm releases` lists them with their tasks by kind
and stage, adds one (with --pr, giving it the tasks of a pull request), edits and archives one. A task's release is
its own, else its sprint's. Every write is under the lock."""

import os

from .core import AtmError, out, parse_iso, to_json, write_json_atomic
from .lock import locked
from .common.vocabularies import KINDS
from .model import (
    DATE,
    RELEASE_ID,
    archive_path,
    effective_release,
    find_release,
    kind_of,
    load_archive,
    load_tasks,
    vocabulary,
)
from .paths import find_paths

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


def describe_release(r):
    details = [r["state"]] + ([f"target {r['target']}"] if r.get("target") else [])
    if r.get("released_at"):
        details.append(f"released {r['released_at'][:10]}")
    if isinstance(r.get("pr"), dict) and r["pr"].get("number"):
        details.append(f"PR #{r['pr']['number']}")
    title = r["id"] + (f" “{r['name']}”" if r.get("name") else "")
    kinds = ", ".join(f"{k} {n}" for k, n in r["by_kind"].items())
    stages = ", ".join(f"{s.replace('_', ' ')} {n}" for s, n in r["by_stage"].items())
    line = f"{title} · {' · '.join(details)}: {r['tasks']} task{'' if r['tasks'] == 1 else 's'}"
    return line + (f" ({kinds})" if kinds else "") + (f"; {stages}" if stages else "")


def releases_list(paths, include_archived, as_json):
    data = load_tasks(paths)
    releases = release_overview(data, load_archive(paths)["tasks"], include_archived)
    if as_json:
        out(to_json(releases))
        return
    if not releases:
        has_any = bool(vocabulary(data)["releases"])
        out("No open releases (--all lists the archived ones)." if has_any else "No releases yet: atm releases add <R>.")
    for r in releases:
        out(describe_release(r))


def check_target(target):
    if target and not DATE.fullmatch(target):
        raise AtmError(f"--target {target!r}: use a date, YYYY-MM-DD", 2)
    return target


def release_fields(release, args):
    """--name and --target on a release; an empty value removes the field. Returns the names changed."""
    changed = []
    for key, value in (("name", args.title), ("target", check_target(args.target))):
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


def stored_release(data, release_id):
    """The release as stored in meta.releases (to change it), or an error naming the releases."""
    vocab = vocabulary(data)
    if find_release(vocab, release_id) is None:
        known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm releases add <R>"
        raise AtmError(f"no release {release_id!r}; the releases: {known}", 2)
    return next(r for r in data["meta"]["releases"] if r.get("id") == release_id)


def releases_add(paths, args):
    release_id = args.id
    if not RELEASE_ID.fullmatch(release_id):
        raise AtmError(f"{release_id!r}: a release ID is letters, digits, '.', '-' and '_' (at most 64)", 2)
    prs = args.pr or []
    if any(n <= 0 for n in prs):
        raise AtmError("--pr needs a pull request number", 2)
    if args.released_at is not None:
        try:
            parse_iso(args.released_at)
        except ValueError:
            raise AtmError(f"--released-at needs a time like 2026-10-24T18:00:00Z, not {args.released_at!r}", 2)
    archive = archive_path(paths)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive_data = load_archive(paths)
        vocab = vocabulary(data)
        if find_release(vocab, release_id):
            raise AtmError(f"there is already a release {release_id!r}", 2)
        release = {"id": release_id, "state": "planned"}
        release_fields(release, args)
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
                raise AtmError(f"no task is in pull request #{number}", 2)
            given += own
        if args.released_at is not None:
            # One that shipped already: released at that time (its PR's merge, for one recovered from PRs)
            release["state"] = "released"
            release["released_at"] = args.released_at
        meta = data.setdefault("meta", {})
        meta["releases"] = list(meta.get("releases") or []) + [release]
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
        if any(t.get("id") in given for t in archive_data["tasks"] if isinstance(t, dict)):
            write_json_atomic(archive, archive_data)
    if args.json:
        out(to_json({"release": release, "tasks": given}))
        return
    line = f"Added the release “{release_id}” ({release['state']})"
    if given:
        numbers = ", ".join(f"#{n}" for n in prs)
        line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
    out(line + ".")


def releases_edit(paths, args):
    with locked(paths["dir"]):
        data = load_tasks(paths)
        release = stored_release(data, args.id)
        changed = release_fields(release, args)
        if changed:
            vocabulary(data)
            write_json_atomic(paths["tasks"], data)
    if args.json:
        out(to_json(release))
    else:
        out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")


def releases_archive(paths, args):
    """Archive a release: it leaves `atm releases` and the dashboard's open releases (--all shows it)."""
    with locked(paths["dir"]):
        data = load_tasks(paths)
        release = stored_release(data, args.id)
        if release.get("state") == "archived":
            raise AtmError(f"the release {release['id']!r} is already archived", 2)
        release["state"] = "archived"
        write_json_atomic(paths["tasks"], data)
    out(to_json(release) if args.json else f"Archived the release “{release['id']}”.")


def outcome_of(task):
    """The task's last outcome note (what `atm done --note` said), or None."""
    notes = [n for n in task.get("notes") or [] if isinstance(n, dict) and n.get("kind") == "outcome"]
    return notes[-1].get("text") if notes else None


def release_notes(data, archived, release_id):
    """A draft of the release's changes (§3.3): its done tasks (by effective release, both files) grouped by type in
    the project's order, each with its ID, name and outcome note; and the tasks in it not done yet."""
    vocab = vocabulary(data)
    release = find_release(vocab, release_id)
    if release is None:
        known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm releases add <R>"
        raise AtmError(f"no release {release_id!r}; the releases: {known}", 2)
    tasks = [t for t in list(data["tasks"]) + list(archived) if isinstance(t, dict)]
    own = [t for t in tasks if effective_release(vocab, t) == release_id]
    done = [t for t in own if kind_of(vocab, t.get("status")) == "done"]
    types = [(t["id"], t["label"]) for t in vocab["types"]]
    types += [(t, t) for t in dict.fromkeys(x.get("type") for x in done) if t not in dict(types)]
    groups = []
    for type_id, label in types:
        items = [
            {"id": t.get("id"), "name": t.get("name"), "outcome": outcome_of(t)}
            for t in done
            if t.get("type") == type_id
        ]
        if items:
            groups.append({"type": type_id, "label": label, "tasks": items})
    open_tasks = [
        {"id": t.get("id"), "name": t.get("name"), "status": t.get("status")}
        for t in own
        if kind_of(vocab, t.get("status")) not in ("done", "skipped")
    ]
    return {"release": release, "groups": groups, "open": open_tasks}


def describe_notes(notes):
    """The draft as Markdown: a heading per type, a line per task; what's not done yet at the end."""
    release = notes["release"]
    lines = [f"# {release['id']}" + (f" “{release['name']}”" if release.get("name") else "")]
    if not notes["groups"]:
        lines += ["", "Nothing done in it yet."]
    for group in notes["groups"]:
        lines += ["", f"## {group['label']}", ""]
        for t in group["tasks"]:
            lines.append(f"- **{t['id']}** {t['name']}" + (f": {t['outcome']}" if t["outcome"] else ""))
    if notes["open"]:
        lines += ["", "## Not done yet", ""]
        lines += [f"- {t['id']} {t['name']} ({t['status']})" for t in notes["open"]]
    return lines


def releases_notes(paths, args):
    notes = release_notes(load_tasks(paths), load_archive(paths)["tasks"], args.id)
    if args.json:
        out(to_json(notes))
        return
    for line in describe_notes(notes):
        out(line)


def cmd_releases(args):
    paths = find_paths(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise AtmError(f"unexpected argument '{args.id}'", 2)
        releases_list(paths, args.all, args.json)
        return
    if not args.id:
        raise AtmError(f"atm releases {args.action} needs a release ID", 2)
    if args.pr is not None and args.action != "add":
        raise AtmError("--pr is for atm releases add", 2)
    if args.released_at is not None and args.action != "add":
        raise AtmError("--released-at is for atm releases add", 2)
    actions = {"add": releases_add, "edit": releases_edit, "archive": releases_archive, "notes": releases_notes}
    actions[args.action](paths, args)
