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

It generates the data model's types too (docs/layered-architecture.md §4): from the JSON Schemas in
atm-cli/atm/common/schema/, the Python TypedDicts in atm-cli/atm/common/types.py and the TypeScript interfaces in
dashboard/src/lib/generated/types.ts. One generator, in the standard library, for both languages.

Usage: python3 atm-cli/build.py           write the generated types and plugin/scripts/atm.py (only if they changed)
       python3 atm-cli/build.py --check   exit 1 if any of them is not what the sources build
       python3 atm-cli/build.py --output F   write the build to F instead (the generated types are left alone)
Design: docs/v4/atm-cli.md, section 2.
"""

import ast
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE = os.path.join(HERE, "atm")
LAYERS_FILE = os.path.join(HERE, "layers.json")
TARGET = os.path.join(os.path.dirname(HERE), "plugin", "scripts", "atm.py")
SCHEMA_DIR = os.path.join(PACKAGE, "common", "schema")
PY_TYPES = os.path.join(PACKAGE, "common", "types.py")
TS_TYPES = os.path.join(os.path.dirname(HERE), "dashboard", "src", "lib", "generated", "types.ts")
# Dependency order: a module may only import from modules before it. A dotted name is a module in a layer's folder.
MODULES = [
    "common.errors",
    "core",
    "paths",
    "lock",
    "registry",
    "model",
    "common.fields",
    "common.types",
    "db.interface",
    "vcs.interface",
    "components",
    "sprint_names",
    "repository",
    "logo",
    "claims",
    "db.jsongit.store",
    "db.memory",
    "vcs.git",
    "lib.project",
    "lib.tasks",
    "lib.query",
    "boot.open",
    "cli.render",
    "tasks",
    "edit",
    "sprints",
    "releases",
    "merge",
    "archive",
    "git_ops",
    "migrate",
    "api.server",
    "cli.query",
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


GENERATED = "Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run"
GENERATED_HOW = "`npm run build:cli` in dashboard/ (the pre-commit hook does it too)."


FILE_TYPES = {}  # schema file -> the type it defines (its title), for $refs between files


def schema_files():
    """The schemas, by file name, with a file another one refers to before it (task.schema.json before
    api.schema.json, so the types each language defines come before their use)."""
    schemas = {}
    for file in sorted(f for f in os.listdir(SCHEMA_DIR) if f.endswith(".schema.json")):
        with open(os.path.join(SCHEMA_DIR, file), encoding="utf-8") as f:
            schemas[file] = json.load(f)
    refers = lambda schema: {r for r in re.findall(r'"\$ref": "([^"#]+)"', json.dumps(schema))}
    ordered = []
    while len(ordered) < len(schemas):
        ready = [f for f in schemas if f not in ordered and refers(schemas[f]) <= set(ordered)]
        if not ready:
            raise BuildError("common/schema: the schemas refer to each other in a cycle")
        ordered.append(ready[0])
    return [(f, schemas[f]) for f in ordered]


def schema_types():
    """The object types the schemas define, in order: each file's $defs, then the file's own type (its title)."""
    types, seen = [], set()
    for file, schema in schema_files():
        FILE_TYPES[file] = schema["title"]
        for name, definition in list(schema.get("$defs", {}).items()) + [(schema["title"], schema)]:
            if name in seen:
                raise BuildError(f"common/schema/{file}: the type {name} is defined twice")
            if definition.get("type") != "object":
                raise BuildError(f"common/schema/{file}: {name} isn't an object type")
            seen.add(name)
            types.append((name, definition))
    return types


def property_type(prop, where, ts):
    """A property's type in TypeScript (`ts`) or Python. The schemas use a small subset of JSON Schema: the scalar
    types, arrays, string enums and $refs to the types the schemas define."""
    if "$ref" in prop:
        ref = prop["$ref"]
        if not ref.startswith("#"):
            if ref not in FILE_TYPES:
                raise BuildError(f"common/schema: {where} refers to {ref}, which isn't one of the schemas")
            name = FILE_TYPES[ref]
        else:
            name = ref.rsplit("/", 1)[-1]
        # In Python, a forward reference: the type may be defined further down
        return name if ts else f'"{name}"'
    if "enum" in prop:
        values = [f"'{v}'" for v in prop["enum"]]
        return " | ".join(values) if ts else f"Literal[{', '.join(values)}]"
    kind = prop.get("type")
    scalars = {"string": ("string", "str"), "integer": ("number", "int"), "number": ("number", "float"), "boolean": ("boolean", "bool")}
    if kind in scalars:
        return scalars[kind][0 if ts else 1]
    if kind == "array":
        item = property_type(prop.get("items", {}), where, ts)
        return (f"({item})[]" if " | " in item else f"{item}[]") if ts else f"List[{item}]"
    if kind == "object" and "properties" not in prop:
        # An object whose keys the schema leaves open (meta, which its own schema will describe)
        return "Record<string, unknown>" if ts else "dict"
    raise BuildError(f"common/schema: {where} has a type the generator doesn't know (define objects in $defs)")


def generated_typescript(types):
    lines = [f"// {GENERATED}", f"// {GENERATED_HOW}"]
    for name, definition in types:
        lines.append("")
        if definition.get("description"):
            lines.append(f"/** {definition['description']} */")
        lines.append(f"export interface {name} {{")
        required = set(definition.get("required", []))
        for prop, spec in definition.get("properties", {}).items():
            if spec.get("description"):
                lines.append(f"  /** {spec['description']} */")
            optional = "" if prop in required else "?"
            lines.append(f"  {prop}{optional}: {property_type(spec, f'{name}.{prop}', True)};")
        lines.append("}")
    return "\n".join(lines) + "\n"


def generated_python(types):
    # The functional form: keys such as `from` and `with` are Python keywords
    body, used = [], set()
    for name, definition in types:
        body += [""]
        if definition.get("description"):
            body.append(f"# {definition['description']}")
        body.append(f'{name} = TypedDict(')
        body.append(f'    "{name}",')
        body.append("    {")
        required = set(definition.get("required", []))
        for prop, spec in definition.get("properties", {}).items():
            annotation = property_type(spec, f"{name}.{prop}", False)
            used.update(t for t in ("List", "Literal") if t + "[" in annotation)
            notes = (["required"] if prop in required else []) + ([spec["description"]] if spec.get("description") else [])
            if notes:
                body.append(f"        # {'; '.join(notes)}")
            body.append(f'        "{prop}": {annotation},')
        body.append("    },")
        body.append("    total=False,")
        body.append(")")
    header = [
        f'"""{GENERATED}',
        f"{GENERATED_HOW} TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has",
        'no typing.Required)."""',
        "",
        f"from typing import {', '.join(sorted(used | {'TypedDict'}))}",
    ]
    return "\n".join(header + body) + "\n"


def generated_files():
    """The files generated from the schemas, by path."""
    types = schema_types()
    return {PY_TYPES: generated_python(types), TS_TYPES: generated_typescript(types)}


def read_text(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def main(argv):
    try:
        generated = generated_files()
        if "--check" not in argv and "--output" not in argv:
            for path, content in generated.items():
                if read_text(path) != content:
                    os.makedirs(os.path.dirname(path), exist_ok=True)
                    with open(path, "w", encoding="utf-8") as f:
                        f.write(content)
        text = build()
    except BuildError as e:
        sys.stderr.write(f"build.py: {e}\n")
        return 1
    if "--check" in argv:
        for path, content in generated.items():
            if read_text(path) != content:
                sys.stderr.write(
                    f"build.py: {os.path.relpath(path, os.path.dirname(HERE))} is stale. Run `npm run build:cli` in "
                    "dashboard/ and commit it.\n"
                )
                return 1
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
