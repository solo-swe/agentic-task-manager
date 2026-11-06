"""How `atm project config` prints where the task files are. The commands about the JSON-in-git store itself (`atm db
migrate`, `db merge-driver`, the lock test; docs/layered-architecture.md §3.2, §11.2) reach it only through
`boot/jsongit.py`: a store that isn't files in git has none of them."""

from ..common.core import out, to_json


def show_paths(paths, as_json):
    """Print where the task files are (`atm project config`)."""
    if as_json:
        out(to_json(paths))
        return
    for key in ["root", "dir", "tasks", "dashboard"]:
        out(f"{key}: {paths[key]}")
    if paths["legacy"]:
        out(f"legacy: {paths['legacy']['tasks']}")
