"""The dashboard server (the REST API): one per user, a detached process serving every registered project.

The server holds the plugin's dashboard in memory and answers its requests through atm-lib: the projects, their
tasks (v1), claims, worktrees and logos. It reads no task file itself (docs/layered-architecture.md §6); its routes
and record are described in docs/multi-project-api.md. It runs as its own process, entered with `serve_main` (the
built atm.py with `--server`, as plugin/scripts/atm-serve runs it); the CLI starts, stops and refreshes it
(`cli/serve.py`) and never imports this module.
"""

import base64
import errno
import hashlib
import os
import re
import signal
import socket
import sys
import threading
from urllib.parse import parse_qs, urlsplit

from ..boot.open import open_registry, project_at, task_paths
from ..common.core import PLUGIN_ROOT, WHOAMI, now, now_iso, read_json, to_json, write_json_atomic
from ..common.dashboard import HEAD_BYTES, SERVER_RECORD, TEMPLATE, page_tags
from ..common.errors import AtmFailure
from ..lib.archive import archive_hint_now
from ..lib.query import Filters, list_claims, project_summary, read_problems, search_tasks, task_views
from ..lib.settings import get_logo
from ..lib.worktrees import worktree_copies

PORT_TRIES = 100
# Computed from the project's data, under the paths the dashboard has always used (docs/multi-project-api.md)
PROJECT_FILES = ("claims.json", "worktrees.json")
# Task data the server will read into memory and send (a big project's archive is a few MB)
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


def logo_stamp(p):
    """(size, nanosecond mtime) of the logo's file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


def file_etag(stamp):
    """A strong ETag from a file's size and nanosecond mtime. Last-Modified alone has one-second resolution, so two
    atm changes in the same second would look unchanged to the dashboard."""
    return f'"{stamp[0]:x}-{stamp[1]:x}"'


class ServedProjects:
    """The registry as the server sees it, re-read when it changes."""

    def __init__(self, home):
        self.registry = open_registry(home)
        self.stamp = None
        self.projects = []

    def entries(self):
        stamp = self.registry.stamp()
        if stamp != self.stamp:
            self.projects = self.registry.entries()
            self.stamp = stamp
        return self.projects

    def find(self, project):
        return next((e for e in self.entries() if e.get("id") == project), None)


class Summaries:
    """Each registered project, opened through boot, and its summary (atm-lib's project_summary), computed again only
    when its task data or claims changed, or the day did (the completions count by day)."""

    def __init__(self):
        self.paths = {}  # project folder -> where its data is (boot's task_paths): finding it runs git
        self.cache = {}  # project folder -> (stamp, summary or None)

    def project(self, entry, tolerant=False):
        """The project, opened afresh for each request: atm-lib reads one snapshot of its data per call. `tolerant`
        reads a malformed task file as far as it can be read, for showing it (T-377). A task folder atm refuses (a
        symlink, docs/security-review.md F7, F8) raises AtmFailure."""
        if entry["path"] not in self.paths:
            self.paths[entry["path"]] = task_paths(entry["path"])
        return project_at(self.paths[entry["path"]], tolerant=tolerant)

    def data_stamp(self, entry):
        """The project's store stamp ({tasks, archive, claims, head}), or None when the project is missing or refused."""
        try:
            stamp = self.project(entry).store.stamp()
        except AtmFailure:
            return None
        return stamp if stamp["tasks"] is not None else None

    def stamp(self, entry):
        """What the summary depends on, or None when the project is missing: its task data, claims, the day, and the
        checked-out branch (on the card)."""
        stamp = self.data_stamp(entry)
        if stamp is None:
            return None
        return (stamp["tasks"], stamp["claims"], now().date().isoformat(), stamp["head"])

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


def project_logo(summaries, entry):
    """(absolute path, content type) of a registered project's logo (meta.logo, or one found), or None."""
    logo = get_logo(summaries.project(entry, tolerant=True))["found"]
    return (logo["file"], logo["type"]) if logo else None


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
    trees = [
        quietly(lambda e=e: summaries.project(e).store.working_copies(), []) if stamp else []
        for e, stamp in zip(entries, stamps)
    ]
    logos = [quietly(lambda e=e: project_logo(summaries, e), None) if stamp else None for e, stamp in zip(entries, stamps)]
    logo_stamps = [logo_stamp(logo[0]) if logo else None for logo in logos]
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
            stamp = summaries.data_stamp(entry)
            if stamp is None or stamp["tasks"][0] > MAX_SERVED_BYTES:
                return []
            return search_tasks(summaries.project(entry), query, also=str(entry.get("name") or ""))

        ids = quietly(matches, [])
        if ids:
            results[entry.get("id")] = ids[:SEARCH_RESULTS_CAP]
    return to_json(results).encode("utf-8")


def stamp_etag(*parts):
    """A strong ETag from what a response depends on (store stamps, the query)."""
    return '"' + hashlib.sha1(repr(list(parts)).encode("utf-8")).hexdigest()[:16] + '"'


def api_claims(summaries, entry, stamp):
    """(body, ETag) of a project's claims.json: the claims of all its worktrees (atm-lib's list_claims, from the shared
    claims store; each worktree's own snapshot has only the claims made there); none before the first claim."""

    def body():
        snapshot = {"generated_at": now_iso(), "claims": list_claims(summaries.project(entry))}
        return to_json(snapshot).encode("utf-8")

    return body, stamp_etag("claims", stamp["claims"])


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


def api_v1_tasks(summaries, entry, query, stamp):
    """(body, ETag) of GET /api/v1/projects/<p>/tasks: the project's tasks the filters keep, each with its derived
    values (atm-lib's task_views), and in the same snapshot the project's meta and how big its task data is. The ETag
    changes with the task data, the archive and the query."""
    etag = stamp_etag(stamp["tasks"], stamp["archive"], query)

    def body():
        # Read tolerantly: the dashboard shows a malformed task file, with what is wrong with it (T-377)
        project = summaries.project(entry, tolerant=True)
        views = task_views(project, task_filters(query))
        snapshot = {
            "generated_at": now_iso(),
            "meta": project.store.document().get("meta") or {},
            "bytes": project.store.sizes()["tasks"],
            "tasks": views,
            "problems": read_problems(project),
        }
        hint = archive_hint_now(project) if not task_filters(query).archived else None
        if hint:
            snapshot["archive_hint"] = hint  # atm's suggestion to archive: the dashboard's footer notice (T-372)
        return to_json(snapshot).encode("utf-8")

    return body, etag


def api_worktrees(summaries, entry, stamp):
    """(body, ETag) of a project's worktrees.json: its working copies, main first, and for each other one the tasks
    whose copy there differs from the main one's (atm-lib's worktree_copies). The dashboard shows them all on one
    board, each task from the working copy where it got furthest. The ETag changes with the copies and the claims
    (a decision made in another worktree hides a stale copy)."""
    def body():
        copies = worktree_copies(summaries.project(entry))
        return to_json({"generated_at": now_iso(), "worktrees": copies}).encode("utf-8")

    return body, stamp_etag(stamp["copies"], stamp["claims"])


def serve_main(argv):
    """The server process's entry (`atm.py --server <home> <port>`): its arguments, then `serve`. Returns its exit
    code: 2 for arguments it can't use."""
    if len(argv) != 2 or not argv[1].isdigit():
        sys.stderr.write("atm-serve: usage: atm-serve <atm folder> <first port>\n")
        return 2
    serve(os.path.abspath(argv[0]), int(argv[1]))
    return 0


def serve(home, port):
    """The detached server process: serve on the first free port from `port`, record it in `home`, run until
    stopped."""
    import http.server
    import socketserver

    record_path = os.path.join(home, SERVER_RECORD)
    with open(TEMPLATE, "rb") as f:
        page = f.read()
    # From the bytes it serves, not a second read of the file: a build writing the file in between would make whoami
    # report another build than the page has, and the page would offer a reload forever
    version, build = page_tags(page[:HEAD_BYTES].decode("utf-8", errors="replace"))
    page_headers = {"Content-Security-Policy": page_csp(page)}
    registry = ServedProjects(home)
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
            stamp = summaries.data_stamp(entry)
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp["tasks"][0] > MAX_SERVED_BYTES:
                # Each request reads the whole task data into memory: a runaway file is refused, not served
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return
            body, etag = api_v1_tasks(summaries, entry, query, stamp)
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
            stamp = summaries.data_stamp(entry)
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if name == "logo":
                self.serve_logo(entry)
            elif name == "worktrees.json":
                self.send_conditional(*api_worktrees(summaries, entry, stamp))
            else:
                self.send_conditional(*api_claims(summaries, entry, stamp))

        def serve_logo(self, entry):
            """The project's logo (the store checks it is inside the project and small enough). The browser may keep
            it, asking again each time (no-cache) and getting a 304 while it is unchanged."""
            logo = project_logo(summaries, entry)
            stamp = logo_stamp(logo[0]) if logo else None
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
