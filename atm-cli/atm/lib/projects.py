"""atm-lib: the user's projects, the ones the dashboard shows (`atm projects`; docs/layered-architecture.md §3.2, §5.1):
list, add, remove and prune them in the registry (`registry`, a db Registry: boot opens it)."""


def list_projects(registry):
    """{projects: the entries, each with `missing`, damaged: where an unreadable registry was moved, or None}."""
    return registry.list_projects()


def add_project(registry, target):
    """Register the project in the folder `target` (the current one when None). {result, project, damaged}."""
    return registry.add_project(target)


def remove_project(registry, target):
    """Unregister the project with this ID or path. {removed, damaged}."""
    return registry.remove_project(target)


def prune_projects(registry):
    """Unregister the projects whose task folder is gone. {removed, damaged}."""
    return registry.prune()
