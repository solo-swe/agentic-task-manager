"""Constants, errors, output, time helpers and atomic writes, shared by every layer. Running git is not here: the
store runs it for the task files (db/jsongit/gitrun.py), the vcs for the code (vcs/git.py)."""

import contextlib
import json
import os
import re
import sys
from datetime import datetime, timezone

from .errors import AtmFailure

# Where the built plugin/scripts/atm.py lives (only the built file is run, never these modules)
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
TASK_DIR = ".agentic-task-manager"
TASKS_FILE = "tasks.json"
DASHBOARD_FILE = "tasks-dashboard.html"


class AtmError(AtmFailure):
    """The errors of the modules from before the layers: atm's errors (common/errors.py) without a kind."""

    def __init__(self, message, code=1):
        super().__init__(message, code)


class AtmExit(Exception):
    """End with an exit code after normal output (not an error)."""

    def __init__(self, code):
        super().__init__(code)
        self.code = code


def out(text=""):
    sys.stdout.write(text + "\n")


def to_json(value):
    return json.dumps(value, indent=2, ensure_ascii=False)


def rel(root, p):
    return os.path.relpath(p, root)


def atm_home():
    """The user's atm folder: ATM_HOME, else ~/.agentic-task-manager (the registry and the server's record)."""
    return os.path.abspath(os.path.expanduser(os.environ.get("ATM_HOME") or "~/.agentic-task-manager"))


def ensure_home(home):
    """Create the atm folder readable by its owner only: it lists the user's projects and the server's pid and port
    (docs/security-review.md F18). A folder that exists already keeps its permissions."""
    os.makedirs(home, mode=0o700, exist_ok=True)


def plugin_version():
    with open(os.path.join(PLUGIN_ROOT, ".claude-plugin", "plugin.json"), encoding="utf-8") as f:
        return json.load(f)["version"]


def parse_version(text):
    parts = []
    for part in (text or "").split("."):
        m = re.match(r"\d+", part)
        parts.append(int(m.group()) if m else 0)
    return parts


def now():
    """The current time; ATM_NOW (ISO 8601 UTC) overrides it, for tests."""
    fixed = os.environ.get("ATM_NOW")
    return parse_iso(fixed) if fixed else datetime.now(timezone.utc)


def now_iso():
    return now().strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def format_duration(seconds):
    seconds = max(0, int(seconds))
    if seconds < 60:
        return f"{seconds}s"
    if seconds < 3600:
        return f"{seconds // 60}m"
    return f"{seconds // 3600}h {seconds % 3600 // 60}m"


def fsync_dir(directory):
    """Make a rename in `directory` durable. Not every platform can open a directory; that's fine to skip."""
    try:
        fd = os.open(directory, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(fd)
    except OSError:
        pass
    finally:
        os.close(fd)


def write_text_atomic(p, text):
    """Replace a file so readers only ever see the complete old or the complete new content."""
    tmp = f"{p}.{os.getpid()}.{os.urandom(4).hex()}.tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, p)
    except BaseException:
        with contextlib.suppress(OSError):
            os.remove(tmp)
        raise
    fsync_dir(os.path.dirname(os.path.abspath(p)))


def write_json_atomic(p, value):
    write_text_atomic(p, to_json(value) + "\n")


def read_json(p):
    """The file's JSON, or None when it can't be read or parsed. Deeply nested JSON (a hostile file) raises
    RecursionError in the parser; that is unreadable too, not a crash (docs/security-review.md F12, F18)."""
    try:
        with open(p, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError, RecursionError):
        return None


def is_zombie(pid):
    """A process that has exited but wasn't reaped yet (common in containers without an init process). Linux only;
    elsewhere an orphaned process is reaped by the system right away."""
    try:
        with open(f"/proc/{pid}/stat", encoding="utf-8") as f:
            stat = f.read()
    except OSError:
        return False
    fields = stat[stat.rfind(")") + 1 :].split()
    return bool(fields) and fields[0] == "Z"


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return not is_zombie(pid)


WHOAMI = "/__atm/whoami"


def whoami(port):
    """Ask the atm server on this port who it is; None if nothing (or not an atm server) answers."""
    import urllib.request

    try:
        with urllib.request.urlopen(f"http://localhost:{port}{WHOAMI}", timeout=1) as response:
            info = json.loads(response.read().decode("utf-8"))
    except (OSError, ValueError):
        return None
    return info if isinstance(info, dict) and info.get("atm") else None
