import { describe, expect, it } from 'vitest';
import type { Stats } from '$generated/types';
import { histogramOf, NO_STATS, numbersOf } from '../../src/lib/projectstats';

// The project's numbers and the time per task are atm's (ATM-722: atm-lib's stats::time, tested there); the Project
// tab reads its answer
const served = (stats: Record<string, unknown>, histogram: Record<string, unknown>) =>
  ({ stats, histogram }) as unknown as Stats;

describe('numbersOf', () => {
  it('atm’s numbers in the panel’s names', () => {
    const s = numbersOf(
      served(
        {
          started: 1000,
          duration: { from: 1000, to: 5000, ongoing: true },
          tasks: { total: 4, done: 3, open: 1, skipped: 0 },
          sprints: { total: 2, closed: 1 },
          task_commits: 1,
          pull_requests: null,
          agent_ms: 600,
          alone_ms: 200,
          with_you_ms: 400,
          max_agents: 2,
          parallelism: 1.5,
        },
        {},
      ),
    );
    expect(s).toEqual({
      started: 1000,
      duration: { from: 1000, to: 5000, ongoing: true },
      tasks: { total: 4, done: 3, open: 1, skipped: 0 },
      sprints: { total: 2, closed: 1 },
      taskCommits: 1,
      pullRequests: null,
      agentMs: 600,
      aloneMs: 200,
      withYouMs: 400,
      maxAgents: 2,
      parallelism: 1.5,
    });
  });

  it('none before atm answers: nothing known yet', () => {
    expect(numbersOf(null)).toBe(NO_STATS);
  });
});

describe('histogramOf', () => {
  it('atm’s bins, the last without an upper bound; its tasks in the panel’s names', () => {
    const task = { id: 'T-1', key: 'ATM-1', name: 'One', minutes: 3, with_you: false };
    const h = histogramOf(
      served(
        {},
        {
          bins: [
            { from: 0, to: 5, label: '0 m–5 m', alone: [task], with_you: [] },
            { from: 240, to: null, label: '4 h+', alone: [], with_you: [{ ...task, id: 'T-2', with_you: true }] },
          ],
          median_alone: 3,
          median_with_you: null,
          min: 3,
          max: 300,
          average: 151.5,
          slowest: [task],
        },
      ),
    )!;
    expect(h.bins.map((b) => [b.from, b.to, b.label, b.alone.length, b.withYou.length])).toEqual([
      [0, 5, '0 m–5 m', 1, 0],
      [240, Infinity, '4 h+', 0, 1],
    ]);
    expect(h.slowest).toEqual([{ id: 'T-1', key: 'ATM-1', name: 'One', minutes: 3, withYou: false }]);
    expect(h.bins[1].withYou[0].withYou).toBe(true);
    expect([h.medianAlone, h.medianWithYou, h.min, h.max, h.average]).toEqual([3, null, 3, 300, 151.5]);
  });

  it('none before atm answers', () => {
    expect(histogramOf(null)).toBeNull();
  });
});
