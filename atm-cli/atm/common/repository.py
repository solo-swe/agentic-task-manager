"""Repository web addresses (meta.repository, e.g. https://github.com/o/r), which the dashboard uses to link commits
and pull requests: a git remote turned into one, and PR links moved when the repository moves."""

import re

SCP_REMOTE = re.compile(r"^(?:[\w.-]+@)?([\w.-]+):(?!//)(.+)$")  # git@github.com:o/r.git
URL = re.compile(r"^(https?|ssh|git)://(?:[^@/]+@)?([^/:]+)(?::\d+)?(/.*)?$")


def repository_url(text):
    """The web address of a repository: an http(s) URL as given, or a git remote turned into one
    (git@github.com:o/r.git, ssh://git@github.com/o/r.git, https://github.com/o/r.git -> https://github.com/o/r).
    Credentials, a trailing .git and a trailing slash go. None if it isn't one."""
    text = (text or "").strip()
    m = URL.match(text)
    if m:
        scheme, host, path = m.group(1), m.group(2), (m.group(3) or "")
        web = "https" if scheme in ("ssh", "git") else scheme
    else:
        m = SCP_REMOTE.match(text)
        if not m or "/" in m.group(1):
            return None
        web, host, path = "https", m.group(1), "/" + m.group(2)
    path = re.sub(r"\.git/?$", "", path.rstrip("/")).rstrip("/")
    if not host or "." not in host and host != "localhost" or not path.strip("/"):
        return None
    return f"{web}://{host}{path}"


def move_pr_links(tasks, old, new):
    """Point the tasks' recorded PR links (pr.url) on the `old` address at the `new` one, after the repository moved.
    Only the link changes: no timestamps or actors, so the history stays as it was. Returns how many moved."""
    moved = 0
    for task in tasks:
        pr = task.get("pr") if isinstance(task, dict) else None
        link = pr.get("url") if isinstance(pr, dict) else None
        if isinstance(link, str) and (link == old or link.startswith(old + "/")):
            pr["url"] = new + link[len(old) :]
            moved += 1
    return moved
