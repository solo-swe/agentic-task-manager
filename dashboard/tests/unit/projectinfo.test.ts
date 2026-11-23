import { describe, expect, it } from 'vitest';
import { actorsOf, vocabCounts } from '../../src/lib/projectinfo';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';
import { fromServedSprints } from '../../src/lib/sprints';
import { sprint, vocabulary, withKinds } from './views';

const T0 = Date.parse('2026-05-01T08:00:00Z');
const at = (min: number) => new Date(T0 + min * 60_000).toISOString();
/** A task as atm serves it: with its kind */
const task = (id: string, fields: Partial<Task>): Task =>
  withKinds([{ id, name: id, phase: 0, type: 'feature', description: '', status: 'todo', ...fields } as Task])[0];

// Who worked on it is atm's (ATM-722: atm-lib's stats::people, its cases there); the dashboard reads its answer
describe('actorsOf', () => {
  it('atm’s people as they are, its agents in the panel’s names; nobody before atm answers', () => {
    const people = [{ name: 'Hakan', created: 4, started: 0, completed: 1 }];
    const served = { actors: { people, agents: { sessions: 3, started: 4, completed: 2, agent_ms: 600 } } };
    expect(actorsOf(served as unknown as Parameters<typeof actorsOf>[0])).toEqual({
      people,
      agents: { sessions: 3, started: 4, completed: 2, agentMs: 600 },
    });
    expect(actorsOf(null)).toEqual({ people: [], agents: { sessions: 0, started: 0, completed: 0, agentMs: 0 } });
  });
});

describe('vocabCounts', () => {
  const vocab = vocabulary({
    phaseNames: new Map([
      [0, 'Foundations'],
      [3, 'Later'],
    ]),
    labels: [
      { id: 'ux', label: 'User experience' },
      { id: 'unused', label: 'Not used yet' },
    ],
  });
  const tasks = [
    task('T-1', { phase: 0, status: 'done', labels: ['ux', 'zeta'] }),
    task('T-2', { phase: 0, status: 'skipped', priority: 'P0' }),
    task('T-3', { phase: 1, status: 'todo', started_at: at(0), labels: ['ux'] }),
    task('T-4', { phase: 2, status: 'todo', type: 'chore' }),
    task('T-5', { phase: 2, status: 'parked' }),
  ];
  const v = vocabCounts(tasks, vocab);

  it('sprints (T-323): the active, the planned in order, the closed most recent first, with atm’s counts; its backlog', () => {
    // atm's sprints and backlog (ATM-720): a sprint's tasks but the skipped, the backlog's unfinished tasks
    const model = fromServedSprints({
      sprints: [
        sprint({ id: 'One', state: 'closed', total: 1 }),
        sprint({ id: 'Two', state: 'closed' }),
        sprint({ id: 'Three', state: 'active', total: 2 }),
        sprint({ id: 'Four' }),
      ],
      backlog: 1,
      from_phases: false,
    });
    const counted = vocabCounts([], DEFAULT_VOCABULARY, model);
    expect(counted.sprints).toEqual([
      { id: 'Three', state: 'active', count: 2 },
      { id: 'Four', state: 'planned', count: 0 },
      { id: 'Two', state: 'closed', count: 0 },
      { id: 'One', state: 'closed', count: 1 },
    ]);
    expect(counted.backlog).toBe(1);
    // Without sprints: none
    expect(v.sprints).toEqual([]);
    expect(v.backlog).toBe(0);
  });

  it("labels: meta.labels' order and text (unused ones too), then the others by name", () => {
    expect(v.labels).toEqual([
      { id: 'ux', text: 'User experience', count: 2, listed: true },
      { id: 'unused', text: 'Not used yet', count: 0, listed: true },
      { id: 'zeta', text: 'zeta', count: 1, listed: false },
    ]);
  });

  it('types, statuses (with kinds) and priorities (with the default), each with its count', () => {
    expect(v.types.find((t) => t.id === 'feature')!.count).toBe(4);
    expect(v.types.at(-1)).toEqual({ id: 'chore', label: 'Chore', count: 1 });
    expect(v.statuses.find((s) => s.id === 'todo')).toEqual({ id: 'todo', label: 'To Do', kind: 'todo', count: 2 });
    expect(v.statuses.at(-1)).toEqual({ id: 'parked', label: 'Parked', kind: undefined, count: 1 });
    expect(v.priorities.map((p) => [p.id, p.count, p.isDefault])).toEqual([
      ['P0', 1, false],
      ['P1', 0, false],
      ['P2', 4, true],
      ['P3', 0, false],
    ]);
  });
});
