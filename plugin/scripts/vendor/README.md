# Vendored libraries

atm's command tree (`atm-cli/atm/cli/main.py`) is built with Typer, on Click. Both are copied here, unchanged except as
noted, so `atm` needs nothing but Python 3.9+; `atm.py` puts this folder first on its import path.

| Package | Version | License |
|---|---|---|
| click | 8.1.8 | BSD-3-Clause (`click/LICENSE.txt`) |
| typer | 0.15.4 | MIT (`typer/LICENSE`) |

The versions are those in `atm-cli/uv.lock` (`tests/scripts/vendor.test.mjs` checks they match). Copied from the
uv environment (`uv sync` in `atm-cli/`). Left out of typer: `rich_utils.py` (no rich: help and errors are plain
Click output on every machine, even where rich is installed), `cli.py`, `testing.py` and `__main__.py`. Changed:
`typer/_completion_classes.py` `_sanitize_help_text` returns the text as it is (it imported `rich_utils`).

A stopgap until the Rust port (T-415) replaces atm-cli: update by hand, if at all.
