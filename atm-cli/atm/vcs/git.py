"""Git as the code's version control (docs/layered-architecture.md §2): committing exactly the files named, the
history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's."""

import os

from ..common.errors import Invalid, NotFound
from ..common.repository import repository_url
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
                raise Invalid(f"atm vcs commit stages only the files you name; {name!r} would stage the whole tree")
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

    def changes_under(self, folders, cwd=None):
        """`atm commit --dir` (T-405): every change git sees under each folder (named relative to `cwd`, None: the
        repository), as `git status` lists it: modified, added, deleted, renamed (both names) and new files git doesn't
        ignore; relative to the repository, in order. Refuses the repository's root or above, a folder outside it, the
        task folder (tasks.json goes with every commit anyway), a file, and a folder with nothing changed."""
        picked = []
        for name in folders:
            full = os.path.realpath(os.path.join(cwd or self.root, name))
            relative = os.path.relpath(full, self.root).replace(os.sep, "/")
            if relative in (".", "..") or relative.startswith("../"):
                raise Invalid(f"--dir {name}: name a folder inside the repository, not the repository itself or above it")
            if relative == ".agentic-task-manager" or relative.startswith(".agentic-task-manager/"):
                raise Invalid(f"--dir {name}: the task folder isn't committed this way (tasks.json goes with every commit)")
            if os.path.isfile(full):
                raise Invalid(f"--dir {name} is a file: name it with --file")
            status = run_git(
                LITERAL + ["status", "--porcelain", "-z", "--untracked-files=all", "--", relative], self.root, strip=False
            )
            entries = [e for e in status.split("\0") if e]
            found = []
            i = 0
            while i < len(entries):
                entry = entries[i]
                i += 1
                if len(entry) < 4:
                    continue
                code, path = entry[:2], entry[3:]
                found.append(path)
                # A rename or a copy: the next entry is the name it had, which goes in the commit too
                if "R" in code or "C" in code:
                    if i < len(entries):
                        found.append(entries[i])
                    i += 1
            found = [p for p in found if p == relative or p.startswith(relative + "/")]
            if not found:
                raise NotFound(f"--dir {name}: nothing changed under it")
            picked += [p for p in found if p not in picked]
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

    def _submodule(self, relative):
        """The submodule `relative` is in, nearest the root, as (its folder, the path inside it), or None. A submodule
        is a folder this repository records as a commit (mode 160000) with a checkout of its own (`.git`); git answers
        nothing about the files in it from here, only from its own repository (T-408). A nested repository that isn't a
        submodule is not one: its files stay this repository's to judge (and git refuses them, so they stay hidden)."""
        parts = relative.split("/")
        for i in range(1, len(parts)):
            folder = "/".join(parts[:i])
            if not os.path.lexists(os.path.join(self.root, folder, ".git")):
                continue
            for line in (git(LITERAL + ["ls-files", "--stage", "--", folder], self.root) or "").splitlines():
                meta, _, path = line.partition("\t")
                if path == folder and meta.split(" ", 1)[0] == "160000":
                    return folder, "/".join(parts[i:])
        return None

    def _in_submodule(self, relative):
        """The submodule's own GitVcs and the path inside it, or None outside one"""
        found = self._submodule(relative)
        return (GitVcs(os.path.join(self.root, found[0])), found[1]) if found else None

    def ignored(self, relative):
        """git's ignore rules, as `git status` applies them: a file git tracks is never ignored. check-ignore takes
        paths, not patterns (and refuses --literal-pathspecs). It fails closed: only git's "not ignored" (exit 1)
        lets a file through; an error counts as ignored. A file in a submodule: the submodule's own rules decide (git
        refuses the question from here), down to the innermost submodule."""
        if not self.in_repository():
            return False
        inside = self._in_submodule(relative)
        if inside:
            return inside[0].ignored(inside[1])
        return git_exit(["check-ignore", "-q", "--", relative], self.root) != 1

    def tracked(self, relative):
        if not self.in_repository():
            return None
        inside = self._in_submodule(relative)
        if inside:
            return inside[0].tracked(inside[1])
        return git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is not None

    def head(self):
        return self.resolve("HEAD") if self.in_repository() else None

    def submodule_of(self, relative):
        """For a file in a submodule (the innermost): {path: its folder from the root, commit: its HEAD, repository:
        its origin's web address or None}; None for any other file."""
        if not self.in_repository():
            return None
        found = self._submodule(relative)
        if not found:
            return None
        sub = GitVcs(os.path.join(self.root, found[0]))
        nested = sub.submodule_of(found[1])
        if nested:
            return {**nested, "path": f"{found[0]}/{nested['path']}"}
        return {"path": found[0], "commit": sub.head(), "repository": repository_url(sub.remote_url())}
