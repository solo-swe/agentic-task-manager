import { describe, expect, it } from 'vitest';
import { completedAt, completions, formatDate, formatDateTime } from '../../src/lib/timeline';
import type { Task } from '../../src/lib/types';
import { withKinds } from './views';

const done = (id: string, updated_at?: string): Task => ({
  id,
  name: `Task ${id}`,
  phase: 0,
  type: 'feature',
  description: '',
  status: 'done',
  updated_at,
});

describe('completions', () => {
  it('keeps done tasks with valid timestamps, oldest first', () => {
    const tasks = [
      done('b', '2026-04-02T00:00:00Z'),
      done('a', '2026-04-01T00:00:00Z'),
      done('none'),
      done('bad', 'not a date'),
      { ...done('todo', '2026-04-01T00:00:00Z'), status: 'todo' },
    ];
    expect(completions(withKinds(tasks)).map((c) => c.id)).toEqual(['a', 'b']);
  });
});

describe('completion time', () => {
  it('prefers completed_at over a later updated_at', () => {
    const tasks = [
      { ...done('a', '2026-04-09T00:00:00Z'), completed_at: '2026-04-01T00:00:00Z' },
      done('b', '2026-04-02T00:00:00Z'),
    ];
    expect(completions(withKinds(tasks)).map((c) => [c.id, new Date(c.time).toISOString()])).toEqual([
      ['a', '2026-04-01T00:00:00.000Z'],
      ['b', '2026-04-02T00:00:00.000Z'],
    ]);
  });

  it('falls back to updated_at for older files', () => {
    expect(completedAt(done('a', '2026-04-03T00:00:00Z'))).toBe('2026-04-03T00:00:00Z');
    expect(completedAt({ ...done('b'), completed_at: '2026-04-04T00:00:00Z' })).toBe('2026-04-04T00:00:00Z');
  });
});

describe('date formatting', () => {
  it('formats in the runtime locale', () => {
    const ts = Date.parse('2026-04-03T11:00:00Z');
    expect(formatDate(ts)).toBe(new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }));
    expect(formatDateTime(ts)).toContain(formatDate(ts));
  });
});
