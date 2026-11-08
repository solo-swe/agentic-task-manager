// @vitest-environment node
// build/generate.ts: the dashboard's TypeScript from atm-common's schemas and vocabularies (T-477)
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, test } from 'vitest';
import {
  GenerateError,
  readSchemas,
  SCHEMA_DIR,
  typescriptTypes,
  typescriptVocabularies,
} from '../../build/generate.ts';

const schemas = readSchemas(SCHEMA_DIR);
const types = typescriptTypes(schemas);

describe('generated types', () => {
  test('every field, required or not', () => {
    expect(types).toMatch(/\n {2}id: string;/);
    expect(types).toMatch(/\n {2}notes\?: Note\[\];/);
    expect(types).toMatch(/\n {2}pr\?: PullRequest;/);
  });

  test('a field that may be null, and a map (T-382)', () => {
    expect(types).toMatch(/\n {2}active_sprint: ActiveSprint \| null;/); // anyOf with null
    expect(types).toMatch(/\n {2}recent_sprints: SprintNumbers\[\] \| null;/); // a list of types with null
    expect(types).toMatch(/\n {2}by_status: Record<string, number>;/);
  });

  test('a type is defined before another file refers to it', () => {
    expect(types.indexOf('export interface Task {')).toBeLessThan(types.indexOf('export interface TaskViewList {'));
  });

  test('a new field in a schema is in the types', () => {
    const task = structuredClone(schemas['task.schema.json']) as { properties: Record<string, unknown> };
    task.properties.estimate = { type: 'integer' };
    expect(typescriptTypes({ ...schemas, 'task.schema.json': task })).toMatch(/estimate\?: number;/);
  });

  test('what the generator does not know is refused, with where it is', () => {
    const task = structuredClone(schemas['task.schema.json']) as { properties: Record<string, unknown> };
    task.properties.estimate = { type: ['integer', 'string'] };
    expect(() => typescriptTypes({ ...schemas, 'task.schema.json': task })).toThrow(
      new GenerateError('common/schema: Task.estimate: a list of types is one type and null'),
    );
    expect(() =>
      typescriptTypes({
        'a.schema.json': { title: 'A', type: 'object', properties: { b: { $ref: 'b.schema.json' } } },
        'b.schema.json': { title: 'B', type: 'object', properties: { a: { $ref: 'a.schema.json' } } },
      }),
    ).toThrow('the schemas refer to each other in a cycle');
  });
});

describe('generated vocabularies', () => {
  const data = JSON.parse(readFileSync(path.join(SCHEMA_DIR, 'vocabularies.json'), 'utf8'));

  test('the kinds and the defaults', () => {
    const text = typescriptVocabularies(data);
    expect(text).toMatch(/^export type StatusKind = 'todo' \| 'active' \| /m);
    expect(text).toContain("  { id: 'todo', label: 'To Do', kind: 'todo' },");
    expect(text).toContain(`export const DEFAULT_PRIORITY = '${data.default_priority}';`);
  });

  test('a missing list is refused', () => {
    const missing = { ...data };
    delete missing.types;
    expect(() => typescriptVocabularies(missing)).toThrow('vocabularies.json: missing types');
  });
});
