"""Build plugin/scripts/atm.py from the modules in atm-cli/atm/.

The plugin ships one readable file (users install by cloning; nothing is built on their side), while development
happens in modules. The build concatenates the modules in dependency order under a banner each, hoists and
deduplicates their standard-library imports, and drops their imports of each other, since everything ends up in one
namespace. It refuses to build when two modules define the same top-level name (concatenation would silently shadow
one), or when a module imports from a later module or a name another module doesn't define.

It also holds the layer rule (docs/layered-architecture.md §1, §8): a module's layer is its folder (`db/jsongit/store.py`
is `db.jsongit.store`, in the db layer), and an import the rule doesn't allow is refused. The modules from before the
layers (flat, in atm-cli/atm/) and the imports that still break the rule are listed in atm-cli/layers.json, the
ratchet: the build refuses a flat module or a forbidden import that isn't listed, and an entry that is no longer
needed, so the list only shrinks as the migration goes on.

Usage: python3 atm-cli/build.py           write plugin/scripts/atm.py (only if it changed)
       python3 atm-cli/build.py --check   exit 1 if plugin/scripts/atm.py is not what the modules build
       python3 atm-cli/build.py --output F   write the build to F instead
Design: docs/v4/atm-cli.md, section 2.
"""

import ast
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE = os.path.join(HERE, "atm")
LAYERS_FILE = os.path.join(HERE, "layers.json")
TARGET = os.path.join(os.path.dirname(HERE), "plugin", "scripts", "atm.py")
# Dependency order: a module may only import from modules before it. A dotted name is a module in a layer's folder.
MODULES = [
    "core",
    "paths",
    "lock",
    "registry",
    "model",
    "common.fields",
    "db.interface",
    "db.jsongit.store",
    "db.memory",
    "vcs.interface",
    "lib.project",
    "lib.tasks",
    "boot.open",
    "cli.render",
    "components",
    "sprint_names",
    "repository",
    "logo",
    "claims",
    "query",
    "tasks",
    "edit",
    "sprints",
    "releases",
    "merge",
    "archive",
    "git_ops",
    "migrate",
    "server",
    "cli.show",
    "cli.main",
]

# The layers a layer may import from (docs/layered-architecture.md §1). `boot` is where a project is opened: the one
# place that picks the store and the version control, so atm-lib knows only their interfaces.
ALLOWED = {
    "common": {"common"},
    "db-interface": {"common", "db-interface"},
    "db": {"common", "db-interface", "db"},
    "vcs-interface": {"common", "vcs-interface"},
    "vcs": {"common", "vcs-interface", "vcs"},
    "lib": {"common", "db-interface", "vcs-interface", "lib"},
    "boot": {"common", "db-interface", "db", "vcs-interface", "vcs", "lib", "boot"},
    "cli": {"common", "lib", "boot", "cli"},
    "api": {"common", "lib", "boot", "api"},
}


def layer_of(module):
    """The layer of a module: its folder, with the db and vcs interfaces apart from their implementations; `legacy`
    for a flat module from before the layers."""
    parts = module.split(".")
    if len(parts) == 1:
        return "legacy"
    if parts[0] in ("db", "vcs") and parts[1] == "interface":
        return parts[0] + "-interface"
    if parts[0] not in {"common", "db", "vcs", "lib", "boot", "cli", "api"}:
        raise BuildError(f"{module}: {parts[0]}/ isn't a layer's folder")
    return parts[0]


def resolve(importer, node):
    """The module a relative import names, from the importing module's package."""
    package = importer.split(".")[:-1]
    if node.level - 1 > len(package):
        raise BuildError(f"{importer}: an import goes above atm/")
    base = package[: len(package) - (node.level - 1)]
    if not node.module:
        raise BuildError(f"{importer}: 'from . import x' between modules doesn't survive the build")
    return ".".join(base + [node.module])


def module_file(name):
    return name.replace(".", "/") + ".py"


def load_layers():
    with open(LAYERS_FILE, encoding="utf-8") as f:
        layers = json.load(f)
    return set(layers.get("legacy", [])), {k: set(v) for k, v in layers.get("exceptions", {}).items()}


def check_layers(imports):
    """The layer rule and the ratchet. `imports` maps each module to the modules it imports from."""
    legacy, exceptions = load_layers()
    flat = {m for m in MODULES if layer_of(m) == "legacy"}
    for m in sorted(flat - legacy):
        raise BuildError(f"{m}.py is a new module outside the layers: put it in a layer's folder (docs/layered-architecture.md §8)")
    for m in sorted(legacy - flat):
        raise BuildError(f"atm-cli/layers.json lists {m} as legacy, but there is no {m}.py any more: remove it")
    used = set()
    for importer, targets in imports.items():
        layer = layer_of(importer)
        if layer == "legacy":
            continue  # the old modules may use anything until they move
        for target in sorted(targets):
            if layer_of(target) in ALLOWED[layer]:
                continue
            if target in exceptions.get(importer, set()):
                used.add((importer, target))
                continue
            raise BuildError(
                f"{module_file(importer)} imports from {target}, which the {layer} layer may not use "
                f"(docs/layered-architecture.md §1)"
            )
    for importer, targets in sorted(exceptions.items()):
        for target in sorted(targets):
            if (importer, target) not in used:
                raise BuildError(
                    f"atm-cli/layers.json allows {importer} to import from {target}, which it no longer does (or "
                    "may): remove the entry"
                )
HEADER = (
    "# Generated from atm-cli/atm/ by atm-cli/build.py. Don't edit this file: edit the modules and run\n"
    "# `npm run build:cli` in dashboard/ (the pre-commit hook does it too).\n"
)


class BuildError(Exception):
    pass


def docstring_node(tree):
    first = tree.body[0] if tree.body else None
    if isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str):
        return first
    return None


def top_level_names(tree):
    """Names a module defines at the top level (functions, classes, assigned names)."""
    names = []
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            names.append(node.name)
        elif isinstance(node, ast.Assign):
            for target in node.targets:
                names.extend(n.id for n in ast.walk(target) if isinstance(n, ast.Name))
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.append(node.target.id)
    return names


def read_module(name):
    path = os.path.join(PACKAGE, module_file(name))
    with open(path, encoding="utf-8") as f:
        source = f.read()
    tree = ast.parse(source, path)
    return source, tree


def build():
    """Return the text of the built atm.py."""
    with open(os.path.join(PACKAGE, "__init__.py"), encoding="utf-8") as f:
        init_tree = ast.parse(f.read())
    package_doc = docstring_node(init_tree)
    if package_doc is None:
        raise BuildError("atm-cli/atm/__init__.py needs a docstring: it becomes the built file's docstring")

    defined = {}  # name -> module that defines it
    plain_imports, from_imports = set(), {}
    module_imports = {}  # module -> the modules it imports from
    sections = []
    for index, name in enumerate(MODULES):
        module_imports[name] = set()
        source, tree = read_module(name)
        lines = source.splitlines()
        drop = set()  # 1-based line numbers left out of the body

        doc = docstring_node(tree)
        comment = []
        if doc:
            drop.update(range(doc.lineno, doc.end_lineno + 1))
            comment = ["# " + line if line.strip() else "#" for line in doc.value.value.strip().splitlines()]

        for node in tree.body:
            if isinstance(node, ast.Import):
                plain_imports.update(
                    alias.name + (f" as {alias.asname}" if alias.asname else "") for alias in node.names
                )
            elif isinstance(node, ast.ImportFrom) and node.level == 0:
                from_imports.setdefault(node.module, set()).update(
                    alias.name + (f" as {alias.asname}" if alias.asname else "") for alias in node.names
                )
            elif isinstance(node, ast.ImportFrom):
                source_module = resolve(name, node)
                if source_module not in MODULES[:index]:
                    raise BuildError(f"{module_file(name)} imports from {source_module}, which isn't an earlier module")
                module_imports[name].add(source_module)
                for alias in node.names:
                    if alias.asname:
                        raise BuildError(f"{module_file(name)}: 'import ... as' between modules doesn't survive the build")
                    if defined.get(alias.name) != source_module:
                        raise BuildError(
                            f"{module_file(name)} imports {alias.name} from {source_module}, which doesn't define it"
                        )
            else:
                continue
            drop.update(range(node.lineno, node.end_lineno + 1))

        for defined_name in top_level_names(tree):
            if defined_name in defined:
                raise BuildError(
                    f"{defined_name} is defined in both {module_file(defined[defined_name])} and {module_file(name)}"
                )
            defined[defined_name] = name

        body = [line for number, line in enumerate(lines, 1) if number not in drop]
        while body and not body[0].strip():
            body.pop(0)
        while body and not body[-1].strip():
            body.pop()
        sections.append("\n".join([f"# ==== {module_file(name)} ===="] + comment + ([""] if comment else []) + body))

    check_layers(module_imports)

    imports = [f"import {name}" for name in sorted(plain_imports)]
    imports += [f"from {module} import {', '.join(sorted(names))}" for module, names in sorted(from_imports.items())]
    docstring = '"""' + package_doc.value.value + '"""'
    return HEADER + docstring + "\n\n" + "\n".join(imports) + "\n\n\n" + "\n\n\n".join(sections) + "\n"


def main(argv):
    try:
        text = build()
    except BuildError as e:
        sys.stderr.write(f"build.py: {e}\n")
        return 1
    if "--check" in argv:
        try:
            with open(TARGET, encoding="utf-8") as f:
                current = f.read()
        except OSError:
            current = None
        if current != text:
            sys.stderr.write(
                "build.py: plugin/scripts/atm.py is stale. Run `npm run build:cli` in dashboard/ and commit it.\n"
            )
            return 1
        return 0
    target = argv[argv.index("--output") + 1] if "--output" in argv else TARGET
    try:
        with open(target, encoding="utf-8") as f:
            if f.read() == text:
                return 0
    except OSError:
        pass
    with open(target, "w", encoding="utf-8") as f:
        f.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
