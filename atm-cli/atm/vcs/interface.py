"""The code's version control, as atm-lib uses it (docs/layered-architecture.md §2). Git's other role, carrying the
task files, belongs to the JSON-in-git store, not here. vcs/git.py implements it with git."""

from typing import List, Optional, Protocol


class Vcs(Protocol):
    def in_repository(self) -> bool:
        """Whether the project's folder is under version control at all."""
        ...

    def paths(self, files: List[str], cwd: str) -> List[str]:
        """The files to commit, relative to the repository, from names relative to `cwd`; refuses a name that would
        commit more than one file (the whole tree, a directory, a pattern) or a file outside the repository."""
        ...

    def commit(self, files: List[str], message: str) -> str:
        """Commit exactly these files; the new commit's SHA."""
        ...

    def resolve(self, rev: str) -> Optional[str]:
        """A revision's SHA, or None if there is no such commit."""
        ...

    def in_history(self, sha: str) -> bool:
        """Whether the commit is reachable from the current one."""
        ...

    def message(self, sha: str) -> str:
        """A commit's message."""
        ...

    def default_base(self) -> Optional[str]:
        """The remote's default branch."""
        ...

    def remote_url(self) -> Optional[str]:
        """The remote's address."""
        ...

    def branch(self) -> Optional[str]:
        """The checked-out branch."""
        ...
