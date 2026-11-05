"""Build plugin/scripts/atm.py from the modules in atm-cli/atm/.

The plugin ships one readable file (users install by cloning; nothing is built on their side), while development
happens in modules. The build concatenates the modules in dependency order under a banner each, hoists and
deduplicates their standard-library imports, and drops their imports of each other, since everything ends up in one
namespace. It refuses to build when two modules define the same top-level name (concatenation would silently shadow
one), or when a module imports from a later module or a name another module doesn't define.

It also holds the layer rule (docs/layered-architecture.md §1, §8): a module's layer is its folder (`db/jsongit/store.py`
is `db.jsongit.store`, in the db layer), and an import the rule doesn't allow is refused, as is a module outside the
layers' folders. The rule has no exceptions (the migration's ratchet, atm-cli/layers.json, went with its last step).

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
from typing import List

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE = os.path.join(HERE, "atm")
TARGET = os.path.join(os.path.dirname(HERE), "plugin", "scripts", "atm.py")
SCHEMA_DIR = os.path.join(PACKAGE, "common", "schema")
PY_TYPES = os.path.join(PACKAGE, "common", "types.py")
TS_TYPES = os.path.join(os.path.dirname(HERE), "dashboard", "src", "lib", "generated", "types.ts")
# Dependency order: a module may only import from modules before it. A dotted name is a module in a layer's folder.
MODULES = [
    "common.errors",
    "common.vocabularies",
    "common.core",
    "common.dashboard",
    "common.model",
    "common.fields",
    "common.types",
    "common.components",
    "common.sprint_names",
    "common.repository",
    "host.git",
    "db.interface",
    "vcs.interface",
    "db.jsongit.gitrun",
    "db.jsongit.files",
    "db.jsongit.paths",
    "db.jsongit.lock",
    "db.jsongit.worktrees",
    "db.jsongit.claims",
    "db.jsongit.logo",
    "db.jsongit.merge",
    "db.jsongit.migrate",
    "db.registry",
    "db.working",
    "db.jsongit.store",
    "db.memory",
    "vcs.git",
    "lib.project",
    "lib.state",
    "lib.loop",
    "lib.tasks",
    "lib.query",
    "lib.status",
    "lib.claims",
    "lib.edit",
    "lib.components",
    "lib.sprints",
    "lib.releases",
    "lib.archive",
    "lib.code",
    "lib.projects",
    "lib.settings",
    "lib.worktrees",
    "lib.documents",
    "boot.identity",
    "boot.open",
    "boot.jsongit",
    "cli.render",
    "cli.query",
    "cli.status",
    "cli.claims",
    "cli.edit",
    "cli.components",
    "cli.sprints",
    "cli.releases",
    "cli.archive",
    "cli.show",
    "cli.code",
    "cli.projects",
    "cli.settings",
    "cli.store",
    "cli.serve",
    "cli.main",
    "api.server",
]

# Third-party packages to vendor into the built plugin/scripts/atm.py.
# Populated by T-416/T-417 when third-party libraries (transitions, ...) are adopted.
# Each entry is a package name whose source lives in VENDORED_DIR/<package>/.
# The build reads each package's .py files, hoists its imports, and concatenates
# its code ahead of the atm modules (so atm modules can import from them).
VENDORED_PACKAGES: List[str] = ["six", "transitions"]

# Directory containing vendored third-party source code.
# T-416 will copy click, transitions, etc. source here (e.g. vendored/click/, vendored/transitions/).
VENDORED_DIR = os.path.join(HERE, "vendored")

# The end of the built file: the one entry, as a process starts it. `--server` (plugin/scripts/atm-serve) runs the
# REST API's server process; anything else is the CLI (plugin/scripts/atm). Every module is in one namespace there,
# so this is the only place the two meet, and neither imports the other (docs/layered-architecture.md §11.1).
ENTRY = """
if __name__ == "__main__":
    if sys.argv[1:2] == [SERVER_ENTRY]:
        sys.exit(serve_main(sys.argv[2:]))
    sys.exit(main(sys.argv[1:]))
"""


def module_file(name):
    return name.replace(".", "/") + ".py"


def resolve(importer, node):
    """The module a relative import names, from the importing module's package."""
    package = importer.split(".")[:-1]
    if node.level - 1 > len(package):
        raise BuildError(f"{importer}: an import goes above atm/")
    base = package[: len(package) - (node.level - 1)]
    if not node.module:
        raise BuildError(f"{importer}: 'from . import x' between modules doesn't survive the build")
    return ".".join(base + [node.module])
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

    # Vendored third-party packages: concatenate their code ahead of the atm modules
    # so atm modules can import from them. Each package is flattened into one section:
    # submodule files first, __init__.py last, relative imports stripped (names are
    # already in the shared flat namespace). Vendored imports stay inline.
    for package in VENDORED_PACKAGES:
        pkg_dir = os.path.join(VENDORED_DIR, package)
        if not os.path.isdir(pkg_dir):
            raise BuildError(f"vendored package {package!r} not found at {pkg_dir}")

        # Collect all .py files, __init__.py last
        py_files = []
        for root, dirs, files in os.walk(pkg_dir):
            dirs.sort()
            for fname in sorted(files):
                if fname.endswith(".py"):
                    py_files.append(os.path.join(root, fname))
        py_files.sort(key=lambda p: (os.path.basename(p) != "__init__.py", p))

        package_body = []
        for fpath in py_files:
            rel = os.path.relpath(fpath, VENDORED_DIR)
            dotted = rel.replace(os.sep, ".")[:-3]
            parts = dotted.split(".")
            if len(parts) == 2 and parts[0] == parts[1]:
                dotted = parts[0]
            pkg_root = dotted.split(".")[0]

            with open(fpath, encoding="utf-8") as f:
                source = f.read()

            # Strip relative imports: in a flat namespace, submodule names are already
            # defined by earlier files in this package section.
            source = re.sub(
                r"^\s*from \.+([A-Za-z_]\w*)( import .*)?$",
                lambda m: f"# (vendored) from .{m.group(1)} imported inline",
                source,
                flags=re.MULTILINE,
            )
            source = re.sub(
                r"^\s*import \.+([A-Za-z_]\w*)$",
                lambda m: f"# (vendored) import .{m.group(1)} imported inline",
                source,
                flags=re.MULTILINE,
            )

            tree = ast.parse(source, fpath)
            lines = source.splitlines()
            drop = set()

            doc = docstring_node(tree)
            comment = []
            if doc:
                drop.update(range(doc.lineno, doc.end_lineno + 1))
                comment = ["# " + line if line.strip() else "#" for line in doc.value.value.strip().splitlines()]

            # Vendored code stays inline (not hoisted): drop only docstrings and
            # __future__ imports (hoisted to top). Keep all other definitions inline
            # so they execute in section order, making names available to later files.
            for node in tree.body:
                if isinstance(node, ast.ImportFrom) and node.level == 0 and node.module == "__future__":
                    future_names = from_imports.setdefault("__future__", set())
                    future_names.update(
                        alias.name + (f" as {alias.asname}" if alias.asname else "") for alias in node.names
                    )
                    drop.update(range(node.lineno, node.end_lineno + 1))
                elif isinstance(node, ast.Import):
                    # Drop imports from vendored packages; keep stdlib imports inline
                    for alias in node.names:
                        if alias.name.split(".")[0] in VENDORED_PACKAGES:
                            drop.update(range(node.lineno, node.end_lineno + 1))
                            break
                elif isinstance(node, ast.ImportFrom) and node.level == 0:
                    if node.module in VENDORED_PACKAGES:
                        # Drop imports from vendored packages: names are already global
                        drop.update(range(node.lineno, node.end_lineno + 1))
                elif isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
                    # Bare docstring: drop
                    drop.update(range(node.lineno, node.end_lineno + 1))
                # else: keep inline (assignments, function defs, class defs, etc.)

            for defined_name in top_level_names(tree):
                existing = defined.get(defined_name)
                if existing is not None and not existing.startswith("vendored "):
                    raise BuildError(
                        f"{defined_name} is defined in both {module_file(existing)} "
                        f"and vendored {dotted}"
                    )
                defined[defined_name] = f"vendored {dotted}"

            body = [line for number, line in enumerate(lines, 1) if number not in drop]
            while body and not body[0].strip():
                body.pop(0)
            while body and not body[-1].strip():
                body.pop()
            package_body.append(
                "\n".join(
                    [f"# ==== vendored {dotted} ===="] + comment + ([""] if comment else []) + body
                )
            )

        sections.append("\n\n\n".join(package_body))

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
                for alias in node.names:
                    mod = alias.name.split(".")[0]
                    if mod not in VENDORED_PACKAGES:
                        plain_imports.add(
                            alias.name + (f" as {alias.asname}" if alias.asname else "")
                        )
                # Always drop the import line: vendored names are already global,
                # stdlib names are hoisted above.
                drop.update(range(node.lineno, node.end_lineno + 1))
            elif isinstance(node, ast.ImportFrom) and node.level == 0:
                if node.module == "__future__":
                    # __future__ imports must be hoisted to the top
                    future_names = from_imports.setdefault("__future__", set())
                    future_names.update(
                        alias.name + (f" as {alias.asname}" if alias.asname else "") for alias in node.names
                    )
                    drop.update(range(node.lineno, node.end_lineno + 1))
                elif node.module in VENDORED_PACKAGES:
                    # Drop vendored imports: names are already global in flat namespace
                    drop.update(range(node.lineno, node.end_lineno + 1))
                else:
                    from_imports.setdefault(node.module, set()).update(
                        alias.name + (f" as {alias.asname}" if alias.asname else "") for alias in node.names
                    )
                    drop.update(range(node.lineno, node.end_lineno + 1))
            elif isinstance(node, ast.ImportFrom):
                source_module = resolve(name, node)
                # Import order: a module may only import from earlier modules (concatenation order).
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
                drop.update(range(node.lineno, node.end_lineno + 1))
            else:
                continue

        for defined_name in top_level_names(tree):
            existing = defined.get(defined_name)
            if existing is not None and not existing.startswith("vendored "):
                raise BuildError(
                    f"{defined_name} is defined in both {module_file(existing)} "
                    f"and {module_file(name)}"
                )
            defined[defined_name] = name

        body = [line for number, line in enumerate(lines, 1) if number not in drop]
        while body and not body[0].strip():
            body.pop(0)
        while body and not body[-1].strip():
            body.pop()
        sections.append(
            "\n".join(
                [f"# ==== {module_file(name)} ===="] + comment + ([""] if comment else []) + body
            )
        )

    # __future__ imports must come first in Python (before any other import)
    future_names = from_imports.pop("__future__", None)
    imports = [f"import {name}" for name in sorted(plain_imports)]
    if future_names:
        imports.insert(
            0, f"from __future__ import {', '.join(sorted(future_names))}"
        )
    imports += [f"from {module} import {', '.join(sorted(names))}" for module, names in sorted(from_imports.items())]
    docstring = '"""' + package_doc.value.value + '"""'
    for name in ("SERVER_ENTRY", "serve_main", "main"):
        if name not in defined:
            raise BuildError(f"the entry needs {name}, which no module defines")
    return HEADER + docstring + "\n\n" + "\n".join(imports) + "\n\n\n" + "\n\n\n".join(sections) + "\n\n" + ENTRY


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
    types, arrays, string enums, $refs to the types the schemas define, maps (`additionalProperties` of one type), and
    any of these that may be null (`"type": [T, "null"]`, or `"anyOf": [<one of these>, {"type": "null"}]`)."""
    kinds = prop.get("type")
    alternatives = prop.get("anyOf")
    if isinstance(kinds, list) or alternatives is not None:
        if isinstance(kinds, list):
            others = [k for k in kinds if k != "null"]
            if len(kinds) != 2 or len(others) != 1:
                raise BuildError(f"common/schema: {where}: a list of types is one type and null")
            inner = {**prop, "type": others[0]}
        else:
            others = [a for a in alternatives if a != {"type": "null"}]
            if len(alternatives) != 2 or len(others) != 1:
                raise BuildError(f"common/schema: {where}: anyOf is one type and {{'type': 'null'}}")
            inner = others[0]
        inner_type = property_type(inner, where, ts)
        return f"{inner_type} | null" if ts else f"Optional[{inner_type}]"
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
    if kind is None:
        # A value of more than one shape (the schema says which in its description)
        return "unknown" if ts else "object"
    scalars ={"string": ("string", "str"), "integer": ("number", "int"), "number": ("number", "float"), "boolean": ("boolean", "bool")}
    if kind in scalars:
        return scalars[kind][0 if ts else 1]
    if kind == "array":
        item = property_type(prop.get("items", {}), where, ts)
        return (f"({item})[]" if " | " in item else f"{item}[]") if ts else f"List[{item}]"
    if kind == "object" and "properties" not in prop:
        values = prop.get("additionalProperties")
        if isinstance(values, dict):
            # A map from open keys to values of one type (atm status's counts by status, sprint, type and label)
            value = property_type(values, where, ts)
            return f"Record<string, {value}>" if ts else f"Dict[str, {value}]"
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
            used.update(t for t in ("Dict", "List", "Literal", "Optional") if t + "[" in annotation)
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


VOCABULARIES = os.path.join(SCHEMA_DIR, "vocabularies.json")
PY_VOCABULARIES = os.path.join(PACKAGE, "common", "vocabularies.py")
TS_VOCABULARIES = os.path.join(os.path.dirname(TS_TYPES), "vocabularies.ts")
VOCABULARY_NAMES = (  # key in vocabularies.json -> the constant's name in both languages
    ("kinds", "KINDS"),
    ("statuses", "DEFAULT_STATUSES"),
    ("types", "DEFAULT_TYPES"),
    ("priorities", "DEFAULT_PRIORITIES"),
    ("default_priority", "DEFAULT_PRIORITY"),
)


def vocabulary_data():
    with open(VOCABULARIES, encoding="utf-8") as f:
        data = json.load(f)
    missing = [key for key, _ in VOCABULARY_NAMES if key not in data]
    if missing:
        raise BuildError(f"common/schema/vocabularies.json: missing {', '.join(missing)}")
    return data


def generated_python_vocabularies(data):
    lines = [
        '"""The status kinds and the default vocabularies, generated from common/schema/vocabularies.json by',
        f'atm-cli/build.py: don\'t edit it, change that file and run {GENERATED_HOW}"""',
        "",
    ]
    for key, name in VOCABULARY_NAMES:
        value = tuple(data[key]) if key == "kinds" else data[key]
        lines.append(f"{name} = {json.dumps(value, indent=4, ensure_ascii=False) if not isinstance(value, tuple) else repr(value)}")
    return "\n".join(lines) + "\n"


def generated_typescript_vocabularies(data):
    kinds = " | ".join(f"'{k}'" for k in data["kinds"])
    term = lambda t: "{ " + ", ".join(f"{k}: '{v}'" for k, v in t.items()) + " }"
    lines = [
        "// The status kinds and the default vocabularies, generated from atm-cli/atm/common/schema/vocabularies.json by",
        f"// atm-cli/build.py: don't edit it, change that file and run {GENERATED_HOW}",
        "",
        "/** What a status means; everything acts on kinds, never on status names. */",
        f"export type StatusKind = {kinds};",
        "export const KINDS: readonly StatusKind[] = [" + ", ".join(f"'{k}'" for k in data["kinds"]) + "];",
        "",
        "export const DEFAULT_STATUSES: { id: string; label: string; kind: StatusKind }[] = [",
        *[f"  {term(s)}," for s in data["statuses"]],
        "];",
        "export const DEFAULT_TYPES: { id: string; label: string }[] = [",
        *[f"  {term(t)}," for t in data["types"]],
        "];",
        "export const DEFAULT_PRIORITIES: { id: string; label: string }[] = [",
        *[f"  {term(p)}," for p in data["priorities"]],
        "];",
        f"export const DEFAULT_PRIORITY = '{data['default_priority']}';",
    ]
    return "\n".join(lines) + "\n"


def generated_files():
    """The files generated from the schemas and the vocabularies, by path."""
    types = schema_types()
    vocabularies = vocabulary_data()
    return {
        PY_TYPES: generated_python(types),
        TS_TYPES: generated_typescript(types),
        PY_VOCABULARIES: generated_python_vocabularies(vocabularies),
        TS_VOCABULARIES: generated_typescript_vocabularies(vocabularies),
    }


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
