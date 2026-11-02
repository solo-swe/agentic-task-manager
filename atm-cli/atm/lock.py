"""The lock directory.

A lock is a directory: mkdir succeeds for exactly one process, on every OS and file system. The holder writes
owner.json inside it and removes the directory when done. A crashed holder leaves the directory behind, so a waiter
breaks the lock when its owner is provably gone (same host, pid not running) or it is older than LOCK_STALE. See
docs/parallel-agents-design.md, section 4.
"""

import contextlib
import os
import shutil
import signal
import socket
import sys
import time

from .common.core import AtmError, now_iso, parse_iso, pid_alive, read_json, write_json_atomic
from .paths import find_paths

LOCK_DIR = ".lock"
LOCK_OWNER = "owner.json"
LOCK_TIMEOUT = 10.0  # seconds a command waits for the lock (ATM_LOCK_TIMEOUT overrides)
LOCK_STALE = 60.0  # a lock older than this is abandoned: commands hold it for milliseconds
LOCK_POLL = 0.05


def lock_timeout():
    try:
        return float(os.environ.get("ATM_LOCK_TIMEOUT", LOCK_TIMEOUT))
    except ValueError:
        return LOCK_TIMEOUT


def lock_owner(lock):
    return read_json(os.path.join(lock, LOCK_OWNER))


def lock_age(lock, owner):
    """Seconds since the lock was taken: from owner.json, or the directory itself if owner.json isn't written yet."""
    try:
        since = parse_iso(owner["acquired_at"]).timestamp() if owner else os.stat(lock).st_mtime
    except (OSError, KeyError, TypeError, ValueError):
        return 0.0
    return time.time() - since


def lock_is_stale(lock, owner):
    if owner and owner.get("host") == socket.gethostname() and isinstance(owner.get("pid"), int):
        if not pid_alive(owner["pid"]):
            return True
    return lock_age(lock, owner) > LOCK_STALE


def break_lock(lock, owner):
    """Move a stale lock aside. Returns True if this process broke it.

    Rename first, then check the owner inside the renamed directory (docs/security-review.md F13): checking first and
    renaming after leaves a gap in which another waiter breaks the same stale lock and takes a fresh one, and this
    process would then move the fresh lock aside, leaving two holders. If what was moved isn't the stale lock judged
    here, it goes back."""
    aside = f"{lock}.stale.{os.getpid()}.{os.urandom(4).hex()}"
    try:
        os.rename(lock, aside)
    except OSError:
        return False
    if lock_owner(aside) != owner:
        try:
            os.rename(aside, lock)  # someone else's live lock: put it back
        except OSError:
            pass  # a newer lock is in place already; the moved one was broken by another waiter too
        return False
    shutil.rmtree(aside, ignore_errors=True)
    return True


def describe_owner(owner):
    if not owner:
        return "another process"
    return f"pid {owner.get('pid')} on {owner.get('host')} since {owner.get('acquired_at')}"


@contextlib.contextmanager
def locked(directory, timeout=None):
    """Hold the lock for `directory` (creating it if needed) for the duration of the block. `timeout` (seconds) replaces
    the usual wait; 0 tries once."""
    os.makedirs(directory, exist_ok=True)
    lock = os.path.join(directory, LOCK_DIR)
    deadline = time.time() + (lock_timeout() if timeout is None else timeout)
    while True:
        try:
            os.mkdir(lock)
            break
        except FileExistsError:
            owner = lock_owner(lock)
            if lock_is_stale(lock, owner) and break_lock(lock, owner):
                continue
            if time.time() >= deadline:
                raise AtmError(f"timed out waiting for the lock {lock} (held by {describe_owner(owner)})")
            time.sleep(LOCK_POLL)
    # SIGTERM would otherwise end the process without running the cleanup below
    previous = signal.signal(signal.SIGTERM, lambda signum, frame: sys.exit(128 + signum))
    try:
        write_json_atomic(
            os.path.join(lock, LOCK_OWNER), {"host": socket.gethostname(), "pid": os.getpid(), "acquired_at": now_iso()}
        )
        yield
    finally:
        shutil.rmtree(lock, ignore_errors=True)
        signal.signal(signal.SIGTERM, previous)


def cmd_lock_test(args):
    """Test hook: hold the task folder's lock for `seconds`, logging to .lock-test.log, so tests can check exclusion."""
    seconds = args.seconds
    task_dir = find_paths(os.getcwd())["dir"]
    log = os.path.join(task_dir, ".lock-test.log")
    with locked(task_dir):
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"acquire {os.getpid()}\n")
        time.sleep(seconds)
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"release {os.getpid()}\n")
