"""The plugin's dashboard file (plugin/templates/tasks-dashboard.html) and the version and build tags at its top: atm
serve serves it, and restarts on a newer one."""

import os
import re

from ..common.core import DASHBOARD_FILE, PLUGIN_ROOT

TEMPLATE = os.path.join(PLUGIN_ROOT, "templates", DASHBOARD_FILE)
VERSION_META = re.compile(r'<meta name="atm-dashboard-version" content="([^"]*)"')
BUILD_META = re.compile(r'<meta name="atm-dashboard-build" content="([^"]*)"')
# The tags sit at the top of the page; reading this much finds them without reading the whole (inlined) dashboard
HEAD_BYTES = 16384


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
