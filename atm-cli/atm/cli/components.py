"""`atm components`, through atm-lib (docs/layered-architecture.md §9, step 10)."""

import os

from ..boot.open import open_project
from ..common.errors import Invalid
from ..common.core import out, to_json
from ..lib.components import add_component, edit_component, list_components


def moved_text(label, from_label, assign, by_files):
    """What --from-label and --assign did, e.g. "6 tasks from the label atm-cli, 2 more by their files"."""
    parts = []
    if label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {label}")
    if assign:
        parts.append(f"{len(by_files)} {'more ' if label else ''}by their files")
    return ", ".join(parts)


def show_list(project, as_json):
    listed = list_components(project)
    if as_json:
        out(to_json(listed))
        return
    if not listed:
        out("No components yet: atm components add <id> --path GLOB.")
    for c in listed:
        details = [f"“{c['label']}”"] if c.get("label") else []
        details.append(", ".join(c["paths"]) if c["paths"] else "no paths")
        if c.get("review"):
            details.append(f"review {c['review']}")
        out(f"{c['id']} · {' · '.join(details)}: {c['tasks']} task{'' if c['tasks'] == 1 else 's'}")


def cmd_components(args):
    project = open_project(os.getcwd())
    if args.action in (None, "list"):
        if args.id is not None:
            raise Invalid(f"unexpected argument '{args.id}'")
        for option in ("label", "path", "add_path", "review", "from_label"):
            if getattr(args, option) is not None:
                raise Invalid(f"--{option.replace('_', '-')} is for atm components add or edit")
        if args.assign:
            raise Invalid("--assign is for atm components add or edit")
        show_list(project, args.json)
        return
    if not args.id:
        raise Invalid(f"atm components {args.action} needs the component's ID")
    if args.action == "add":
        if args.add_path is not None:
            raise Invalid("--add-path is for atm components edit (add takes --path)")
        result = add_component(project, args.id, args.label, args.path, args.review, args.from_label, args.assign)
        if args.json:
            out(to_json(result))
            return
        text = moved_text(args.from_label, result["from_label"], args.assign, result["by_files"])
        out(f"Added the component {args.id}" + (f": {text}." if text else "."))
        return
    if args.path is not None:
        raise Invalid("--path is for atm components add (edit takes --add-path)")
    result = edit_component(project, args.id, args.label, args.add_path, args.review, args.from_label, args.assign)
    if args.json:
        out(to_json(result))
        return
    moved = moved_text(args.from_label, result["from_label"], args.assign, result["by_files"])
    text = ", ".join(([f"changed {', '.join(result['changed'])}"] if result["changed"] else []) + [t for t in [moved] if t])
    out(f"{args.id}: {text or 'nothing changed'}")
