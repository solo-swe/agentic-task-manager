"""The `atm` command itself: the CLI, with the REST API's server commands (`atm serve`, the server process, the
refresh after a plugin update) handed to it. The CLI and the REST API are siblings on atm-lib and never import each
other (docs/layered-architecture.md §1); this is the one place that puts them together."""

import sys

from ..api.server import DEFAULT_PORT, cmd_serve, cmd_serve_child, refresh_server
from ..cli.main import Serving, main


def atm(argv):
    return main(argv, Serving(cmd_serve, cmd_serve_child, refresh_server, DEFAULT_PORT))


if __name__ == "__main__":
    sys.exit(atm(sys.argv[1:]))
