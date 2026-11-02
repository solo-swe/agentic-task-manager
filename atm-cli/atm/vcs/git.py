"""Git as the code's version control (docs/layered-architecture.md §2): committing exactly the files named, the
history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's."""

import os
import subprocess

from ..common.errors import AtmFailure, Invalid, NotFound
from ..core import git
from ..registry import head_branch

# Pathspecs that stage more than the files named: another agent's work can be in the same tree
WHOLE_TREE = {".", "-A", "--all", "-u", "--update", ":/", "*"}
# git reads a path as a pattern (`*.env` is every .env file, `src/[id]` matches `src/i`); with this, a path names
# exactly one file (docs/security-review.md F6)
LITERAL = ["--literal-pathspecs"]


def run_git(args, root):
    """Run git; return stdout stripped, or raise with git's own message."""
    result = subprocess.run(["git"] + args, cwd=root, capture_output=True, text=True)
    if result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        command = next((a for a in args if not a.startswith("-")), "")
        raise AtmFailure(f"git {command} failed: {message}")
    return result.stdout.strip()


class GitVcs:
    def __init__(self, root):
        self.root = root

    def in_repository(self):
        return git(["rev-parse", "--git-dir"], self.root) is not None

    def branch(self):
        """The main worktree's checked-out branch (a detached HEAD's short SHA), read from .git/HEAD without running
        git; None outside git."""
        git_dir = os.path.join(self.root, ".git")
        return head_branch(git_dir) if os.path.isdir(git_dir) else None

    def default_base(self):
        """The remote's default branch (origin/HEAD), else main."""
        ref = git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], self.root)
        return ref.split("/", 1)[1] if ref and "/" in ref else "main"

    def remote_url(self):
        """The origin remote's address, as git has it (atm repository turns it into a web address)."""
        return git(["remote", "get-url", "origin"], self.root)

    def paths(self, files, cwd):
        """The files to commit, relative to the repository, from names relative to `cwd`; refuses anything that
        would stage more than the files named, and a missing file git doesn't track."""
        picked = []
        for name in files:
            if name in WHOLE_TREE or name.startswith(":"):
                raise Invalid(f"atm commit stages only the files you name; {name!r} would stage the whole tree")
            full = os.path.realpath(os.path.join(cwd, name))
            relative = os.path.relpath(full, self.root)
            if relative == "." or relative.startswith(".." + os.sep) or relative == "..":
                raise Invalid(f"{name} is outside the repository")
            if os.path.isdir(full):
                raise Invalid(f"{name} is a directory: name the files in it")
            # A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
            if (
                not os.path.exists(full)
                and git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is None
                and git(["cat-file", "-e", f"HEAD:{relative}"], self.root) is None
            ):
                raise NotFound(f"{name}: no such file, and git doesn't track it")
            if relative not in picked:
                picked.append(relative)
        return picked

    def commit(self, files, message):
        """Commit exactly `files` (relative to the repository); whatever else is staged stays staged and out of this
        commit. Returns the new commit's SHA."""
        # A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
        to_add = [
            f
            for f in files
            if git(LITERAL + ["ls-files", "--error-unmatch", "--", f], self.root) is not None
            or os.path.lexists(os.path.join(self.root, f))
        ]
        if to_add:
            run_git(LITERAL + ["add", "-A", "--"] + to_add, self.root)
        run_git(LITERAL + ["commit", "-q", "-m", message, "--"] + list(files), self.root)
        return run_git(["rev-parse", "HEAD"], self.root)

    def resolve(self, rev):
        """A revision's SHA, or None if there is no such commit."""
        return git(["rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"], self.root)

    def in_history(self, sha):
        """Whether the commit is an ancestor of HEAD (on the current branch)."""
        return git(["merge-base", "--is-ancestor", sha, "HEAD"], self.root) is not None

    def message(self, sha):
        return run_git(["log", "-1", "--format=%B", sha], self.root)
