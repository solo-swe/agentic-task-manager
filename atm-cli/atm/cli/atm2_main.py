"""atm2: Typer-based CLI entry point for `atm`.

Run with: python3 -m atm.cli.atm2  or  ./atm-cli/atm2
"""

from __future__ import annotations

import os
import sys

# Ensure the package parent is on sys.path so relative imports work
# when run as a script directly (./atm-cli/atm2).
_ATM_CLI = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# The third-party code atm-cli uses (transitions) from atm-cli/vendored/, the copy the build bundles, as atm-lib's unit
# tests take it (tests/lib/support.py), and Typer and Click from plugin/scripts/vendor/, the copy the plugin ships:
# nothing to install. six is one module in a folder of its name.
_VENDORED = os.path.join(_ATM_CLI, "vendored")
_SHIPPED = os.path.join(os.path.dirname(_ATM_CLI), "plugin", "scripts", "vendor")
sys.path[:0] = [_SHIPPED, _ATM_CLI, _VENDORED, os.path.join(_VENDORED, "six")]

from atm.cli.atm2 import command
from atm.common.core import AtmError, AtmExit, to_json
from atm.common.errors import AtmFailure


def report(error: AtmFailure, as_json: bool) -> None:
    """An error as atm reports it: `atm: <message>` on stderr, or {"error", "code"} with --json."""
    if as_json:
        sys.stderr.write(to_json({"error": str(error), "code": error.code}) + "\n")
    else:
        sys.stderr.write(f"atm: {error}\n")


def main() -> None:
    as_json = "--json" in sys.argv or "-j" in sys.argv
    try:
        command()(prog_name="atm2")
    except AtmFailure as e:
        report(e, as_json)
        sys.exit(e.code)
    except AtmExit as e:
        sys.exit(e.code)
    except Exception as e:  # noqa: BLE001 - an error message, not a traceback, as atm does
        report(
            AtmError(f"unexpected error ({type(e).__name__}: {e}); run atm check, and report it if that is clean"),
            as_json,
        )
        sys.exit(1)


if __name__ == "__main__":
    main()
