"""Git as the carrier of the task files (docs/layered-architecture.md §2): finding the repository and its common
directory, staging the task folder's files, the merge driver's config. Git as the code's version control is vcs/git.py."""

import subprocess

from ...common.core import rel


def git(args, cwd):
    """Run git; return stdout stripped, or None if git fails or isn't installed."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def is_tracked(root, p):
    return git(["ls-files", "--error-unmatch", "--", rel(root, p)], root) is not None


def in_git(root):
    return git(["rev-parse", "--git-dir"], root) is not None
