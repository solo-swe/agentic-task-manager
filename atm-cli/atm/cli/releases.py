"""`atm releases`, through atm-lib (docs/layered-architecture.md §9, step 11)."""

import os

from ..boot.open import open_project
from ..common.errors import Invalid
from ..common.core import out, to_json
from ..lib.releases import add_release, archive_release, edit_release, list_releases, release_notes


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


def cmd_releases(args):
    project = open_project(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise Invalid(f"unexpected argument '{args.id}'")
        listed = list_releases(project, args.all)
        if args.json:
            out(to_json(listed["releases"]))
            return
        if not listed["releases"]:
            out(
                "No open releases (--all lists the archived ones)."
                if listed["has_releases"]
                else "No releases yet: atm releases add <R>."
            )
        for r in listed["releases"]:
            out(describe_release(r))
        return
    if not args.id:
        raise Invalid(f"atm releases {args.action} needs a release ID")
    if args.pr is not None and args.action != "add":
        raise Invalid("--pr is for atm releases add")
    if args.released_at is not None and args.action != "add":
        raise Invalid("--released-at is for atm releases add")
    if args.action == "add":
        result = add_release(
            project, args.id, name=args.title, target=args.target, prs=args.pr or [], released_at=args.released_at
        )
        if args.json:
            out(to_json(result))
            return
        release, given, prs = result["release"], result["tasks"], args.pr or []
        line = f"Added the release “{args.id}” ({release['state']})"
        if given:
            numbers = ", ".join(f"#{n}" for n in prs)
            line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
        out(line + ".")
    elif args.action == "edit":
        result = edit_release(project, args.id, name=args.title, target=args.target)
        release, changed = result["release"], result["changed"]
        if args.json:
            out(to_json(release))
        else:
            out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")
    elif args.action == "archive":
        release = archive_release(project, args.id)
        out(to_json(release) if args.json else f"Archived the release “{release['id']}”.")
    else:
        notes = release_notes(project, args.id)
        if args.json:
            out(to_json(notes))
            return
        for line in describe_notes(notes):
            out(line)
