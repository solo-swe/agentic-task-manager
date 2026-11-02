"""Checking data against common's JSON Schemas, for the tests: the subset of JSON Schema the schemas use (types,
`required`, array `items`, string `enum`s and `$ref`s to `$defs`). Unknown properties are allowed, as in the files."""

import json
import os

from support import REPO

SCHEMA_DIR = os.path.join(REPO, "atm-cli", "atm", "common", "schema")

TYPES = {
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "array": lambda v: isinstance(v, list),
    "object": lambda v: isinstance(v, dict),
}


def load_schema(name):
    with open(os.path.join(SCHEMA_DIR, name), encoding="utf-8") as f:
        return json.load(f)


def problems(value, schema, root=None, where="$", required=True):
    """What is wrong with `value`, as a list of messages (empty when it fits). With `required` false, only the
    fields present are checked (the scenario tests' task files are minimal on purpose)."""
    root = root or schema
    if "$ref" in schema:
        return problems(value, root["$defs"][schema["$ref"].rsplit("/", 1)[-1]], root, where, required)
    found = []
    kind = schema.get("type")
    if kind and not TYPES[kind](value):
        return [f"{where}: expected {kind}, got {type(value).__name__}"]
    if "enum" in schema and value not in schema["enum"]:
        found.append(f"{where}: {value!r} is not one of {schema['enum']}")
    if kind == "object":
        for key in schema.get("required", []) if required else []:
            if key not in value:
                found.append(f"{where}: missing {key!r}")
        for key, spec in schema.get("properties", {}).items():
            if key in value:
                found += problems(value[key], spec, root, f"{where}.{key}", required)
    if kind == "array" and "items" in schema:
        for i, item in enumerate(value):
            found += problems(item, schema["items"], root, f"{where}[{i}]", required)
    return found
