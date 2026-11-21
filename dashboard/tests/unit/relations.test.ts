import { describe, expect, it } from 'vitest';
import { notDone, relationsOf, resolutionOf } from '../../src/lib/relations';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { vocabulary, withKinds } from './views';

/** A task as atm serves it: with its kind in `vocab` */
const t = (id: string, status: string, extra: Partial<Task> = {}, vocab: Vocabulary = DEFAULT_VOCABULARY): Task =>
  withKinds(
    [{ id, name: id, phase: 0, type: 'feature', description: '', status, dependencies: [], ...extra } as Task],
    vocab,
  )[0];

describe('resolutionOf', () => {
  it('a stored resolution, else the default: done for a done task, won’t do for a skipped one', () => {
    expect(resolutionOf(t('a', 'done'))).toBe('done');
    expect(resolutionOf(t('a', 'done', { resolution: 'obsolete' }))).toBe('obsolete');
    expect(resolutionOf(t('a', 'skipped'))).toBe('wont_do');
    expect(resolutionOf(t('a', 'skipped', { resolution: 'duplicate' }))).toBe('duplicate');
  });

  it('none while the task isn’t finished; by kind in a project’s own statuses', () => {
    expect(resolutionOf(t('a', 'todo', { resolution: 'obsolete' }))).toBeUndefined();
    const vocab = vocabulary({
      statuses: [
        { id: 'open', label: 'Open', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
        { id: 'dropped', label: 'Dropped', kind: 'skipped' },
      ],
    });
    expect(resolutionOf(t('a', 'shipped', {}, vocab))).toBe('done');
    expect(resolutionOf(t('a', 'dropped', {}, vocab))).toBe('wont_do');
  });
});

describe('relationsOf', () => {
  const tasks = [
    t('T-1', 'done', { related: ['T-3'] }),
    t('T-2', 'skipped', { resolution: 'duplicate', duplicate_of: 'T-1' }),
    t('T-3', 'todo', { related: ['T-4', 'T-3'] }),
    t('T-4', 'todo', { related: ['T-3'] }),
  ];

  it('related both ways, its own first, each once, never itself', () => {
    expect(relationsOf(tasks[2], tasks).related).toEqual(['T-4', 'T-1']);
    expect(relationsOf(tasks[0], tasks).related).toEqual(['T-3']);
  });

  it('what it duplicates, and what duplicates it', () => {
    expect(relationsOf(tasks[1], tasks)).toMatchObject({ duplicateOf: 'T-1', duplicatedBy: [] });
    expect(relationsOf(tasks[0], tasks)).toMatchObject({ duplicateOf: undefined, duplicatedBy: ['T-2'] });
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
