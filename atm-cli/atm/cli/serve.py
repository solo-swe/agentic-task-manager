"""`atm serve`: the CLI's side of the dashboard server, one per user, a detached process serving every registered project
(docs/multi-project-api.md). The CLI starts, stops, checks and refreshes the server as a process (its record in the
user's atm folder, its pid); the server itself is the REST API's, run by `plugin/scripts/atm-serve` (the built
atm.py entered as the server). The two never import each other (docs/layered-architecture.md §11.1)."""

import os
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone

from ..boot.jsongit import dashboard_id, migrate_folder
from ..boot.open import open_project
from ..common.core import (
    PLUGIN_ROOT,
    atm_home,
    ensure_home,
    format_duration,
    out,
    parse_iso,
    parse_version,
    pid_alive,
    read_json,
    to_json,
    whoami,
)
from ..common.dashboard import SERVER_LOG, SERVER_RECORD, TEMPLATE, dashboard_head, server_command
from ..common.errors import AtmFailure


def server_url(port, project=None):
    return f"http://localhost:{port}/" + (f"#project={project}" if project else "")


def live_server(home):
    """The record of the user's running server, or None. A stale record is removed."""
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        return None
    info = whoami(record.get("port"))
    if info and info.get("pid") == record.get("pid"):
        return record
    os.remove(record_path)
    return None


def open_browser(url):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.Popen([opener, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        out(f"Could not open a browser; open {url} yourself.")


def ensure_server(home, port):
    """The running server's record, starting the server process if none runs. Returns (record, started)."""
    record = live_server(home)
    if record:
        return record, False
    ensure_home(home)
    with open(os.path.join(home, SERVER_LOG), "ab") as log:
        child = subprocess.Popen(
            server_command(home, port),
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
    deadline = time.time() + 10
    while time.time() < deadline:
        record = read_json(os.path.join(home, SERVER_RECORD))
        if record and record.get("pid") == child.pid and whoami(record["port"]):
            return record, True
        if child.poll() is not None:
            break
        time.sleep(0.05)
    raise AtmFailure(f"the dashboard server did not start; see {os.path.join(home, SERVER_LOG)}")


def serve_start(cwd, port, overview):
    """Start (or reuse) the server; returns the URL to show: the project's (at or above `cwd`), or the overview's."""
    project = None
    if not overview:
        migration = migrate_folder(cwd, register_it=False)
        for line in migration["lines"]:
            if line != "Nothing to migrate.":
                out(line)
        if not migration["tasks_exists"]:
            raise AtmFailure("no tasks.json to serve. Run /atm:create-tasks first.")
        project, shown = dashboard_id(open_project(cwd))
        if not shown:
            out("This project isn't listed on the dashboard (meta.dashboard.listed is false).")
            project = None
    home = atm_home()
    record, started = ensure_server(home, port)
    url = server_url(record["port"], project)
    out(f"Dashboard server {'started' if started else 'already running'}: {url}")
    return url


def stop_server(home, pid):
    """Stop the user's server (whose pid whoami confirmed) and remove its record."""
    os.kill(pid, signal.SIGTERM)
    deadline = time.time() + 5
    while time.time() < deadline and pid_alive(pid):
        time.sleep(0.05)
    record_path = os.path.join(home, SERVER_RECORD)
    if os.path.exists(record_path):
        os.remove(record_path)


def serve_stop(home):
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        out("Dashboard server is not running.")
        return
    info = whoami(record.get("port"))
    if not (info and info.get("pid") == record.get("pid")):
        os.remove(record_path)
        out(f"Dashboard server is not running (removed a stale record for pid {record.get('pid')}).")
        return
    stop_server(home, record["pid"])
    out(f"Dashboard server stopped (pid {record['pid']}).")


def outdated(record):
    """True when the server runs an older dashboard than this plugin's: an older version, or the same version but
    another build of this plugin's own dashboard file (it was rebuilt since the server started). A newer server is
    left alone, so a session still on an older plugin never downgrades it. Another install of the same version (a
    development checkout, a worktree, the plugin cache) is left alone too: neither build is newer, and replacing it
    would make each install restart the server in turn, every open page offering a reload each time."""
    mine, mine_build = dashboard_head(TEMPLATE) or ("", "")
    theirs, theirs_build = str(record.get("version") or ""), str(record.get("build") or "")
    if parse_version(mine) != parse_version(theirs):
        return parse_version(mine) > parse_version(theirs)
    theirs_root = record.get("plugin_root")
    if theirs_root and os.path.realpath(str(theirs_root)) != os.path.realpath(PLUGIN_ROOT):
        return False
    return bool(mine_build) and mine_build != theirs_build


def refresh_server(home=None):
    """Restart the user's server on the same port if it runs an older dashboard than this plugin's (after a plugin
    update). Cheap when there is nothing to do: one read of server.json, and no request unless it is outdated.
    Never starts a server that isn't running. Returns the new record, or None."""
    home = home or atm_home()
    record = read_json(os.path.join(home, SERVER_RECORD))
    if not isinstance(record, dict) or not outdated(record):
        return None
    live = live_server(home)
    if not live:
        return None
    stop_server(home, live["pid"])
    return ensure_server(home, live["port"])[0]


def serve_refresh(home):
    record = refresh_server(home)
    if record:
        out(f"Dashboard server restarted with dashboard {record.get('version')}: {server_url(record['port'])}")


def serve_status(home, as_json):
    record = live_server(home)
    if as_json:
        out(to_json({"running": bool(record), **(record or {}), "url": server_url(record["port"]) if record else None}))
        return
    if not record:
        out("Dashboard server is not running.")
        return
    up = format_duration((datetime.now(timezone.utc) - parse_iso(record["started_at"])).total_seconds())
    out(
        f"Dashboard server running: {server_url(record['port'])} "
        f"(pid {record['pid']}, dashboard {record.get('version')}, up {up})"
    )


def cmd_serve(args):
    # --dir acts on another project without a cd (which permission rules can't check)
    cwd = os.path.abspath(args.dir) if args.dir else os.getcwd()
    if not os.path.isdir(cwd):
        raise AtmFailure(f"--dir {args.dir}: not a directory", 2)
    home = atm_home()
    if args.refresh:
        serve_refresh(home)
    elif args.action == "start":
        refresh_server(home)  # an outdated server is replaced before its URL is shown
        url = serve_start(cwd, args.port, args.all)
        if args.open:
            open_browser(url)
    elif args.action == "stop":
        serve_stop(home)
    else:
        serve_status(home, args.json)
