"""atm's errors (docs/layered-architecture.md §4.2, §5): atm-lib raises them by what went wrong, and each front end
says it its own way: the CLI as a message and an exit code (`code`), the REST API as an HTTP status."""


class AtmFailure(Exception):
    """Something atm can't do, with a message for the person or agent who asked. `code` is the CLI's exit code."""

    code = 1

    def __init__(self, message, code=None):
        super().__init__(message)
        if code is not None:
            self.code = code


class NotFound(AtmFailure):
    """No such task, sprint, release, component or project."""


class Invalid(AtmFailure):
    """The request itself is wrong: an unknown option value, a malformed ID (exit code 2, like a usage error)."""

    code = 2


class Conflict(AtmFailure):
    """The data changed since it was read, or the change isn't possible in the task's state."""


class NothingAvailable(AtmFailure):
    """No task to hand out (exit code 3, `atm next`)."""

    code = 3


class Claimed(AtmFailure):
    """Another agent holds the task, or the caller's lease ran out (exit code 4)."""

    code = 4
