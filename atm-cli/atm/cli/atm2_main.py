"""atm2: Typer-based CLI entry point for `atm`.

Run with: python3 -m atm.cli.atm2  or  ./atm-cli/atm2
"""

from __future__ import annotations

import os
import sys

# Ensure the package parent is on sys.path so relative imports work
# when run as a script directly (./atm-cli/atm2).
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from atm.cli.atm2 import app


def main() -> None:
    app(prog_name="atm2")


if __name__ == "__main__":
    main()
