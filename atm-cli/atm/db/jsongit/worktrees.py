"""The JSON-in-git store's working copies: a clone's git worktrees, each with its own tasks.json (the main worktree's
is the project's; the others show on the board as worktree copies, `worktrees.json`). Read from the git directory,
without running git where it can, so the server can ask on every poll."""

import os

from .gitrun import git


def main_worktree(root):
    """The main worktree's folder for a project root (itself unless it is a linked git worktree)."""
    common = git(["rev-parse", "--git-common-dir"], root)
    if common:
        common = os.path.normpath(os.path.join(root, common))
        if os.path.basename(common) == ".git":
            return os.path.dirname(common)
    return root


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


def worktrees(root):
    """The git worktrees of the project whose main worktree is `root`, main first, as `git worktree list` has them:
    [{path, branch, main}]. Linked worktrees whose folder is gone are left out; outside git (or a bare repository) the
    list is empty."""
    git_dir = os.path.join(root, ".git")
    if not os.path.isdir(git_dir):
        return []
    found = [{"path": root, "branch": head_branch(git_dir), "main": True}]
    linked = os.path.join(git_dir, "worktrees")
    try:
        names = sorted(os.listdir(linked))
    except OSError:
        names = []
    for name in names:
        admin = os.path.join(linked, name)
        try:
            with open(os.path.join(admin, "gitdir"), encoding="utf-8") as f:
                path = os.path.dirname(f.readline().strip())  # the worktree's .git file
        except OSError:
            continue
        if path and os.path.isdir(path):
            found.append({"path": path, "branch": head_branch(admin), "main": False})
    return found
