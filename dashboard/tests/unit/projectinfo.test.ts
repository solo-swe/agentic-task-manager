import { describe, expect, it } from 'vitest';
import { actors, vocabCounts, workflowOf } from '../../src/lib/projectinfo';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';

const T0 = Date.parse('2026-05-01T08:00:00Z');
const at = (min: number) => new Date(T0 + min * 60_000).toISOString();
const task = (id: string, fields: Partial<Task>): Task =>
  ({ id, name: id, phase: 0, type: 'feature', description: '', status: 'todo', ...fields }) as Task;

describe('actors (T-252)', () => {
  const tasks = [
    // Hakan created all four; a session started and finished two; Hakan approved one review
    task('T-1', {
      created_by: 'Hakan',
      started_by: 'claude:a',
      started_at: at(0),
      completed_by: 'claude:a',
      completed_at: at(30),
      status: 'done',
    }),
    task('T-2', {
      created_by: 'Hakan',
      started_by: 'claude:b',
      started_at: at(10),
      review_requested_at: at(40),
      completed_by: 'Hakan',
      completed_at: at(60),
      status: 'done',
    }),
    // Before v4: `agent`, and a task nobody recorded
    task('T-3', { created_by: 'Hakan', agent: 'claude:c', started_at: at(60), status: 'in_progress' }),
    task('T-4', { created_by: 'Hakan', started_at: at(100), completed_at: at(110), status: 'done' }),
    // A person did one themselves
    task('T-5', { created_by: 'claude:a', started_by: 'Ana', started_at: at(0), status: 'in_progress' }),
  ];

  it('people with what they created, started and completed, most active first', () => {
    expect(actors(tasks, T0 + 120 * 60_000).people).toEqual([
      { name: 'Hakan', created: 4, started: 0, completed: 1 },
      { name: 'Ana', created: 0, started: 1, completed: 0 },
    ]);
  });

  it('agents in one line: sessions, tasks started and completed, agent time (parallel agents add up)', () => {
    const { agents } = actors(tasks, T0 + 120 * 60_000);
    // Sessions a, b, c; T-4 (nobody recorded) counts as agent work but has no session. Completed: T-1 by a, and
    // T-4, finished without a completed_by (before v4); T-2 was Hakan's approval
    expect(agents.sessions).toBe(3);
    expect([agents.started, agents.completed]).toEqual([4, 2]);
    // T-1 30 m, T-2 30 m (to the review), T-3 60 m (running until now), T-4 10 m; Ana's T-5 isn't agent time
    expect(agents.agentMs / 60_000).toBe(130);
  });

  it('no tasks: nobody, and no agent time', () => {
    expect(actors([], T0)).toEqual({ people: [], agents: { sessions: 0, started: 0, completed: 0, agentMs: 0 } });
  });
});

describe('workflowOf', () => {
  it('reads meta.workflow, verify on unless it says false, and the trailers commits must carry', () => {
    expect(
      workflowOf({
        workflow: {
          type: 'pull-request',
          base: 'main',
          commit_template: 'feat({id}): {name}',
          trailers: { require: ['Signed-off-by: x', 3] },
        },
      }),
    ).toEqual({
      type: 'pull-request',
      base: 'main',
      verify: true,
      commitTemplate: 'feat({id}): {name}',
      trailers: ['Signed-off-by: x'],
    });
    expect(workflowOf({ workflow: { type: 'direct', verify: false } })).toMatchObject({ verify: false, trailers: [] });
    expect(workflowOf({ workflow: { base: 'main' } })).toBeNull();
    expect(workflowOf(undefined)).toBeNull();
  });
});

describe('vocabCounts', () => {
  const vocab = resolveVocabulary({
    phases: [
      { id: 0, name: 'Foundations' },
      { id: 3, name: 'Later' },
    ],
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

  it('sprints (T-323): the active, the planned in order, the closed most recent first, with counts; the backlog', () => {
    const meta = {
      sprints: [
        { id: 'One', state: 'closed' },
        { id: 'Two', state: 'closed' },
        { id: 'Three', state: 'active' },
        { id: 'Four', state: 'planned' },
      ],
    };
    const inSprints = [
      task('T-1', { sprint: 'One', status: 'done' }),
      task('T-2', { sprint: 'Three' }),
      task('T-3', { sprint: 'Three' }),
      task('T-4', {}),
    ];
    const counted = vocabCounts(
      inSprints,
      resolveVocabulary(meta),
      sprintModel(meta, inSprints, resolveVocabulary(meta)),
    );
    expect(counted.sprints).toEqual([
      { id: 'Three', state: 'active', count: 2 },
      { id: 'Four', state: 'planned', count: 0 },
      { id: 'Two', state: 'closed', count: 0 },
      { id: 'One', state: 'closed', count: 1 },
    ]);
    expect(counted.backlog).toBe(1);
    // Without sprints: none, everything in the backlog
    expect(v.sprints).toEqual([]);
    expect(v.backlog).toBe(5);
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
