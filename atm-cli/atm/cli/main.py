"""The command line: atm's command tree, in groups (task, loop, claim, vcs, release, component, project, sprint; db,
doctor and service for maintenance), built with Typer on Click (vendored in plugin/scripts/vendor/), and main().
Design: docs/improving-atm-cli-ergonomics.md. The root of the tree is cli/app.py; each top-level command is a module
of cli/commands/ that adds itself to it. Here: shell completion for the whole tree, and the entry.

`--json` and `--dry-run` (`-n`) go anywhere on the line; `--dir PATH` first runs as if started in PATH. Errors are
`atm: <message>` on stderr, or {"error": "...", "code": N} with --json; a usage error has exit code 2.
"""

import os
import sys

import click
import typer

from ..boot.identity import agent_identity
from ..boot.jsongit import hold_lock_for_test
from ..boot.open import open_project
from ..common.core import AtmError, AtmExit, to_json
from ..common.errors import AtmFailure
from ..lib.agents import HARNESSES
from ..lib.claims import renew_due_claims
from ..lib.query import Filters, task_views
from ..lib.sprints import backlog_tasks
from .agent_docs import AGENT_DOCS
from .app import app
from .serve import refresh_server

# Each command module adds its commands to `app` as it is imported, in this order (the build's MODULES order)
from .commands.task import task_app  # noqa: F401
from .commands.loop import loop_app  # noqa: F401
from .commands.claim import claim_app  # noqa: F401
from .commands.vcs import vcs_app  # noqa: F401
from .commands.db import db_app  # noqa: F401
from .commands.release import release_app  # noqa: F401
from .commands.component import component_app  # noqa: F401
from .commands.project import project_app  # noqa: F401
from .commands.sprint import sprint_app  # noqa: F401
from .commands.maintenance import doctor  # noqa: F401
from .commands.agents import agents_app  # noqa: F401
from .commands.meta import version  # noqa: F401


# ---------------------------------------------------------------------------
# shell completion (T-423)
# ---------------------------------------------------------------------------
# Typer installs the scripts (--install-completion, --show-completion; zsh, bash, fish); these complete atm's own
# values, read through atm-lib in the folder the shell is in. Completion never fails loudly: outside a project, or
# with a task file atm can't read, it completes nothing.

# The targets of `atm task state` (task_state's cmd_map), with what each runs
STATE_TARGETS = [
    ("in_progress", "start"),
    ("in_review", "review"),
    ("done", "done"),
    ("todo", "back to to do: fail, release, drop (rejected), unblock or reopen"),
    ("blocked", "block"),
    ("skipped", "skip"),
]


def _vocabulary():
    return backlog_tasks(open_project(os.getcwd()))["vocab"]


def _task_ids():
    return [(v["task"]["id"], v["task"].get("name") or "") for v in task_views(open_project(os.getcwd()), Filters())]


def _sprints():
    return [(s["id"], s.get("goal") or s.get("state") or "") for s in _vocabulary()["sprints"] if s.get("state") != "closed"]


def _releases():
    return [(r["id"], r.get("name") or r.get("state") or "") for r in _vocabulary()["releases"]]


def _components():
    return [(c["id"], c.get("label") or "") for c in _vocabulary()["components"]]


def _completion_terms(key):
    return lambda: [(t["id"], t.get("label") or "") for t in _vocabulary()[key]]


def _completer(values):
    """A Click completer for `values()` ((value, help) pairs), matching what has been typed; nothing on any error."""

    def complete(ctx, param, incomplete):
        from click.shell_completion import CompletionItem

        try:
            pairs = values()
        except Exception:  # completion must never break the shell, whatever the project holds
            return []
        return [CompletionItem(value, help=help or None) for value, help in pairs if value.startswith(incomplete)]

    return complete


# (command path, parameter name) -> completer; a None path is any command with that parameter
COMPLETIONS = {
    (None, "task_id"): _task_ids,
    (None, "task_ids"): _task_ids,
    (None, "ids"): _task_ids,
    (None, "sprint"): _sprints,
    (None, "release"): _releases,
    (None, "component"): _components,
    (None, "status"): _completion_terms("statuses"),
    (None, "type_"): _completion_terms("types"),
    (None, "priority"): _completion_terms("priorities"),
    ("task state", "next_state"): lambda: STATE_TARGETS,
    ("db unarchive", "ids"): None,  # archived tasks, which the task list doesn't hold
    **{(f"sprint {c}", "name"): _sprints for c in ("edit", "start", "close", "delete")},
    ("sprint close", "carry_to"): _sprints,
    ("sprint delete", "move_to"): _sprints,
    ("sprint move", "to"): _sprints,
    **{(f"release {c}", "release_id"): _releases for c in ("edit", "archive", "notes")},
    ("component edit", "component_id"): _components,
    ("docs", "topic"): lambda: [(name, summary) for name, (summary, _) in AGENT_DOCS.items()],
    **{(f"agents {c}", "harness"): lambda: [(h, s["label"]) for h, s in HARNESSES.items()] for c in ("install", "uninstall")},
}


def _attach(cmd, path):
    for param in getattr(cmd, "params", []):
        key = (path, param.name) if (path, param.name) in COMPLETIONS else (None, param.name)
        values = COMPLETIONS.get(key)
        if values is not None:
            param._custom_shell_complete = _completer(values)
    for name, sub in getattr(cmd, "commands", {}).items():
        _attach(sub, f"{path} {name}".strip())


@app.command("_lock-test", hidden=True)
def lock_test(seconds: float = typer.Argument(0.0)) -> None:
    """Test hook: hold the task folder's lock for `seconds`, so tests can check exclusion."""
    hold_lock_for_test(os.getcwd(), seconds)


def command():
    """atm's Click command, with atm's values to complete."""
    cmd = typer.main.get_command(app)
    _attach(cmd, "")
    return cmd


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

# The options of the whole tree, which may go anywhere on the line: moved before the command, where Click reads them
GLOBAL_FLAGS = ("--json", "-j", "--dry-run", "-n")

# Commands that don't check for an outdated dashboard server first: service does it itself, git runs the merge
# driver in the middle of a merge (the server process isn't a CLI command: atm.py --server, plugin/scripts/atm-serve)
NO_REFRESH = (("service",), ("db", "merge-driver"), ("help",), ("docs",), ("version",), ("_lock-test",))
# Commands that leave the caller's claims alone: those that work on no project, and the heartbeat, which renews its own
NO_RENEW = NO_REFRESH + (("claim", "heartbeat"),)


def renew_own_claims(argv, path):
    """Every atm command an agent runs renews that agent's live claims, so they last while it works, in any harness,
    with no hook (T-439, docs/porting-to-rust.md §4). As `claim heartbeat --auto`: only claims whose last renewal is
    ten minutes old, so most commands only read. Not for a command run as someone else (`--agent`), and never in the
    way of the command itself."""
    if not path or any(path[: len(skip)] == skip for skip in NO_RENEW):
        return
    if any(a == "--agent" or a.startswith("--agent=") for a in argv[: argv.index("--") if "--" in argv else len(argv)]):
        return
    try:
        renew_due_claims(open_project(os.getcwd(), agent_identity()))
    except Exception:  # noqa: BLE001
        pass  # no project here, or one atm can't read: the command says so itself


def report(error, as_json):
    """An error as atm reports it: `atm: <message>` on stderr, or {"error", "code"} with --json."""
    if as_json:
        sys.stderr.write(to_json({"error": str(error), "code": error.code}) + "\n")
    else:
        sys.stderr.write(f"atm: {error}\n")


def leading_dir(argv):
    """`atm --dir PATH <command> ...` (or `--dir=PATH`, T-351): the folder to run in, and the rest. Only before the
    command: `atm service --dir PATH` keeps its own meaning."""
    if argv and argv[0] == "--dir":
        return (argv[1] if len(argv) > 1 else ""), argv[2:]
    if argv and argv[0].startswith("--dir="):
        return argv[0][len("--dir=") :], argv[1:]
    return None, argv


def global_first(argv):
    """argv with the tree's own options (GLOBAL_FLAGS) first, wherever they were given; what follows `--` (the command
    a program's, never atm's) is left as it is."""
    split = argv.index("--") if "--" in argv else len(argv)
    head, tail = argv[:split], argv[split:]
    flags = [a for a in head if a in GLOBAL_FLAGS]
    return flags + [a for a in head if a not in GLOBAL_FLAGS] + tail


def command_path(argv):
    """The command's names (`("task", "state")`), up to its first option or argument."""
    names = [a for a in argv if not a.startswith("-")]
    return tuple(names[:2])


def main(argv):
    target, argv = leading_dir(argv)
    as_json = any(a in ("--json", "-j") for a in argv[: argv.index("--") if "--" in argv else len(argv)])
    if target is not None:
        # As if started in that folder: the project is found from there, and relative paths count from there
        if not target or not os.path.isdir(target):
            report(AtmError(f"--dir: no folder '{target}'", 2), as_json)
            return 2
        os.chdir(target)
    argv = global_first(argv)
    path = command_path(argv)
    try:
        if path and not any(path[: len(skip)] == skip for skip in NO_REFRESH):
            try:
                refresh_server()  # after a plugin update, the first atm command brings the dashboard server along
            except Exception:
                pass  # never in the way of the command itself
        renew_own_claims(argv, path)
        code = command().main(args=argv, prog_name="atm", standalone_mode=False)
        return code if isinstance(code, int) else 0
    except click.UsageError as e:
        usage = e.ctx.get_usage() if e.ctx else ""
        report(AtmError(e.format_message() + (f"\n{usage}" if usage and not as_json else ""), 2), as_json)
        return 2
    except click.exceptions.Exit as e:
        return e.exit_code
    except click.Abort:
        return 1
    except AtmFailure as e:
        # atm-lib's errors and those of the modules from before the layers (AtmError is one of them)
        report(e, as_json)
        return e.code
    except AtmExit as e:
        return e.code
    except Exception as e:  # noqa: BLE001
        # A file atm didn't expect (a hostile or damaged one) is an error message, not a traceback: the skills and
        # the merge driver read stderr (docs/security-review.md F12)
        report(
            AtmError(f"unexpected error ({type(e).__name__}: {e}); run atm doctor, and report it if that is clean"),
            as_json,
        )
        return 1
