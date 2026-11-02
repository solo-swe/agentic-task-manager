"""`atm show`: tasks in full, through atm-lib (docs/layered-architecture.md §9, step 1)."""

import os

from ..boot.open import open_project
from ..core import out, to_json
from ..lib.tasks import show_tasks
from .render import render_task


def cmd_show(args):
    views = show_tasks(open_project(os.getcwd()), args.ids)
    if args.json:
        # An archived task says so (T-353)
        out(to_json([{**v["task"], "archived": True} if v["archived"] else v["task"] for v in views]))
    else:
        out("\n\n".join(render_task(v) for v in views))
