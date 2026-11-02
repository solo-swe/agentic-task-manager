"""Git as the code's version control (docs/layered-architecture.md §2). It grows with the migration: the branch for
the project's card now; committing, history and the remote when `atm commit` moves (§9, step 13)."""

import os

from ..registry import head_branch


class GitVcs:
    def __init__(self, root):
        self.root = root

    def branch(self):
        """The main worktree's checked-out branch (a detached HEAD's short SHA), read from .git/HEAD without running
        git; None outside git."""
        git_dir = os.path.join(self.root, ".git")
        return head_branch(git_dir) if os.path.isdir(git_dir) else None
