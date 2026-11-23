import { describe, expect, it, vi } from 'vitest';
import type { Progress } from '$generated/types';
import { statsUrl } from '../../src/lib/api/http';
import { callFor } from '../../src/lib/api/ipc';
import { coveredMs, loadStats, NO_PROGRESS, progressOf, timeUseOf } from '../../src/lib/stats';

// A project's stats are atm's (ATM-722: the stats route, atm-lib's `stats`, tested there); the dashboard asks for them
// and reads them

describe('asking for them', () => {
  it('the route takes the slice and the period; none, the whole project', () => {
    expect(statsUrl('p1')).toBe('/api/v1/projects/p1/stats');
    expect(statsUrl('p1', { tasks: ['T-1', 'ATM-2'], from: 10.4, to: 20 })).toBe(
      '/api/v1/projects/p1/stats?tasks=T-1%2CATM-2&from=10&to=20',
    );
  });

  it('in the app, the same is the project_stats command', () => {
    expect(callFor('/api/v1/projects/p1/stats?tasks=T-1,T-2&from=10')).toEqual({
      command: 'project_stats',
      args: { workspace: 'default', project: 'p1', tasks: ['T-1', 'T-2'], from: 10, to: null },
    });
    expect(callFor('/api/v1/projects/p1/stats')?.args).toMatchObject({ tasks: null, from: null, to: null });
  });

  it('a refusal is an error', async () => {
    const ok = vi.fn(async () => new Response(JSON.stringify({ progress: {} }), { status: 200 }));
    expect(await loadStats('p1', {}, ok)).toEqual({ progress: {} });
    expect(ok).toHaveBeenCalledWith('/api/v1/projects/p1/stats', { cache: 'no-store' });
    const refused = async () => new Response('{"error": "no"}', { status: 404 });
    await expect(loadStats('p1', {}, refused)).rejects.toThrow('HTTP 404');
  });
});

describe('progressOf', () => {
  it('atm’s progress in the widget’s names; nothing before atm answers', () => {
    const share = { key: 'all', label: 'All tasks', done: 1, total: 2, skipped: 0, ratio: 0.5 };
    const served: Progress = {
      overall: share,
      archived: 3,
      by_status: [{ status: 'done', count: 1 }],
      by_type: [{ ...share, key: 'feature', label: 'feature' }],
      by_sprint: [{ ...share, key: null, label: 'Backlog' }],
    };
    expect(progressOf(served)).toEqual({
      overall: share,
      archived: 3,
      byStatus: [{ status: 'done', count: 1 }],
      byType: [{ ...share, key: 'feature', label: 'feature' }],
    });
    expect(progressOf(undefined)).toBe(NO_PROGRESS);
  });
});

describe('timeUseOf', () => {
  const served = {
    from: 0,
    to: 100,
    working_ms: 40,
    waiting_ms: 20,
    idle_ms: 40,
    agent_ms: 60,
    max_agents: 2,
    waiting: [[40, 60]],
    idle: [[60, 100]],
  };

  it('atm’s time use, and the same split for any part of it (a chart’s bucket)', () => {
    const use = timeUseOf(served)!;
    expect([use.workingMs, use.waitingMs, use.idleMs, use.agentMs, use.maxAgents]).toEqual([40, 20, 40, 60, 2]);
    expect(use.waiting).toEqual([{ start: 40, end: 60 }]);
    // 30–70: waiting 40–60, idle 60–70, the rest working
    expect(use.split(30, 70)).toEqual({ workingMs: 10, waitingMs: 20, idleMs: 10 });
    // Outside the period: nothing
    expect(use.split(200, 300)).toEqual({ workingMs: 0, waitingMs: 0, idleMs: 0 });
  });

  it('none when atm has none', () => {
    expect(timeUseOf(null)).toBeNull();
    expect(timeUseOf(undefined)).toBeNull();
  });

  it('coveredMs: what the stretches cover inside the bounds', () => {
    expect(
      coveredMs(
        [
          { start: 0, end: 10 },
          { start: 20, end: 30 },
        ],
        5,
        25,
      ),
    ).toBe(10);
    expect(coveredMs([], 0, 100)).toBe(0);
  });
});
