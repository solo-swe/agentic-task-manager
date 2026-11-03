"""Git as the carrier of the task files (docs/layered-architecture.md §2): whether a task file is tracked. Running git
itself is host/git.py's (§11.8); git as the code's version control is vcs/git.py."""

from ...common.core import rel
from ...host.git import git


def is_tracked(root, p):
    return git(["ls-files", "--error-unmatch", "--", rel(root, p)], root) is not None
