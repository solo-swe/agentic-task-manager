import { describe, expect, it } from 'vitest';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { velocity } from '../../src/lib/velocity';
import { resolveVocabulary } from '../../src/lib/vocab';

const vocab = resolveVocabulary({});
const task = (id: string, sprint: string, status: string) => ({ id, sprint, status, type: 'feature' }) as Task;

describe('velocity (T-323)', () => {
  const meta = {
    sprints: [
      {
        id: 'A',
        state: 'closed',
        end: '2026-04-01T00:00:00Z',
        report: { done: ['T-1', 'T-2', 'T-3'], carried: ['T-4'] },
      },
      { id: 'B', state: 'closed', report: { done: ['T-4'], added: ['T-5', 'T-6'] } },
      // Without a report (a project's phases, read as sprints, have none): its tasks done
      { id: 'C', state: 'closed' },
      { id: 'D', state: 'active' },
    ],
  };
  const tasks = [
    task('T-7', 'C', 'done'),
    task('T-8', 'C', 'done'),
    task('T-9', 'C', 'skipped'),
    task('T-10', 'D', 'done'),
  ];
  const model = sprintModel(meta, tasks, vocab);

  it('the closed sprints, oldest first: done from the report (else the tasks), carried, added; the average', () => {
    const { sprints, average } = velocity(model, tasks, vocab);
    expect(sprints).toEqual([
      { id: 'A', done: 3, carried: 1, added: 0, end: '2026-04-01T00:00:00Z' },
      { id: 'B', done: 1, carried: 0, added: 2 },
      { id: 'C', done: 2, carried: 0, added: 0 },
    ]);
    expect(average).toBe(2);
  });

  it('the last few only; none closed: nothing', () => {
    expect(velocity(model, tasks, vocab, 2).sprints.map((s) => s.id)).toEqual(['B', 'C']);
    const open = sprintModel({ sprints: [{ id: 'D', state: 'active' }] }, [], vocab);
    expect(velocity(open, [], vocab)).toEqual({ sprints: [], average: 0 });
  });
});
