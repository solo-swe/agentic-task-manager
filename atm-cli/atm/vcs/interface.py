"""The code's version control, as atm-lib uses it (docs/layered-architecture.md §2). Git's other role, carrying the
task files, belongs to the JSON-in-git store, not here. Nothing implements it yet: the slice that moves `atm commit`
does (§9, step 13)."""

from typing import List, Optional, Protocol


class Vcs(Protocol):
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
