"""What the CLI and the REST API's server both know about the dashboard server (docs/layered-architecture.md §11.1):
the plugin's dashboard file (plugin/templates/tasks-dashboard.html) and the version and build tags at its top, the
server's record and log in the user's atm folder, and the command that starts the server process. The CLI starts,
stops and refreshes the server as a process; the server itself is the REST API's (`api/server.py`)."""

import hashlib
import os
import re
import sys

from .core import DASHBOARD_FILE, PLUGIN_ROOT, SCRIPT_DIR

TEMPLATE = os.path.join(PLUGIN_ROOT, "templates", DASHBOARD_FILE)
VERSION_META = re.compile(r'<meta name="atm-dashboard-version" content="([^"]*)"')
BUILD_META = re.compile(r'<meta name="atm-dashboard-build" content="([^"]*)"')
# The tags sit at the top of the page; reading this much finds them without reading the whole (inlined) dashboard
HEAD_BYTES = 16384

# In the user's atm folder (ATM_HOME): the running server's {pid, port, version, build, plugin_root, home, started_at}
SERVER_RECORD = "server.json"
SERVER_LOG = "server.log"
DEFAULT_PORT = 8765
# How many ports from the first one `atm serve status` and `stop` look at for a server the record doesn't name (T-390)
SCAN_PORTS = 20
# The argument the built atm.py takes to run the server instead of the CLI (plugin/scripts/atm-serve passes it)
SERVER_ENTRY = "--server"


def home_id(home):
    """A short ID of an atm folder: its real path, hashed. The server gives it in whoami and its record, so the CLI can
    tell its own folder's servers from another folder's (T-390) without the path going over HTTP."""
    return hashlib.sha256(os.path.realpath(home).encode("utf-8")).hexdigest()[:16]


def page_tags(head):
    """(version, build ID) from the start of a dashboard page (text), "" for a missing tag."""
    version, build = VERSION_META.search(head), BUILD_META.search(head)
    return (version.group(1) if version else "", build.group(1) if build else "")


def dashboard_head(p):
    """(version, build ID) of a dashboard file, "" for a missing tag; None if there is no file."""
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            head = f.read(HEAD_BYTES)
    except OSError:
        return None
    return page_tags(head)


def server_command(home, port):
    """The command that runs the server process for the atm folder `home`, from the first free port from `port`: this
    plugin's atm.py, with this Python, entered as the server (what plugin/scripts/atm-serve runs)."""
    return [sys.executable, os.path.join(SCRIPT_DIR, "atm.py"), SERVER_ENTRY, home, str(port)]
