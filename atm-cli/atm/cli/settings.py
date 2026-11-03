"""`atm init`, `atm repository` and `atm logo`: the project's settings, through atm-lib. Setting the store itself up
(the merge driver, the dashboard's list) is boot's, the one place that knows the store."""

import os

from ..boot.jsongit import register_project, set_up_store
from ..boot.open import open_project
from ..common.core import out, read_json, to_json
from ..common.errors import Invalid
from ..lib.settings import (
    ensure_project_id,
    ensure_repository,
    get_logo,
    get_repository,
    init_meta,
    set_logo,
    set_repository,
    unset_logo,
)


def cmd_init(args):
    project = open_project(os.getcwd())
    if args.meta_file:
        new = read_json(args.meta_file)
        if not isinstance(new, dict):
            raise Invalid(f"{args.meta_file} must hold a JSON object: the meta keys to set")
        created, keys = init_meta(project, new)
        project = open_project(os.getcwd())  # the task data as it is now
        if not args.json:
            out(("Created tasks.json; meta: " if created else "Updated tasks.json meta: ") + (", ".join(keys) or "-"))
    project_id = ensure_project_id(project)
    repository = ensure_repository(project)
    register_project(project)  # the dashboard lists every project atm runs in
    setup = set_up_store(project)
    if args.json:
        out(
            to_json(
                {
                    "project_id": project_id,
                    "repository": repository,
                    "git": setup["git"],
                    "gitattributes_added": setup["gitattributes_added"],
                }
            )
        )
        return
    if project_id:
        out(f"Gave this project an id ({project_id}) in tasks.json meta, for the dashboard's saved view.")
    if repository:
        out(f"Recorded the repository address ({repository}), for commit and PR links on the dashboard.")
    if not setup["git"]:
        out("Not in a git repository: there is no merge driver to set up.")
        return
    out(
        "Registered the tasks.json merge driver (git config merge.atm)"
        + (
            " and added .agentic-task-manager/.gitattributes (staged)."
            if setup["gitattributes_added"]
            else "; .gitattributes already set."
        )
    )


def cmd_repository(args):
    project = open_project(os.getcwd())
    if args.action is None:
        if args.url is not None or args.old is not None:
            raise Invalid(f"unexpected argument '{args.url or args.old}' (atm repository set URL)")
        current = get_repository(project)
        if args.json:
            out(to_json({"repository": current}))
        elif current:
            out(current)
        else:
            out("No repository address yet. Set it with `atm repository set <url>` (a git remote works too).")
        return
    result = set_repository(project, args.url, args.old)
    moved = result["moved_links"]
    if args.json:
        out(to_json({"repository": result["repository"], "moved_links": moved}))
    else:
        links = f" ({moved} PR link{'' if moved == 1 else 's'} moved from {result['previous']})" if moved else ""
        out(f"Repository address set: {result['repository']}{links}")


def cmd_logo(args):
    project = open_project(os.getcwd())
    if args.action is None:
        if args.path is not None:
            raise Invalid(f"unexpected argument '{args.path}' (atm logo set PATH)")
        info = get_logo(project)
        logo, chosen = info["found"], info["set"]
        if args.json:
            found = {"logo": logo["path"], "source": logo["source"]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": chosen}))
        elif logo:
            out(logo["path"] + (" (found; set one with `atm logo set <path>`)" if logo["source"] == "found" else ""))
        elif chosen:
            out(f"meta.logo is {chosen}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if args.action == "unset":
        removed = unset_logo(project)
        out(to_json({"logo": None}) if args.json else "Logo unset." if removed else "No logo was set.")
        return
    if args.path is None:
        raise Invalid("atm logo set needs a path")
    # The path is relative to where atm runs, like every path atm takes; it is kept relative to the project root
    root = project.store.working_copy()
    relative = os.path.relpath(os.path.abspath(args.path), os.path.realpath(root)).replace(os.sep, "/")
    set_logo(project, relative)
    out(to_json({"logo": relative}) if args.json else f"Logo set: {relative}")
