"""Running git, and reading a working copy's HEAD (docs/layered-architecture.md §11.8): the one place that does, for
both of git's roles, carrying the task files (the JSON-in-git store) and the code's version control (vcs/git.py)."""

import os
import subprocess

from ..common.errors import AtmFailure


def git(args, cwd):
    """Run git; return stdout stripped, or None if git fails or isn't installed (a question, not a step)."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def git_exit(args, cwd):
    """Run git for its exit code alone (a command that answers yes or no that way); None if git isn't installed."""
    try:
        return subprocess.run(["git"] + args, cwd=cwd, capture_output=True).returncode
    except OSError:
        return None


def run_git(args, cwd):
    """Run git as a step that must work: return stdout stripped, or raise with git's own message."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError as e:
        raise AtmFailure(f"git isn't available: {e}")
    if result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        command = next((a for a in args if not a.startswith("-")), "")
        raise AtmFailure(f"git {command} failed: {message}")
    return result.stdout.strip()


def in_git(folder):
    """Whether the folder is in a git repository: the one way atm asks."""
    return git(["rev-parse", "--git-dir"], folder) is not None


def head_branch(git_dir):
    """The branch checked out per a git directory's HEAD file; a detached HEAD gives its short SHA, no file None."""
    try:
        with open(os.path.join(git_dir, "HEAD"), encoding="utf-8") as f:
            head = f.readline().strip()
    except OSError:
        return None
    if head.startswith("ref: "):
        ref = head[5:]
        return ref[len("refs/heads/") :] if ref.startswith("refs/heads/") else ref
    return head[:7] or None


def checked_out_branch(worktree):
    """What "the branch" means everywhere in atm (a claim's, a project card's, the worktree list's): the branch the
    working copy at `worktree` has checked out, read from its HEAD without running git; a detached HEAD gives its
    short SHA; None outside git. A linked worktree's `.git` is a file naming its git directory."""
    dot_git = os.path.join(worktree, ".git")
    if os.path.isfile(dot_git):
        try:
            with open(dot_git, encoding="utf-8") as f:
                line = f.readline().strip()
        except OSError:
            return None
        if not line.startswith("gitdir: "):
            return None
        return head_branch(os.path.join(worktree, line[len("gitdir: ") :]))
    return head_branch(dot_git)
