"""`atm serve`: one dashboard server per user, a detached process serving every registered project.

The server holds the plugin's dashboard in memory and serves each registered project's task files under /p/<id>/.
Its routes and record are described in docs/multi-project-api.md.
"""

import base64
import errno
import hashlib
import os
import re
import signal
import socket
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone
from urllib.parse import parse_qs, urlsplit

from ..core import (
    PLUGIN_ROOT,
    TASK_DIR,
    TASKS_FILE,
    WHOAMI,
    format_duration,
    now,
    now_iso,
    out,
    parse_iso,
    parse_version,
    pid_alive,
    read_json,
    to_json,
    whoami,
    write_json_atomic,
)
from ..claims import claims_store_path
from ..common.errors import AtmFailure
from ..logo import find_logo
from ..migrate import HEAD_BYTES, TEMPLATE, dashboard_head, describe_migration, migrate, page_tags
from ..paths import find_paths
from ..boot.open import project_at
from ..lib.query import Filters, list_claims, project_summary, search_tasks, task_views
from ..registry import (
    REGISTRY_FILE,
    atm_home,
    ensure_home,
    listed,
    main_worktree,
    project_id,
    project_meta,
    register,
    worktrees,
)

SERVER_RECORD = "server.json"
SERVER_LOG = "server.log"
DEFAULT_PORT = 8765
PORT_TRIES = 100
PROJECT_FILES = ("tasks.json", "claims.json", "tasks-archive.json", "worktrees.json")
# A task file the server will read into memory and send (a big project's archive is a few MB)
MAX_SERVED_BYTES = 100_000_000
PROJECT_FILE = re.compile(r"/p/([^/]+)/(" + "|".join(re.escape(f) for f in PROJECT_FILES + ("logo",)) + r")")
# A logo is the project's own file, which could be an SVG with a script: it is shown as an image, never run
LOGO_HEADERS = {
    "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; sandbox",
    "X-Content-Type-Options": "nosniff",
}
# On every response: no content sniffing, and no other site may embed it (docs/security-review.md F11)
SECURITY_HEADERS = {"X-Content-Type-Options": "nosniff", "Cross-Origin-Resource-Policy": "same-origin"}
INLINE_SCRIPT = re.compile(rb"<script\b([^>]*)>(.*?)</script>", re.S | re.I)


def page_csp(page):
    """The dashboard's Content-Security-Policy: only its own inline scripts run (by hash), it talks only to this
    server, and no page may frame it. Any script injected into it, or a `javascript:` link, is refused by the browser.
    tests/e2e/serve.mjs builds the same policy, so the e2e suite runs under it."""
    hashes = [
        "'sha256-" + base64.b64encode(hashlib.sha256(body).digest()).decode("ascii") + "'"
        for attrs, body in INLINE_SCRIPT.findall(page)
        if b"src=" not in attrs
    ]
    return "; ".join(
        [
            "default-src 'none'",
            "script-src " + (" ".join(hashes) or "'none'"),
            "style-src 'unsafe-inline'",
            "img-src 'self' data:",
            "connect-src 'self'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ]
    )


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
    """The running server's record, starting the server if none runs. Returns (record, started)."""
    record = live_server(home)
    if record:
        return record, False
    ensure_home(home)
    with open(os.path.join(home, SERVER_LOG), "ab") as log:
        child = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "_serve-child", home, str(port)],
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


def project_for(paths):
    """(id, listed) of the project at `paths`, after registering it."""
    register(paths)
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"]) or {}
    # The id the registry gave it: meta.id, or another when that one is taken (registry.unique_id)
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE)) or {}
    entry = next((e for e in registry.get("projects", []) if isinstance(e, dict) and e.get("path") == path), None)
    return (entry or {}).get("id") or project_id(meta, path), listed(meta)


def serve_start(paths, port, overview):
    """Start (or reuse) the server; returns the URL to show: the project's, or the overview's."""
    project = None
    if not overview:
        for line in describe_migration(migrate(paths), paths):
            if line != "Nothing to migrate.":
                out(line)
        if not os.path.isfile(paths["tasks"]):
            raise AtmFailure("no tasks.json to serve. Run /atm:create-tasks first.")
        project, shown = project_for(paths)
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
        url = serve_start(find_paths(cwd), args.port, args.all)
        if args.open:
            open_browser(url)
    elif args.action == "stop":
        serve_stop(home)
    else:
        serve_status(home, args.json)


def port_in_use(port):
    """Something accepts connections on 127.0.0.1:port or [::1]:port (`localhost` may resolve to either)."""
    for family, host in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
        try:
            with socket.socket(family, socket.SOCK_STREAM) as s:
                s.settimeout(0.5)
                if s.connect_ex((host, port)) == 0:
                    return True
        except OSError:  # no IPv6 on this machine
            pass
    return False


def file_stamp(p):
    """(size, nanosecond mtime) of a file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


def file_etag(stamp):
    """A strong ETag from a file's size and nanosecond mtime. Last-Modified alone has one-second resolution, so two
    atm changes in the same second would look unchanged to the dashboard."""
    return f'"{stamp[0]:x}-{stamp[1]:x}"'


class Registry:
    """The registry as the server sees it, re-read when projects.json changes."""

    def __init__(self, home):
        self.path = os.path.join(home, REGISTRY_FILE)
        self.stamp = None
        self.projects = []

    def entries(self):
        stamp = file_stamp(self.path)
        if stamp != self.stamp:
            data = read_json(self.path)
            entries = data.get("projects") if isinstance(data, dict) else None
            self.projects = [e for e in entries or [] if isinstance(e, dict) and isinstance(e.get("path"), str)]
            self.stamp = stamp
        return self.projects

    def find(self, project):
        return next((e for e in self.entries() if e.get("id") == project), None)


def task_file(entry, name):
    return os.path.join(entry["path"], TASK_DIR, name)


def served_file(entry, name):
    """The project's task file `name` if it may be served: a regular file, not a symlink, in a task folder that isn't
    one either (docs/security-review.md F8: a repository could otherwise link tasks-archive.json to ~/.ssh/id_rsa).
    None otherwise."""
    folder = os.path.join(entry["path"], TASK_DIR)
    p = os.path.join(folder, name)
    if os.path.islink(folder) or os.path.islink(p) or not os.path.isfile(p):
        return None
    real_folder = os.path.realpath(folder)
    return p if os.path.dirname(os.path.realpath(p)) == real_folder else None


class Summaries:
    """Each project's summary (atm-lib's project_summary), computed again only when its tasks.json or claims store
    changed, or the day did (the completions count by day)."""

    def __init__(self):
        self.paths = {}  # project folder -> (atm paths, claims store path): finding them runs git
        self.cache = {}  # project folder -> (stamp, summary or None)

    def store(self, entry):
        """The project's claims store, shared by all its worktrees."""
        if entry["path"] not in self.paths:
            paths = find_paths(entry["path"])
            self.paths[entry["path"]] = (paths, claims_store_path(paths))
        return self.paths[entry["path"]][1]

    def project(self, entry):
        """The project, opened afresh for each request: atm-lib reads one snapshot of its files per call."""
        self.store(entry)
        return project_at(self.paths[entry["path"]][0])

    def stamp(self, entry):
        """What the summary depends on, or None when the project is missing: tasks.json, the claims store, the day,
        and .git/HEAD (the branch on the card)."""
        tasks = file_stamp(task_file(entry, TASKS_FILE))
        if tasks is None:
            return None
        head = file_stamp(os.path.join(entry["path"], ".git", "HEAD"))
        return (tasks, file_stamp(self.store(entry)), now().date().isoformat(), head)

    def summary(self, entry, stamp):
        cached = self.cache.get(entry["path"])
        if cached and cached[0] == stamp:
            return cached[1]
        try:
            summary = project_summary(self.project(entry))
        except AtmFailure:
            summary = None  # unreadable tasks.json or an unusable vocabulary: listed as missing
        self.cache[entry["path"]] = (stamp, summary)
        return summary


def project_logo(entry):
    """(absolute path, content type) of a registered project's logo (meta.logo, or one found), or None."""
    logo = find_logo(entry["path"], project_meta(entry["path"]) or {})
    return (logo[1], logo[2]) if logo else None


def quietly(fn, default):
    """fn(), or `default` if it fails: one project's broken files never break the list of all projects (F18)."""
    try:
        return fn()
    except Exception:  # noqa: BLE001 - any failure in one project's files
        return default


def api_projects(registry, summaries):
    """(body, ETag) of /api/projects. The ETag changes with the registry, and with each project's tasks.json, claims,
    logo and the day. A project whose files can't be read is listed as missing; the others are unaffected."""
    entries = registry.entries()
    stamps = [quietly(lambda e=e: summaries.stamp(e), None) for e in entries]
    trees = [quietly(lambda e=e: worktrees(e["path"]), []) if stamp else [] for e, stamp in zip(entries, stamps)]
    logos = [quietly(lambda e=e: project_logo(e), None) if stamp else None for e, stamp in zip(entries, stamps)]
    logo_stamps = [file_stamp(logo[0]) if logo else None for logo in logos]
    etag = (
        '"'
        + hashlib.sha1(repr([registry.stamp] + stamps + trees + logo_stamps).encode("utf-8")).hexdigest()[:16]
        + '"'
    )

    def body():
        projects = []
        for entry, stamp, tree, logo in zip(entries, stamps, trees, logos):
            summary = quietly(lambda: summaries.summary(entry, stamp), None) if stamp else None
            projects.append(
                {
                    "id": entry.get("id"),
                    "name": entry.get("name"),
                    "path": entry["path"],
                    "missing": summary is None,
                    "summary": summary,
                    "worktrees": tree if summary else [],
                    # Relative to the dashboard, like the project's files; a new one shows as a new ETag
                    "logo": f"p/{entry.get('id')}/logo" if logo and summary else None,
                }
            )
        return to_json({"generated_at": now_iso(), "projects": projects}).encode("utf-8")

    return body, etag


SEARCH_QUERY_CAP = 200  # characters of the query that count
SEARCH_RESULTS_CAP = 200  # task IDs per project


def api_search(registry, summaries, query):
    """The body of /api/search?q=: per project, the IDs of the tasks.json tasks (not the archive) in which every word of
    the query appears, case-insensitively, as the dashboard's own search finds them (atm-lib's search_tasks); the
    project's name counts as part of every task's text. Read-only, like every route; `{}` for an empty query."""
    query = query[:SEARCH_QUERY_CAP]
    results = {}
    if not query.split():
        return to_json(results).encode("utf-8")
    for entry in registry.entries():

        def matches(entry=entry):
            p = served_file(entry, TASKS_FILE)
            stamp = file_stamp(p) if p else None
            if stamp is None or stamp[0] > MAX_SERVED_BYTES:
                return []
            return search_tasks(summaries.project(entry), query, also=str(entry.get("name") or ""))

        ids = quietly(matches, [])
        if ids:
            results[entry.get("id")] = ids[:SEARCH_RESULTS_CAP]
    return to_json(results).encode("utf-8")


def api_claims(summaries, entry):
    """(body, ETag) of a project's claims.json: the claims of all its worktrees (atm-lib's list_claims, from the shared
    claims store; each worktree's own snapshot has only the claims made there). None when there is no store yet."""
    stamp = file_stamp(summaries.store(entry))
    if stamp is None:
        return None

    def body():
        snapshot = {"generated_at": now_iso(), "claims": list_claims(summaries.project(entry))}
        return to_json(snapshot).encode("utf-8")

    return body, file_etag(stamp)


V1_TASKS = re.compile(r"/api/v1/projects/([^/]+)/tasks")
LIST_PARAMETERS = ("status", "label", "sprint")  # repeatable: ?status=todo&status=blocked
FLAG_PARAMETERS = ("backlog", "ready", "blocked", "in_pr", "with_user", "archived")
VALUE_PARAMETERS = ("type", "priority", "agent", "release", "component")


def task_filters(query):
    """atm-lib's Filters from a v1 route's query string (docs/layered-architecture.md §6.2): the list filters
    repeatable, the flags `true` or `1`, other parameters ignored."""
    given = parse_qs(query)
    filters = {name: given[name] for name in LIST_PARAMETERS if name in given}
    filters.update({name: given[name][0] for name in VALUE_PARAMETERS if name in given})
    filters.update({name: given[name][0].lower() in ("true", "1") for name in FLAG_PARAMETERS if name in given})
    return Filters(**filters)


def api_v1_tasks(summaries, entry, query):
    """(body, ETag) of GET /api/v1/projects/<p>/tasks: the project's tasks the filters keep, each with its derived
    values (atm-lib's task_views). The ETag changes with tasks.json, the archive and the query."""
    stamps = [file_stamp(task_file(entry, name)) for name in (TASKS_FILE, "tasks-archive.json")]
    etag = '"' + hashlib.sha1(repr([stamps, query]).encode("utf-8")).hexdigest()[:16] + '"'

    def body():
        views = task_views(summaries.project(entry), task_filters(query))
        return to_json({"generated_at": now_iso(), "tasks": views}).encode("utf-8")

    return body, etag


def api_worktrees(entry, store):
    """(body, ETag) of a project's worktrees.json: its git worktrees, main first, and for each linked one the tasks
    whose copy there differs from the main worktree's (or that only it has). The dashboard shows them all on one
    board, each task from the worktree where it got furthest. A linked worktree's copy of a task decided in another
    worktree (answered, reviewed: the claims store's `resolved`, T-204) is left out: it is stale until atm next runs
    there, and would otherwise outrank the decision."""
    trees = worktrees(entry["path"])
    stamps = [file_stamp(os.path.join(t["path"], TASK_DIR, TASKS_FILE)) for t in trees]
    etag = '"' + hashlib.sha1(repr([trees, stamps, file_stamp(store)]).encode("utf-8")).hexdigest()[:16] + '"'

    def tasks_of(tree):
        data = read_json(os.path.join(tree["path"], TASK_DIR, TASKS_FILE))
        tasks = data.get("tasks") if isinstance(data, dict) else None
        return [t for t in tasks or [] if isinstance(t, dict) and isinstance(t.get("id"), str)]

    def body():
        data = read_json(store)
        resolved = data.get("resolved") if isinstance(data, dict) and isinstance(data.get("resolved"), dict) else {}
        stale = {(r.get("worktree"), task_id) for task_id, r in resolved.items() if isinstance(r, dict)}
        listed_trees = []
        main = {}
        for tree in trees:
            if tree["main"]:
                main = {t["id"]: t for t in tasks_of(tree)}
                listed_trees.append(tree)
            else:
                differing = [
                    t for t in tasks_of(tree) if main.get(t["id"]) != t and (tree["path"], t["id"]) not in stale
                ]
                listed_trees.append({**tree, "tasks": differing})
        return to_json({"generated_at": now_iso(), "worktrees": listed_trees}).encode("utf-8")

    return body, etag


def cmd_serve_child(args):
    """The detached server process: serve on the first free port from `port`, record it, run until stopped."""
    import http.server
    import socketserver

    home, port = args.home, args.port
    record_path = os.path.join(home, SERVER_RECORD)
    with open(TEMPLATE, "rb") as f:
        page = f.read()
    # From the bytes it serves, not a second read of the file: a build writing the file in between would make whoami
    # report another build than the page has, and the page would offer a reload forever
    version, build = page_tags(page[:HEAD_BYTES].decode("utf-8", errors="replace"))
    page_headers = {"Content-Security-Policy": page_csp(page)}
    registry = Registry(home)
    summaries = Summaries()

    class Handler(http.server.BaseHTTPRequestHandler):
        server_version = "atm"
        # A connection that sends nothing (or trickles) is dropped instead of holding a thread forever (F18)
        timeout = 30

        def send(self, status, body=b"", content_type="application/json", etag=None, headers=None):
            self.send_response(status)
            if body or status == 200:
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
            if etag:
                self.send_header("ETag", etag)
            for name, value in {"Cache-Control": "no-store", **SECURITY_HEADERS, **(headers or {})}.items():
                self.send_header(name, value)
            self.end_headers()
            if body and self.command != "HEAD":
                self.wfile.write(body)

        def send_error_json(self, status, message):
            self.send(status, to_json({"error": message}).encode("utf-8"))

        def send_conditional(self, body_fn, etag, content_type="application/json", headers=None):
            if self.headers.get("If-None-Match") == etag:
                # Unchanged since the dashboard's last poll: it skips parsing and re-rendering (D-5.1)
                self.send(304, etag=etag, headers=headers)
                return
            self.send(200, body_fn(), content_type, etag, headers)

        def do_GET(self):
            # One bad request, or one project's broken files, gets a 500, never a dead handler thread with no answer
            try:
                self.handle_get()
            except Exception:  # noqa: BLE001
                try:
                    self.send_error_json(500, "internal error")
                except OSError:
                    pass

        def handle_get(self):
            port_now = self.server.server_address[1]
            if self.headers.get("Host") not in (f"localhost:{port_now}", f"127.0.0.1:{port_now}"):
                # A page on another site that points its own domain at 127.0.0.1 (DNS rebinding) is refused
                self.send_error_json(403, "unexpected Host header")
                return
            path = urlsplit(self.path).path
            if path == "/":
                self.send(200, page, "text/html; charset=utf-8", headers=page_headers)
            elif path == WHOAMI:
                info = {"atm": True, "pid": os.getpid(), "port": port_now, "version": version, "build": build}
                self.send(200, to_json(info).encode("utf-8"))
            elif path == "/api/projects":
                body, etag = api_projects(registry, summaries)
                self.send_conditional(body, etag)
            elif path == "/api/search":
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                self.send(200, api_search(registry, summaries, query))
            elif V1_TASKS.fullmatch(path):
                self.serve_v1_tasks(V1_TASKS.fullmatch(path).group(1), urlsplit(self.path).query)
            else:
                self.serve_project_file(path)

        def serve_v1_tasks(self, project, query):
            entry = registry.find(project)
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            p = served_file(entry, TASKS_FILE)
            stamp = file_stamp(p) if p else None
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp[0] > MAX_SERVED_BYTES:
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return
            body, etag = api_v1_tasks(summaries, entry, query)
            if self.headers.get("If-None-Match") == etag:
                self.send(304, etag=etag)
                return
            try:
                content = body()
            except AtmFailure as e:
                # A filter naming no such sprint or release (400), an unreadable task file (500)
                self.send_error_json(400 if e.code == 2 else 500, str(e))
                return
            self.send(200, content, "application/json", etag)

        do_HEAD = do_GET

        def serve_project_file(self, path):
            match = PROJECT_FILE.fullmatch(path)
            if not match:
                self.send_error_json(404, "not found")
                return
            entry = registry.find(match.group(1))
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            name = match.group(2)
            if served_file(entry, TASKS_FILE) is None:
                self.send_error_json(404, "not found")
                return
            if name == "logo":
                self.serve_logo(entry)
                return
            computed = None
            if name == "worktrees.json":
                computed = api_worktrees(entry, summaries.store(entry))
            elif name == "claims.json":
                computed = api_claims(summaries, entry)
            if computed:
                self.send_conditional(*computed)
                return
            p = served_file(entry, name)
            stamp = file_stamp(p) if p else None
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp[0] > MAX_SERVED_BYTES:
                # Each poll reads the whole file into memory: a runaway file is refused, not served
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return

            def read():
                with open(p, "rb") as f:
                    return f.read()

            self.send_conditional(read, file_etag(stamp))

        def serve_logo(self, entry):
            """The project's logo (logo.find_logo checks it is inside the project and small enough). The browser
            may keep it, asking again each time (no-cache) and getting a 304 while it is unchanged."""
            logo = project_logo(entry)
            stamp = file_stamp(logo[0]) if logo else None
            if stamp is None:
                self.send_error_json(404, "no logo")
                return

            def read():
                with open(logo[0], "rb") as f:
                    return f.read()

            self.send_conditional(read, file_etag(stamp), logo[1], {**LOGO_HEADERS, "Cache-Control": "no-cache"})

        def not_allowed(self):
            self.send_error_json(405, "only GET and HEAD")

        do_POST = do_PUT = do_DELETE = do_PATCH = do_OPTIONS = not_allowed

    class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
        daemon_threads = True
        # A restarted server takes its port back while the old one's connections linger in TIME_WAIT. On macOS the
        # same option would also let it share a port another server listens on, hence port_in_use() first.
        allow_reuse_address = True

    class Server6(Server):
        address_family = socket.AF_INET6

    # Browsers, curl and urllib try [::1] before 127.0.0.1 for `localhost`: the server listens on both, so no other
    # local user can take [::1] on our port and answer for us (docs/security-review.md F10). Without IPv6, 127.0.0.1
    # alone is enough: nobody can listen on [::1] then.
    server = server6 = None
    for candidate in range(port, port + PORT_TRIES):
        if port_in_use(candidate):
            continue
        try:
            server = Server(("127.0.0.1", candidate), Handler)
        except OSError:
            continue
        try:
            server6 = Server6(("::1", candidate), Handler)
        except OSError as e:
            if e.errno == errno.EADDRINUSE:
                server.server_close()
                server = None
                continue
        break
    if server is None:
        sys.stderr.write(f"atm: no free port in {port}-{port + PORT_TRIES - 1}\n")
        return
    if server6 is not None:
        threading.Thread(target=server6.serve_forever, daemon=True).start()
    record = {
        "pid": os.getpid(),
        "port": server.server_address[1],
        "version": version,
        "build": build,
        "plugin_root": PLUGIN_ROOT,
        "started_at": now_iso(),
    }
    write_json_atomic(record_path, record)

    def stop(signum, frame):
        if (read_json(record_path) or {}).get("pid") == os.getpid():
            os.remove(record_path)
        os._exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    server.serve_forever()
