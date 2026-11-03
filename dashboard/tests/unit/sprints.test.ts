import { describe, expect, it } from 'vitest';
import {
  activeSprint,
  addedMidSprint,
  backlog,
  carriedCount,
  daysUntil,
  dueText,
  effectiveRelease,
  sprintModel,
  sprintSummary,
  sprintTasks,
} from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import fixture from '../fixtures/sprints.json';
import standard from '../fixtures/standard.json';

const file = fixture as TasksFile;
const tasks = file.tasks as Task[];
const model = sprintModel(file.meta, tasks, resolveVocabulary(file.meta));
const byId = (id: string) => tasks.find((t) => t.id === id)!;
const ids = (list: Task[]) => list.map((t) => t.id);

describe('a migrated project', () => {
  it('reads meta.sprints in order, with their fields', () => {
    expect(model.derived).toBe(false);
    expect(model.sprints.map((s) => [s.id, s.state])).toEqual([
      ['Foundations', 'closed'],
      ['Ada Lovelace', 'active'],
      ['Alan Turing', 'planned'],
    ]);
    expect(activeSprint(model)).toMatchObject({ goal: 'Sign-in, end to end', due: '2026-04-12', release: '1.0' });
  });

  it("finds a task's sprint by name, in any case; no sprint is the backlog", () => {
    expect(ids(sprintTasks(model, tasks, 'Ada Lovelace'))).toEqual(['T-002', 'T-003', 'T-004']);
    expect(ids(backlog(model, tasks))).toEqual(['T-005']);
  });

  it("a task's release is its own, else its sprint's", () => {
    expect(effectiveRelease(model, byId('T-002'))).toBe('1.0');
    expect(effectiveRelease(model, byId('T-004'))).toBe('1.1');
    expect(effectiveRelease(model, byId('T-005'))).toBeUndefined();
  });

  it("is atm's to say when the task comes with atm's values (T-371): none from atm is none", () => {
    const derived = { kind: 'todo', ready: true, waiting_for: [], priority: 'P2', archived: false };
    expect(effectiveRelease(model, { ...byId('T-002'), derived: { ...derived, effective_release: '2.0' } })).toBe(
      '2.0',
    );
    expect(effectiveRelease(model, { ...byId('T-002'), derived })).toBeUndefined();
  });

  it('added mid-sprint: joined after the sprint started', () => {
    const sprint = activeSprint(model)!;
    expect(addedMidSprint(byId('T-002'), sprint)).toBe(false);
    expect(addedMidSprint(byId('T-003'), sprint)).toBe(false);
    expect(addedMidSprint(byId('T-004'), sprint)).toBe(true);
  });

  it('carried: how many earlier sprints the task was in', () => {
    expect(carriedCount(byId('T-003'), 'Ada Lovelace')).toBe(1);
    expect(carriedCount(byId('T-002'), 'Ada Lovelace')).toBe(0);
    expect(carriedCount(byId('T-005'), 'Ada Lovelace')).toBe(0);
  });

  it('is lenient: entries without a name are left out, an unknown state is planned, a duplicate name is dropped', () => {
    const meta = { sprints: [{ id: '' }, { id: 'A', state: 'finished' }, { id: 'a' }, 'x'] };
    const m = sprintModel(meta, [], resolveVocabulary(meta));
    expect(m.sprints).toEqual([{ id: 'A', state: 'planned' }]);
  });
});

describe('the sprint header', () => {
  const vocab = resolveVocabulary(file.meta);
  const noon = (date: string) => new Date(`${date}T12:00:00`).getTime();

  it('counts days to the due date from the local day', () => {
    expect(daysUntil('2026-04-12', noon('2026-04-09'))).toBe(3);
    expect(daysUntil('2026-04-12', new Date('2026-04-12T23:59:00').getTime())).toBe(0);
    expect(daysUntil('2026-04-12', noon('2026-04-14'))).toBe(-2);
    expect(daysUntil('12 April', noon('2026-04-09'))).toBeUndefined();
  });

  it('says how far off the due date is', () => {
    expect([3, 1, 0, -1, -4].map(dueText)).toEqual([
      'due in 3 days',
      'due tomorrow',
      'due today',
      '1 day overdue',
      '4 days overdue',
    ]);
  });

  it('sums up a sprint: done of total, added mid-sprint, carried in, days to due', () => {
    const sprint = activeSprint(model)!;
    const withDone = tasks.map((t) => (t.id === 'T-002' ? { ...t, status: 'done' } : t));
    expect(sprintSummary(model, withDone, sprint, vocab, noon('2026-04-09'))).toEqual({
      sprint,
      done: 1,
      total: 3,
      added: 1,
      carried: 1,
      dueIn: 3,
      reviewPending: 0,
    });
  });

  it('leaves skipped tasks out of the total, and a closed sprint has no due date to count', () => {
    const closed = { ...activeSprint(model)!, state: 'closed' as const };
    const skipped = tasks.map((t) => (t.id === 'T-004' ? { ...t, status: 'skipped' } : t));
    expect(sprintSummary(model, skipped, closed, vocab, noon('2026-04-09'))).toMatchObject({
      done: 0,
      total: 2,
      dueIn: undefined,
    });
  });
});

describe('a project not migrated yet: its phases read as sprints', () => {
  const file = standard as TasksFile;
  const vocab = resolveVocabulary(file.meta);
  const m = sprintModel(file.meta, file.tasks as Task[], vocab);

  it('one sprint per phase, named "Phase n" or after its name, closed when all its tasks are finished', () => {
    expect(m.derived).toBe(true);
    expect(m.sprints.map((s) => s.id)).toEqual(
      [...new Set((file.tasks as Task[]).map((t) => t.phase))]
        .sort()
        .map((p) => vocab.phaseNames.get(p) ?? `Phase ${p}`),
    );
    expect(m.sprints.every((s) => s.state !== 'active')).toBe(true);
    const t = (file.tasks as Task[])[0];
    expect(m.sprintOf(t)).toBe(vocab.phaseNames.get(t.phase) ?? `Phase ${t.phase}`);
  });

  it('as atm migrate names them: a name taken by an earlier phase gets " (phase n)"', () => {
    const meta = {
      phases: [
        { id: 1, name: 'Polish' },
        { id: 2, name: 'polish' },
      ],
    };
    const phased = [
      { id: 'T-1', name: 'a', phase: 1, type: 'feature', description: '', status: 'done' },
      { id: 'T-2', name: 'b', phase: 2, type: 'feature', description: '', status: 'todo' },
    ] as Task[];
    const m2 = sprintModel(meta, phased, resolveVocabulary(meta));
    expect(m2.sprints).toEqual([
      { id: 'Polish', state: 'closed' },
      { id: 'polish (phase 2)', state: 'planned' },
    ]);
    expect(m2.sprintOf(phased[1])).toBe('polish (phase 2)');
  });

  it('a project with neither sprints nor phases has none', () => {
    const m3 = sprintModel({}, [], resolveVocabulary({}));
    expect(m3).toMatchObject({ sprints: [], derived: false });
  });
});
