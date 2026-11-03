"""Git as the code's version control (docs/layered-architecture.md §2): committing exactly the files named, the
history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's."""

import os

from ..common.errors import Invalid, NotFound
from ..host.git import checked_out_branch, git, git_exit, in_git, run_git

# Pathspecs that stage more than the files named: another agent's work can be in the same tree
WHOLE_TREE = {".", "-A", "--all", "-u", "--update", ":/", "*"}
# git reads a path as a pattern (`*.env` is every .env file, `src/[id]` matches `src/i`); with this, a path names
# exactly one file (docs/security-review.md F6)
LITERAL = ["--literal-pathspecs"]


class GitVcs:
    def __init__(self, root):
        self.root = root

    def in_repository(self):
        return in_git(self.root)

    def branch(self):
        """The checked-out branch (a detached HEAD's short SHA), read from HEAD without running git, as the store's
        claims record it (host/git.py); None outside git."""
        return checked_out_branch(self.root)

    def user_name(self):
        """Who git says the user is (`user.name`), for naming the actor; None when it isn't set."""
        return git(["config", "user.name"], self.root)

    def default_base(self):
        """The remote's default branch (origin/HEAD), else main."""
        ref = git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], self.root)
        return ref.split("/", 1)[1] if ref and "/" in ref else "main"

    def remote_url(self):
        """The origin remote's address, as git has it (atm repository turns it into a web address)."""
        return git(["remote", "get-url", "origin"], self.root)

    def paths(self, files, cwd=None):
        """The files to commit, relative to the repository, from names relative to `cwd` (None: the repository);
        refuses anything that would stage more than the files named, and a missing file git doesn't track."""
        picked = []
        for name in files:
            if name in WHOLE_TREE or name.startswith(":"):
                raise Invalid(f"atm commit stages only the files you name; {name!r} would stage the whole tree")
            full = os.path.realpath(os.path.join(cwd or self.root, name))
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

    # The working copy's files, for the document viewer (docs/compmd-viewer-design.md §4). atm-lib checks the path
    # first (lib/documents.py); these read only below the root, through no symbolic link

    def _file(self, relative):
        """The file's absolute path, or None when a segment from the root down is a symbolic link or it isn't a
        regular file."""
        current = self.root
        for segment in relative.split("/"):
            current = os.path.join(current, segment)
            if os.path.islink(current):
                return None
        return current if os.path.isfile(current) else None

    def file_stamp(self, relative):
        path = self._file(relative)
        if path is None:
            return None
        try:
            st = os.stat(path)
        except OSError:
            return None
        return (st.st_size, st.st_mtime_ns)

    def read_file(self, relative):
        path = self._file(relative)
        if path is None:
            raise NotFound(f"{relative}: no such file")
        with open(path, "rb") as f:
            return f.read()

    def ignored(self, relative):
        """git's ignore rules, as `git status` applies them: a file git tracks is never ignored. check-ignore takes
        paths, not patterns (and refuses --literal-pathspecs). It fails closed: only git's "not ignored" (exit 1)
        lets a file through; an error counts as ignored."""
        if not self.in_repository():
            return False
        return git_exit(["check-ignore", "-q", "--", relative], self.root) != 1

    def tracked(self, relative):
        if not self.in_repository():
            return None
        return git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is not None

    def head(self):
        return self.resolve("HEAD") if self.in_repository() else None
