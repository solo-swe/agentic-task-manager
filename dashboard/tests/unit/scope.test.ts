import { describe, expect, it } from 'vitest';
import { ACTIVE, BACKLOG } from '../../src/lib/filters';
import { countKinds, lastClosed, scopeOf, scopeTasks } from '../../src/lib/scope';
import { NO_SPRINTS, sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import sprintsFile from '../fixtures/sprints.json';

const file = sprintsFile as TasksFile;
const tasks = file.tasks as Task[];
const vocab = resolveVocabulary(file.meta);
const model = sprintModel(file.meta, tasks, vocab);
const ids = (list: Task[]) => list.map((t) => t.id);
type SprintEntry = { id: string; state: string };
const withStates = (states: Record<string, string>) =>
  sprintModel(
    {
      ...file.meta,
      sprints: (file.meta!.sprints as SprintEntry[]).map((s) => ({ ...s, state: states[s.id] ?? s.state })),
    },
    tasks,
    vocab,
  );

describe('scopeOf (T-323)', () => {
  it('no sprints: no scope', () => {
    expect(scopeOf(NO_SPRINTS, new Set())).toEqual({ kind: 'none' });
  });

  it('nothing selected: all tasks, naming the active sprint', () => {
    expect(scopeOf(model, new Set())).toMatchObject({ kind: 'all', active: { id: 'Ada Lovelace' } });
  });

  it('nothing selected and none active: the last closed and the next planned', () => {
    const scope = scopeOf(withStates({ 'Ada Lovelace': 'closed' }), new Set());
    expect(scope).toMatchObject({ kind: 'all', last: { id: 'Foundations' }, next: { id: 'Alan Turing' } });
    expect(scope).not.toHaveProperty('active');
  });

  it('one sprint: by ACTIVE it follows, by name (in any case) it does not', () => {
    expect(scopeOf(model, new Set([ACTIVE]))).toMatchObject({
      kind: 'sprint',
      sprint: { id: 'Ada Lovelace' },
      follows: true,
    });
    expect(scopeOf(model, new Set(['foundations']))).toMatchObject({
      kind: 'sprint',
      sprint: { id: 'Foundations', state: 'closed' },
      follows: false,
    });
  });

  it('the backlog, and several: in the sprints’ order, the backlog last', () => {
    expect(scopeOf(model, new Set([BACKLOG]))).toEqual({ kind: 'backlog' });
    expect(scopeOf(model, new Set([BACKLOG, ACTIVE, 'foundations']))).toEqual({
      kind: 'several',
      names: ['Foundations', 'Ada Lovelace', 'Backlog'],
    });
  });
});

describe('scopeTasks and countKinds', () => {
  it("the scope's own tasks: all, a sprint (ACTIVE or by name), the backlog", () => {
    expect(scopeTasks(tasks, model, new Set())).toHaveLength(5);
    expect(ids(scopeTasks(tasks, model, new Set([ACTIVE])))).toEqual(['T-002', 'T-003', 'T-004']);
    expect(ids(scopeTasks(tasks, model, new Set(['Foundations', BACKLOG])))).toEqual(['T-001', 'T-005']);
  });

  it('counts by kind; the total leaves the skipped out', () => {
    const counted = countKinds([...tasks, { ...tasks[0], id: 'T-009', status: 'skipped' }], vocab);
    expect(counted).toEqual({ total: 5, done: 1, todo: 3, active: 1, review: 0, blocked: 0, skipped: 1 });
  });
});

describe('lastClosed', () => {
  it('the latest end, else the last closed one listed', () => {
    expect(lastClosed(model)?.id).toBe('Foundations');
    const both = withStates({ 'Ada Lovelace': 'closed' });
    // Ada Lovelace has no end: Foundations, which has one, is taken
    expect(lastClosed(both)?.id).toBe('Foundations');
    expect(lastClosed(withStates({ Foundations: 'planned', 'Ada Lovelace': 'closed' }))?.id).toBe('Ada Lovelace');
    expect(lastClosed(withStates({ Foundations: 'planned' }))).toBeUndefined();
  });
});
