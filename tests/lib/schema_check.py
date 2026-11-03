"""Checking data against common's JSON Schemas, for the tests: the subset of JSON Schema the schemas use (types,
`required`, array `items`, string `enum`s, `$ref`s to `$defs`, maps (`additionalProperties`) and nullable types
(`"type": [T, "null"]`, `anyOf` with null)). Unknown properties are allowed, as in the files."""

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
        ref = schema["$ref"]
        if not ref.startswith("#"):  # another schema file
            other = load_schema(ref)
            return problems(value, other, other, where, required)
        return problems(value, root["$defs"][ref.rsplit("/", 1)[-1]], root, where, required)
    if "anyOf" in schema:  # one of these, or null (the generator's nullable form)
        tries = [problems(value, s, root, where, required) for s in schema["anyOf"]]
        return [] if any(not t for t in tries) else min(tries, key=len)
    found = []
    kind = schema.get("type")
    if isinstance(kind, list):  # a type, or null
        if value is None and "null" in kind:
            return []
        kind = next(k for k in kind if k != "null")
    if kind == "null":
        return [] if value is None else [f"{where}: expected null, got {type(value).__name__}"]
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
        if isinstance(schema.get("additionalProperties"), dict):  # a map: every value of one type
            for key, item in value.items():
                found += problems(item, schema["additionalProperties"], root, f"{where}.{key}", required)
    if kind == "array" and "items" in schema:
        for i, item in enumerate(value):
            found += problems(item, schema["items"], root, f"{where}[{i}]", required)
    return found
