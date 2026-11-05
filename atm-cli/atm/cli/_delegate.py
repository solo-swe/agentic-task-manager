"""Delegate to the `atm` CLI via subprocess.

`atm2` is a Typer-based CLI entry point that reuses the existing `atm` command
implementation. Each `atm2` subcommand builds an argument list for `atm <cmd>`
and runs it as a subprocess, passing through stdout, stderr, and the exit code.

This lets us evaluate Typer's ergonomics (nested groups, type-hinted options,
auto-help) without duplicating business logic or touching the `atm` command
surface.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys


def _repo_root() -> str:
    """The repository root: three levels up from atm-cli/atm/cli/."""
    here = os.path.abspath(__file__)
    # atm-cli/atm/cli/_delegate.py -> atm-cli/atm/cli -> atm-cli/atm -> atm-cli -> repo
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(here))))


def find_atm_cmd() -> list[str]:
    """Return the argv list for invoking `atm`.

    Prefers the `atm` executable on PATH (respects user's installed plugin).
    Falls back to running plugin/scripts/atm.py directly via the current Python.
    """
    atm_on_path = shutil.which("atm")
    if atm_on_path:
        return [atm_on_path]
    atm_py = os.path.join(_repo_root(), "plugin", "scripts", "atm.py")
    if os.path.exists(atm_py):
        return [sys.executable, atm_py]
    raise RuntimeError("could not find the `atm` command (not on PATH, and plugin/scripts/atm.py missing)")


def run_atm(args: list[str]) -> int:
    """Run `atm <args>` and passthrough stdout/stderr to the terminal.

    Returns `atm`'s exit code.
    """
    cmd = find_atm_cmd() + args
    result = subprocess.run(cmd, text=True)
    # Propagate stdout/stderr (subprocess.run with no capture already inherits
    # the parent's stdout/stderr by default).
    return result.returncode


def run_atm_capture(args: list[str]) -> tuple[int, str, str]:
    """Run `atm <args>` and capture stdout/stderr.

    Returns (exit_code, stdout, stderr).
    """
    cmd = find_atm_cmd() + args
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.returncode, result.stdout, result.stderr
