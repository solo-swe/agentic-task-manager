// The dashboard's generated TypeScript (T-477; it was atm-cli/build.py's): the interfaces of the data model's types,
// from atm-common's JSON Schemas (crates/atm-common/schema/*.schema.json), and the status kinds and default
// vocabularies (its vocabularies.json), into .generated/dashboard/ (never committed). `pnpm run generate` runs it.
import { mkdirSync, readdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { GENERATED } from './generated.ts';

export const SCHEMA_DIR = fileURLToPath(new URL('../../crates/atm-common/schema', import.meta.url));

const HEADER = [
  "// Generated from crates/atm-common/schema/ by dashboard/build/generate.ts: don't edit it, change the schema and run",
  '// `pnpm run generate` in dashboard/ (every script that needs it runs it first).',
];

type Schema = { [key: string]: unknown };

export class GenerateError extends Error {}

/** The schemas' `$ref`s to other files (`task.schema.json`), anywhere in a schema. */
function fileRefs(value: unknown, found = new Set<string>()): Set<string> {
  if (Array.isArray(value)) for (const item of value) fileRefs(item, found);
  else if (value && typeof value === 'object')
    for (const [key, item] of Object.entries(value)) {
      if (key === '$ref' && typeof item === 'string' && !item.startsWith('#')) found.add(item);
      else fileRefs(item, found);
    }
  return found;
}

/** The schemas, by file name, with a file another one refers to before it (task.schema.json before api.schema.json,
 * so a type is defined before its use). */
export function orderSchemas(schemas: Record<string, Schema>): [string, Schema][] {
  const names = Object.keys(schemas).sort();
  const ordered: string[] = [];
  while (ordered.length < names.length) {
    const ready = names.find(
      (f) => !ordered.includes(f) && [...fileRefs(schemas[f])].every((r) => ordered.includes(r)),
    );
    if (!ready) throw new GenerateError('common/schema: the schemas refer to each other in a cycle');
    ordered.push(ready);
  }
  return ordered.map((f) => [f, schemas[f]]);
}

/** The object types the schemas define, in order: each file's $defs, then the file's own type (its title); and the
 * type each file defines, for $refs between files. */
export function schemaTypes(schemas: Record<string, Schema>): {
  types: [string, Schema][];
  files: Map<string, string>;
} {
  const types: [string, Schema][] = [];
  const files = new Map<string, string>();
  const seen = new Set<string>();
  for (const [file, schema] of orderSchemas(schemas)) {
    files.set(file, schema.title as string);
    const defs = Object.entries((schema.$defs ?? {}) as Record<string, Schema>);
    for (const [name, definition] of [...defs, [schema.title as string, schema] as [string, Schema]]) {
      if (seen.has(name)) throw new GenerateError(`common/schema/${file}: the type ${name} is defined twice`);
      if (definition.type !== 'object') throw new GenerateError(`common/schema/${file}: ${name} isn't an object type`);
      seen.add(name);
      types.push([name, definition]);
    }
  }
  return { types, files };
}

const SCALARS: Record<string, string> = { string: 'string', integer: 'number', number: 'number', boolean: 'boolean' };

/** A property's type. The schemas use a small subset of JSON Schema: the scalar types, arrays, string enums, $refs to
 * the types the schemas define, maps (`additionalProperties` of one type), and any of these that may be null
 * (`"type": [T, "null"]`, or `"anyOf": [<one of these>, {"type": "null"}]`). */
export function propertyType(prop: Schema, where: string, files: Map<string, string>): string {
  const kinds = prop.type;
  const alternatives = prop.anyOf as Schema[] | undefined;
  if (Array.isArray(kinds) || alternatives !== undefined) {
    let inner: Schema;
    if (Array.isArray(kinds)) {
      const others = kinds.filter((k) => k !== 'null');
      if (kinds.length !== 2 || others.length !== 1)
        throw new GenerateError(`common/schema: ${where}: a list of types is one type and null`);
      inner = { ...prop, type: others[0] };
    } else {
      const isNull = (a: Schema) => Object.keys(a).length === 1 && a.type === 'null';
      const others = alternatives!.filter((a) => !isNull(a));
      if (alternatives!.length !== 2 || others.length !== 1)
        throw new GenerateError(`common/schema: ${where}: anyOf is one type and {'type': 'null'}`);
      inner = others[0];
    }
    return `${propertyType(inner, where, files)} | null`;
  }
  if (typeof prop.$ref === 'string') {
    const ref = prop.$ref;
    if (ref.startsWith('#')) return ref.split('/').pop()!;
    const name = files.get(ref);
    if (!name) throw new GenerateError(`common/schema: ${where} refers to ${ref}, which isn't one of the schemas`);
    return name;
  }
  if (Array.isArray(prop.enum)) return prop.enum.map((v) => `'${v}'`).join(' | ');
  if (kinds === undefined) return 'unknown'; // a value of more than one shape (the schema says which)
  if (typeof kinds === 'string' && kinds in SCALARS) return SCALARS[kinds];
  if (kinds === 'array') {
    const item = propertyType((prop.items ?? {}) as Schema, where, files);
    return item.includes(' | ') ? `(${item})[]` : `${item}[]`;
  }
  if (kinds === 'object' && !('properties' in prop)) {
    const values = prop.additionalProperties;
    // A map from open keys to values of one type (atm status's counts by status, sprint, type and label)
    if (values && typeof values === 'object') return `Record<string, ${propertyType(values as Schema, where, files)}>`;
    return 'Record<string, unknown>'; // an object whose keys the schema leaves open (meta)
  }
  throw new GenerateError(`common/schema: ${where} has a type the generator doesn't know (define objects in $defs)`);
}

export function typescriptTypes(schemas: Record<string, Schema>): string {
  const { types, files } = schemaTypes(schemas);
  const lines = [...HEADER];
  for (const [name, definition] of types) {
    lines.push('');
    if (definition.description) lines.push(`/** ${definition.description} */`);
    lines.push(`export interface ${name} {`);
    const required = new Set((definition.required ?? []) as string[]);
    for (const [prop, spec] of Object.entries((definition.properties ?? {}) as Record<string, Schema>)) {
      if (spec.description) lines.push(`  /** ${spec.description} */`);
      const optional = required.has(prop) ? '' : '?';
      lines.push(`  ${prop}${optional}: ${propertyType(spec, `${name}.${prop}`, files)};`);
    }
    lines.push('}');
  }
  return lines.join('\n') + '\n';
}

type Term = Record<string, string>;
type Vocabularies = { kinds: string[]; statuses: Term[]; types: Term[]; priorities: Term[]; default_priority: string };

export function typescriptVocabularies(data: Vocabularies): string {
  const missing = ['kinds', 'statuses', 'types', 'priorities', 'default_priority'].filter((k) => !(k in data));
  if (missing.length) throw new GenerateError(`common/schema/vocabularies.json: missing ${missing.join(', ')}`);
  const quoted = data.kinds.map((k) => `'${k}'`);
  const term = (t: Term) =>
    '{ ' +
    Object.entries(t)
      .map(([k, v]) => `${k}: '${v}'`)
      .join(', ') +
    ' }';
  return (
    [
      '// The status kinds and the default vocabularies, generated from crates/atm-common/schema/vocabularies.json by',
      "// dashboard/build/generate.ts: don't edit it, change that file and run `pnpm run generate` in dashboard/.",
      '',
      '/** What a status means; everything acts on kinds, never on status names. */',
      `export type StatusKind = ${quoted.join(' | ')};`,
      `export const KINDS: readonly StatusKind[] = [${quoted.join(', ')}];`,
      '',
      'export const DEFAULT_STATUSES: { id: string; label: string; kind: StatusKind }[] = [',
      ...data.statuses.map((s) => `  ${term(s)},`),
      '];',
      'export const DEFAULT_TYPES: { id: string; label: string }[] = [',
      ...data.types.map((t) => `  ${term(t)},`),
      '];',
      'export const DEFAULT_PRIORITIES: { id: string; label: string }[] = [',
      ...data.priorities.map((p) => `  ${term(p)},`),
      '];',
      `export const DEFAULT_PRIORITY = '${data.default_priority}';`,
    ].join('\n') + '\n'
  );
}

/** Read the schemas in `dir`, by file name. */
export function readSchemas(dir: string): Record<string, Schema> {
  const schemas: Record<string, Schema> = {};
  for (const file of readdirSync(dir).filter((f) => f.endsWith('.schema.json')))
    schemas[file] = JSON.parse(readFileSync(path.join(dir, file), 'utf8'));
  return schemas;
}

/** Write a file only when it changes, in one step: several processes may generate at once, and none may read half a
 * file. */
function writeText(file: string, content: string) {
  try {
    if (readFileSync(file, 'utf8') === content) return;
  } catch {
    // not written yet
  }
  mkdirSync(path.dirname(file), { recursive: true });
  const partial = `${file}.${process.pid}.tmp`;
  writeFileSync(partial, content);
  renameSync(partial, file);
}

/** Generate .generated/dashboard/ from the schemas in `schemaDir`, into `outDir`. */
export function generate(schemaDir = SCHEMA_DIR, outDir = GENERATED) {
  const types = typescriptTypes(readSchemas(schemaDir));
  const vocabularies = typescriptVocabularies(
    JSON.parse(readFileSync(path.join(schemaDir, 'vocabularies.json'), 'utf8')),
  );
  writeText(path.join(outDir, 'types.ts'), types);
  writeText(path.join(outDir, 'vocabularies.ts'), vocabularies);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    generate();
  } catch (e) {
    if (!(e instanceof GenerateError)) throw e;
    process.stderr.write(`generate: ${e.message}\n`);
    process.exit(1);
  }
}
