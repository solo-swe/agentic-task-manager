"""`atm component`: the project's components (list, add, edit)."""

import os
from typing import Annotated, Optional

import typer

from ...boot.open import open_project
from ...common.core import out, to_json
from ...lib.components import add_component, edit_component, list_components
from ..app import app


# How `atm component add` and `component edit` describe what they moved (docs/layered-architecture.md §9, step 10).


def moved_text(label, from_label, assign, by_files):
    """What --from-label and --assign did, e.g. "6 tasks from the label atm-cli, 2 more by their files"."""
    parts = []
    if label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {label}")
    if assign:
        parts.append(f"{len(by_files)} {'more ' if label else ''}by their files")
    return ", ".join(parts)


component_app = typer.Typer(help="Component management")
app.add_typer(component_app, name="component")


@component_app.command("list")
def component_list(
    ctx: typer.Context,
) -> None:
    """List components with their task counts."""
    project = open_project(os.getcwd())
    listed = list_components(project)
    if ctx.obj.get("json_out", False):
        out(to_json(listed))
        return
    if not listed:
        out("No components yet: atm component add <id> --path GLOB.")
        return
    for c in listed:
        details = [f"“{c['label']}”"] if c.get("label") else []
        details.append(", ".join(c["paths"]) if c["paths"] else "no paths")
        if c.get("review"):
            details.append(f"review {c['review']}")
        out(f"{c['id']} · {' · '.join(details)}: {c['tasks']} task{'' if c['tasks'] == 1 else 's'}")


@component_app.command("add")
def component_add(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown"),
    path: Optional[list[str]] = typer.Option(None, "--path", metavar="GLOB", help="Add: a glob over its files, e.g. 'dashboard/**' (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Add a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label:
            parts.append(f"--label {label}")
        if path:
            for p in path:
                parts.append(f"--path {p}")
        if review:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would add component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_component(project, component_id, label, path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    text = moved_text(from_label, result["from_label"], assign, result["by_files"])
    out(f"Added the component {component_id}" + (f": {text}." if text else "."))


@component_app.command("edit")
def component_edit(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown (add: default the label's own text, with --from-label; edit: empty removes it)"),
    add_path: Optional[list[str]] = typer.Option(None, "--add-path", metavar="GLOB", help="Edit: add a glob over its files (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed (blocker, async, batch, none; edit: empty removes it)"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Edit a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label is not None:
            parts.append(f"--label {label}")
        if add_path:
            for p in add_path:
                parts.append(f"--add-path {p}")
        if review is not None:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would edit component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_component(project, component_id, label, add_path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    moved = moved_text(from_label, result["from_label"], assign, result["by_files"])
    text = ", ".join(([f"changed {', '.join(result['changed'])}"] if result["changed"] else []) + [t for t in [moved] if t])
    out(f"{component_id}: {text or 'nothing changed'}")
