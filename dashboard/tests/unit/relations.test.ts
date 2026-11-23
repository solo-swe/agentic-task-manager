import { describe, expect, it } from 'vitest';
import { notDone, relationsOf, resolutionOf } from '../../src/lib/relations';
import type { Task } from '../../src/lib/types';
import { withKinds } from './views';

/** A task as atm serves it: with its kind, and the answers the test gives (`derived`) */
const t = (id: string, status: string, extra: Partial<Task> = {}): Task =>
  withKinds([
    { id, name: id, phase: 0, type: 'feature', description: '', status, dependencies: [], ...extra } as Task,
  ])[0];

/** atm's answers on a task, as a test gives them */
const served = (more: object) => ({ derived: { kind: 'todo', ...more } }) as Partial<Task>;

describe('resolutionOf', () => {
  it("is atm's (ATM-719): none from atm is none, whatever the task stores", () => {
    expect(resolutionOf({ id: 'a', status: 'skipped', ...served({ resolution: 'duplicate' }) } as Task)).toBe(
      'duplicate',
    );
    expect(resolutionOf({ id: 'a', status: 'todo', resolution: 'obsolete', ...served({}) } as Task)).toBeUndefined();
  });
});

describe('relationsOf', () => {
  it('its own related first, never itself, then those atm finds pointing at it (ATM-719)', () => {
    const task = t('T-3', 'todo', { related: ['T-4', 'T-3'], ...served({ related_by: ['T-1'] }) });
    expect(relationsOf(task).related).toEqual(['T-4', 'T-1']);
    expect(relationsOf(t('T-5', 'todo')).related).toEqual([]);
  });

  it('what it duplicates, its own; what duplicates it, as atm says', () => {
    expect(relationsOf(t('T-2', 'skipped', { duplicate_of: 'T-1' }))).toMatchObject({
      duplicateOf: 'T-1',
      duplicatedBy: [],
    });
    expect(relationsOf(t('T-1', 'done', served({ duplicated_by: ['T-2'] })))).toMatchObject({
      duplicateOf: undefined,
      duplicatedBy: ['T-2'],
    });
  });
});

describe('notDone', () => {
  it('finished tasks that weren’t done, by resolution: won’t do, obsolete, duplicate, then any other', () => {
    const tasks = [
      t('a', 'done'),
      t('b', 'skipped'),
      t('c', 'skipped', { resolution: 'duplicate' }),
      t('d', 'done', { resolution: 'obsolete' }),
      t('e', 'skipped'),
      t('f', 'skipped', { resolution: 'later' }),
      t('g', 'todo'),
    ];
    expect(notDone(tasks)).toEqual([
      { resolution: 'wont_do', count: 2 },
      { resolution: 'obsolete', count: 1 },
      { resolution: 'duplicate', count: 1 },
      { resolution: 'later', count: 1 },
    ]);
    expect(notDone([t('a', 'done')])).toEqual([]);
  });
});
