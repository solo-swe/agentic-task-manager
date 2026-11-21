import { describe, expect, it } from 'vitest';
import type { ServedSprints } from '$generated/types';
import {
  activeSprint,
  backlog,
  dueText,
  effectiveRelease,
  fromServedSprints,
  NO_SPRINTS,
  sprintTasks,
} from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { sprint, viewed } from './views';

// The sprints as atm serves them with the tasks (ATM-720): their figures and each task's sprint are atm's (atm-lib's
// sprints::views tests); the dashboard reads them
const served: ServedSprints = {
  sprints: [
    sprint({ id: 'Foundations', state: 'closed', done: 1, total: 1 }),
    sprint({ id: 'Ada Lovelace', state: 'active', goal: 'Sign-in', release: '1.0', done: 1, total: 3, percent: 33 }),
    sprint({ id: 'Alan Turing' }),
  ],
  backlog: 2,
  from_phases: false,
};
const task = (id: string, sprintName?: string, fields: Partial<Task> = {}): Task =>
  viewed(
    { id, name: id, phase: 0, type: 'feature', description: '', status: 'todo', ...fields },
    sprintName ? { sprint: sprintName } : {},
  );
const tasks = [task('T-1', 'Foundations'), task('T-2', 'Ada Lovelace'), task('T-3', 'Ada Lovelace'), task('T-4')];
const model = fromServedSprints(served);
const ids = (list: Task[]) => list.map((t) => t.id);

describe('fromServedSprints', () => {
  it("atm's sprints in its order, its backlog count, each task in the sprint atm says", () => {
    expect(model.sprints.map((s) => [s.id, s.state, s.percent])).toEqual([
      ['Foundations', 'closed', 100],
      ['Ada Lovelace', 'active', 33],
      ['Alan Turing', 'planned', 100],
    ]);
    expect([model.backlog, model.derived]).toEqual([2, false]);
    expect(activeSprint(model)).toMatchObject({ id: 'Ada Lovelace', goal: 'Sign-in' });
    expect(ids(sprintTasks(model, tasks, 'Ada Lovelace'))).toEqual(['T-2', 'T-3']);
    expect(ids(backlog(model, tasks))).toEqual(['T-4']);
  });

  it("an unmigrated project's phases, read-only; none before atm has served any", () => {
    expect(fromServedSprints({ ...served, from_phases: true }).derived).toBe(true);
    expect(fromServedSprints(undefined)).toBe(NO_SPRINTS);
    expect(activeSprint(NO_SPRINTS)).toBeUndefined();
  });
});

describe('effectiveRelease', () => {
  it("is atm's when the task comes with atm's values (T-371): none from atm is none", () => {
    const derived = { kind: 'todo', ready: true, waiting_for: [], priority: 'P2', archived: false };
    expect(effectiveRelease(model, { ...tasks[1], derived: { ...derived, effective_release: '2.0' } })).toBe('2.0');
    expect(effectiveRelease(model, { ...tasks[1], derived })).toBeUndefined();
  });

  it("without atm's values (a linked worktree's copy): its own, else its sprint's", () => {
    const copy = (sprintName: string | undefined, release?: string) =>
      ({ id: 'X', status: 'todo', ...(release ? { release } : {}), derived: { sprint: sprintName } }) as Task;
    expect(effectiveRelease(model, copy('Ada Lovelace'))).toBe('1.0');
    expect(effectiveRelease(model, copy('Ada Lovelace', '1.1'))).toBe('1.1');
    expect(effectiveRelease(model, copy(undefined))).toBeUndefined();
  });
});

describe('dueText', () => {
  it("says atm's days to the due date in words", () => {
    expect([3, 1, 0, -1, -4].map(dueText)).toEqual([
      'due in 3 days',
      'due tomorrow',
      'due today',
      '1 day overdue',
      '4 days overdue',
    ]);
  });
});
