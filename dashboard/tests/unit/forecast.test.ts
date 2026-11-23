import { describe, expect, it } from 'vitest';
import type { Forecast as ServedForecast } from '$generated/types';
import { BACKLOG } from '../../src/lib/filters';
import { fromServed, NO_FORECAST, sprintKey } from '../../src/lib/forecast';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';
import { progressTasks, sprintModel } from './views';

// The forecast's rules are atm's (ATM-722: atm-lib's stats::forecast, tested there); the dashboard reads its answer
const model = sprintModel(progress.meta, progressTasks, DEFAULT_VOCABULARY);

const served: ServedForecast = {
  pace: { likely: 20, low: 15, high: 25, based_on: 3 },
  sprints: [
    {
      sprint: 'Phase 0',
      total: 2,
      done: 2,
      skipped: 0,
      remaining: 0,
      blocked: 0,
      in_review: 0,
      state: 'done',
      completed_at: 1000,
    },
    {
      sprint: null,
      total: 3,
      done: 0,
      skipped: 1,
      remaining: 2,
      blocked: 1,
      in_review: 1,
      state: 'not_started',
      estimate: {
        minutes: { likely: 40, low: 30, high: 50 },
        finish_at: { likely: 9, low: 8, high: 10 },
      },
    },
  ],
  next: { sprint: null },
};

describe('fromServed (ATM-722)', () => {
  it('atm’s rows as the charts read them: the backlog keyed BACKLOG, a sprint’s row with its sprint, camel case', () => {
    const f = fromServed(served, model);
    expect(f.pace).toEqual({ likely: 20, low: 15, high: 25, basedOn: 3 });
    expect(f.sprints.map((s) => [s.key, s.sprint?.id, s.state, s.inReview])).toEqual([
      ['Phase 0', 'Phase 0', 'done', 0],
      [BACKLOG, undefined, 'not_started', 1],
    ]);
    expect(f.sprints[0].completedAt).toBe(1000);
    expect(f.sprints[0].estimate).toBeUndefined();
    expect(f.sprints[1].estimate).toEqual({
      minutes: { likely: 40, low: 30, high: 50 },
      finishAt: { likely: 9, low: 8, high: 10 },
    });
    // The loop's next: the backlog here
    expect(f.next).toBe(BACKLOG);
  });

  it('none before atm answers; no pace and nothing next when atm has none', () => {
    expect(fromServed(undefined, model)).toBe(NO_FORECAST);
    const bare = fromServed({ pace: null, sprints: [], next: null }, model);
    expect(bare).toEqual({ pace: null, sprints: [] });
  });

  it('a sprint the model doesn’t list keeps its key, without a sprint', () => {
    const row = { ...served.sprints[0], sprint: 'Gone' };
    expect(fromServed({ ...served, sprints: [row] }, model).sprints[0]).toMatchObject({
      key: 'Gone',
      sprint: undefined,
    });
  });
});

describe('sprintKey', () => {
  it('a task’s sprint, as the model says it, or BACKLOG', () => {
    const stub = { ...model, sprintOf: (t: { id: string }) => (t.id === 'T-001' ? 'S1' : undefined) };
    const [first, second] = progressTasks;
    expect(sprintKey(stub, first)).toBe('S1');
    expect(sprintKey(stub, second)).toBe(BACKLOG);
  });
});
