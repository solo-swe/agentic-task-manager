import { describe, expect, it } from 'vitest';
import {
  activitySeries,
  cycleScaleMax,
  gaps,
  MAX_ACTIVITY_BUCKETS,
  median,
  progressBreakdown,
  periodPhrase,
  PRESET_BUCKET_MS,
  rangeAxis,
  rangeLabel,
  rangeWindow,
  sparklineTitle,
} from '../../src/lib/insights';
import type { Task } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import standard from '../fixtures/standard.json';

const H = 3_600_000;
const T0 = Date.parse('2026-04-01T08:10:00Z');
const iso = (ms: number) => new Date(ms).toISOString();

const task = (id: string, fields: Partial<Task> = {}): Task => ({
  id,
  name: `Task ${id}`,
  phase: 0,
  type: 'feature',
  description: '',
  status: 'todo',
  ...fields,
});

/** A task started at T0 + `start` hours and, when `end` is given, done at T0 + `end` hours */
const worked = (id: string, start: number, end?: number, fields: Partial<Task> = {}) =>
  task(id, {
    status: end === undefined ? 'in_progress' : 'done',
    created_at: iso(T0),
    started_at: iso(T0 + start * H),
    ...(end === undefined ? {} : { completed_at: iso(T0 + end * H) }),
    ...fields,
  });

describe('median', () => {
  it('is the middle value, or the mean of the two middle ones', () => {
    expect(median([])).toBeNull();
    expect(median([5])).toBe(5);
    expect(median([9, 1, 5])).toBe(5);
    expect(median([4, 1, 3, 2])).toBe(2.5);
  });
});

describe('progressBreakdown', () => {
  it('shares done overall, per phase and per type', () => {
    const tasks = [
      task('a', { status: 'done', phase: 1 }),
      task('b', { phase: 1, type: 'test' }),
      task('c', { status: 'done', phase: 0, type: 'test' }),
      task('d', { status: 'in_progress', phase: 0 }),
    ];
    const p = progressBreakdown(tasks);
    expect(p.overall).toEqual({ key: 'all', label: 'All tasks', done: 2, total: 4, skipped: 0, ratio: 0.5 });
    expect(p.byPhase.map((s) => [s.label, s.done, s.total])).toEqual([
      ['Phase 0', 1, 2],
      ['Phase 1', 1, 2],
    ]);
    expect(p.byType.map((s) => [s.key, s.ratio])).toEqual([
      ['feature', 0.5],
      ['test', 0.5],
    ]);
  });

  it('counts statuses in workflow order, unknown statuses last', () => {
    const tasks = ['todo', 'weird', 'done', 'blocked', 'done', 'in_progress'].map((status, i) =>
      task(String(i), { status }),
    );
    expect(progressBreakdown(tasks).byStatus).toEqual([
      { status: 'done', count: 2 },
      { status: 'in_progress', count: 1 },
      { status: 'todo', count: 1 },
      { status: 'blocked', count: 1 },
      { status: 'weird', count: 1 },
    ]);
  });

  it('no tasks: a ratio of 0, not NaN', () => {
    expect(progressBreakdown([]).overall.ratio).toBe(0);
  });

  it('archived tasks count as done overall, so archiving never lowers the percentage (T-238)', () => {
    const current = [task('a', { status: 'done' }), task('b', { status: 'todo' }), task('c', { status: 'todo' })];
    // 6 archived, not loaded: counted from meta.archived
    const p = progressBreakdown(current, undefined, 6);
    expect(p.overall).toMatchObject({ done: 7, total: 9 });
    expect(p.overall.ratio).toBeCloseTo(7 / 9);
    expect(p.archived).toBe(6);
    // Phases and types are the tasks passed in: tasks.json's until the archive is loaded
    expect(p.byPhase.map((s) => s.total)).toEqual([3]);
    // Loaded: the history has them, and the count is only for the tooltip
    const archived = Array.from({ length: 6 }, (_, i) => task(`x${i}`, { status: 'done' }));
    const loaded = progressBreakdown([...current, ...archived], undefined, 0, 6);
    expect(loaded.overall).toMatchObject({ done: 7, total: 9 });
    expect(loaded.archived).toBe(6);
  });

  it('skipped tasks count as neither done nor left: 7 done and 3 skipped is 100%, the 3 shown apart', () => {
    const tasks = [
      ...Array.from({ length: 7 }, (_, i) => task(`d${i}`, { status: 'done' })),
      ...Array.from({ length: 3 }, (_, i) => task(`s${i}`, { status: 'skipped' })),
    ];
    expect(progressBreakdown(tasks).overall).toMatchObject({ done: 7, total: 7, skipped: 3, ratio: 1 });
    const someLeft = progressBreakdown([...tasks, task('t', { status: 'todo' })]).overall;
    expect(someLeft).toMatchObject({ done: 7, total: 8, skipped: 3, ratio: 7 / 8 });
  });

  it('a group of only skipped tasks is finished, not 0%', () => {
    const p = progressBreakdown([task('a', { status: 'skipped', phase: 2 }), task('b', { status: 'todo', phase: 1 })]);
    expect(p.byPhase.map((s) => [s.label, s.total, s.skipped, s.ratio])).toEqual([
      ['Phase 1', 1, 0, 0],
      ['Phase 2', 0, 1, 1],
    ]);
  });

  it("follows a project's statuses by kind: done-kind is done, the split is in kind order", () => {
    const vocab = resolveVocabulary({
      statuses: [
        { id: 'backlog', kind: 'todo' },
        { id: 'doing', kind: 'active' },
        { id: 'awaiting_user', kind: 'review' },
        { id: 'shipped', kind: 'done' },
      ],
    });
    const tasks = ['backlog', 'awaiting_user', 'shipped', 'doing', 'shipped'].map((status, i) =>
      task(String(i), { status }),
    );
    const p = progressBreakdown(tasks, vocab);
    expect(p.overall.done).toBe(2);
    expect(p.byStatus.map((s) => s.status)).toEqual(['shipped', 'doing', 'awaiting_user', 'backlog']);
  });

  it('works on the standard fixture', () => {
    const p = progressBreakdown(standard.tasks as Task[]);
    expect([p.overall.total, p.overall.skipped]).toEqual([11, 1]); // one of its 12 tasks is skipped
    expect(p.byStatus.reduce((n, s) => n + s.count, 0)).toBe(12);
  });
});

describe('activitySeries', () => {
  it('measures time per task and agent work until the review was requested', () => {
    const s = activitySeries(
      [
        worked('a', 0, 4, { agent: 'alice', review_requested_at: iso(T0 + H) }),
        worked('b', 1, undefined, { agent: 'bob', status: 'in_review', review_requested_at: iso(T0 + 2 * H) }),
      ],
      null,
      T0 + 5 * H,
    )!;
    expect(s.cycleTimes.map((c) => [c.id, c.minutes])).toEqual([['a', 60]]);
    // Each worked one hour (hourly buckets, aligned to the clock: 08:00, 09:00, ...), not until completion or now
    const busyHours = (agent: string) => s.buckets.reduce((sum, b) => sum + (b.agentBusy[agent] ?? 0), 0);
    expect(busyHours('alice')).toBeCloseTo(1);
    expect(busyHours('bob')).toBeCloseTo(1);
  });

  it("follows a project's statuses by kind", () => {
    const vocab = resolveVocabulary({
      statuses: [
        { id: 'backlog', kind: 'todo' },
        { id: 'doing', kind: 'active' },
        { id: 'shipped', kind: 'done' },
      ],
    });
    const tasks = [worked('a', 0, 1, { status: 'shipped' }), worked('b', 1, undefined, { status: 'doing' })];
    expect(activitySeries(tasks, null, T0 + 2 * H)).toBeNull();
    const s = activitySeries(tasks, null, T0 + 2 * H, vocab)!;
    // b, in the project's active status, is still at work in the last bucket
    expect([s.totalCompleted, s.buckets.at(-1)!.byKind.agents]).toEqual([1, 1]);
  });

  it('is null without completions', () => {
    expect(activitySeries([])).toBeNull();
    expect(activitySeries([worked('a', 0)])).toBeNull();
  });

  it('buckets completions by hour over a short span, with per-type counts and a rolling average', () => {
    const tasks = [
      worked('a', 0, 0.5),
      worked('b', 0.2, 0.6, { type: 'test' }),
      worked('c', 1, 2.5),
      worked('d', 2, 3.2),
      worked('e', 3, undefined),
      task('f', { created_at: iso(T0 + 2.5 * H) }),
    ];
    const s = activitySeries(tasks)!;
    expect(s.unitLabel).toBe('hour');
    expect(s.bucketMs).toBe(H);
    expect(s.totalCompleted).toBe(4);
    expect(s.buckets.map((b) => b.completed.length)).toEqual([2, 0, 1, 1]);
    expect(s.buckets[0].byType).toEqual({ feature: 1, test: 1 });
    // Mean of this bucket and up to two before it
    expect(s.buckets.map((b) => b.average)).toEqual([2, 1, 1, 2 / 3]);
    expect(s.types).toEqual([
      { type: 'feature', count: 3 },
      { type: 'test', count: 1 },
    ]);
  });

  it('measures cycle times from start to completion, oldest first, with their median', () => {
    const s = activitySeries([worked('slow', 0, 3), worked('fast', 1, 1.5), worked('bad', 2, 1)])!;
    expect(s.cycleTimes.map((c) => [c.id, c.minutes])).toEqual([
      ['fast', 30],
      ['slow', 180],
    ]);
    expect(s.medianCycleMinutes).toBe(105);
  });

  it('the time-per-task scale stops at the 90th percentile, so one outlier does not flatten the rest (T-248)', () => {
    // 20 tasks of 10–29 m and one of 20 h: the scale ends at the 19th value, 28 m
    const minutes = [...Array.from({ length: 20 }, (_, i) => 10 + i), 1200];
    expect(cycleScaleMax(minutes)).toBe(28);
    // Few tasks: the nearest rank is the slowest, so none is an outlier
    expect(cycleScaleMax([20, 30, 50, 60, 60, 80])).toBe(80);
    expect(cycleScaleMax([])).toBe(1);
    expect(cycleScaleMax([0.2])).toBe(1);
  });

  it('marks interactive tasks: their time includes working with you (T-246)', () => {
    const s = activitySeries([worked('alone', 0, 1), worked('pair', 1, 3, { labels: ['ux', 'interactive'] })])!;
    expect(s.cycleTimes.map((c) => [c.id, c.withYou])).toEqual([
      ['alone', false],
      ['pair', true],
    ]);
  });

  it('picks longer buckets for longer spans, never more than the maximum', () => {
    const days = activitySeries([worked('a', 0, 1), worked('b', 0, 24 * 20)])!;
    expect(days.unitLabel).toBe('day');
    const months = activitySeries([worked('a', 0, 1), worked('b', 0, 24 * 365 * 5)])!;
    expect(months.unitLabel).toBe('30 days');
    for (const s of [days]) expect(s.buckets.length).toBeLessThanOrEqual(MAX_ACTIVITY_BUCKETS);
  });

  it('tasks without a start time still count as completions, but have no cycle time', () => {
    const s = activitySeries([task('a', { status: 'done', completed_at: iso(T0) })])!;
    expect(s.totalCompleted).toBe(1);
    expect(s.cycleTimes).toEqual([]);
    expect(s.medianCycleMinutes).toBeNull();
  });
});

describe('the sparkline tooltip (T-249)', () => {
  it('says what the line is, and what the current bucket has so far while it is not over', () => {
    const tasks = [worked('a', 0, 0.5), worked('b', 0.2, 0.6), worked('c', 1, 2.5)];
    // The last bucket is the hour from 10:00; at 10:40 it isn't over
    const s = activitySeries(tasks, null, T0 + 2.5 * H)!;
    expect(sparklineTitle(s, T0 + 2.5 * H)).toBe(
      'Tasks completed per hour, all time: 3 · the line is the average of the last 3 hours · this hour so far: 1 · ' +
        'click for details',
    );
    // Later, the last bucket is over: nothing "so far"
    expect(sparklineTitle(s, T0 + 5 * H)).toBe(
      'Tasks completed per hour, all time: 3 · the line is the average of the last 3 hours · click for details',
    );
    const months = activitySeries([worked('a', 0, 1), worked('b', 0, 24 * 365 * 5)])!;
    expect(sparklineTitle(months, Date.parse('2040-01-01'))).toContain('the average of the last 3 periods of 30 days');
  });
});

describe('rangeWindow and activity in a period', () => {
  const now = new Date(2026, 3, 1, 15, 30).getTime(); // Wednesday 1 April, 15:30 local

  it('turns presets into periods ending now', () => {
    expect(rangeWindow({ preset: 'hour' }, now)).toEqual({ start: now - H, end: now });
    expect(rangeWindow({ preset: 'day' }, now)!.start).toBe(new Date(2026, 3, 1).getTime());
    expect(rangeWindow({ preset: 'week' }, now)!.start).toBe(new Date(2026, 2, 30).getTime()); // Monday
    expect(rangeWindow({ preset: 'month' }, now)!.start).toBe(new Date(2026, 3, 1).getTime());
    expect(rangeWindow({ preset: 'all' }, now)).toBeNull();
    expect(rangeWindow({ preset: 'custom', from: 1, to: 5 }, now)).toEqual({ start: 1, end: 5 });
    expect(rangeWindow({ preset: 'custom', from: 5, to: 1 }, now)).toBeNull();
  });

  it('rangeAxis: a running period spans all of it, to come included; the others are their window (T-350)', () => {
    expect(rangeAxis({ preset: 'day' }, now)).toEqual({
      start: new Date(2026, 3, 1).getTime(),
      end: new Date(2026, 3, 2).getTime(),
    });
    expect(rangeAxis({ preset: 'week' }, now)).toEqual({
      start: new Date(2026, 2, 30).getTime(),
      end: new Date(2026, 3, 6).getTime(),
    });
    expect(rangeAxis({ preset: 'month' }, now)!.end).toBe(new Date(2026, 4, 1).getTime());
    expect(rangeAxis({ preset: 'hour' }, now)).toEqual(rangeWindow({ preset: 'hour' }, now));
    expect(rangeAxis({ preset: 'week', offset: -1 }, now)).toEqual(rangeWindow({ preset: 'week', offset: -1 }, now));
    expect(rangeAxis({ preset: 'all' }, now)).toBeNull();
  });

  it('a running week on its axis: a bucket per day of it, the days to come empty (T-350)', () => {
    const t = (d: number, h: number) => new Date(2026, 3, d, h).toISOString();
    const tasks = [
      { id: 'A', name: 'A', type: 'feature', status: 'done', started_at: t(1, 9), completed_at: t(1, 10) },
      { id: 'B', name: 'B', type: 'feature', status: 'done', started_at: t(1, 11), completed_at: t(1, 12) },
    ] as Task[];
    const week = activitySeries(tasks, rangeAxis({ preset: 'week' }, now), now, undefined, PRESET_BUCKET_MS.week)!;
    expect(week.buckets).toHaveLength(7);
    expect(week.buckets.map((b) => b.completed.length)).toEqual([0, 0, 1 + 1, 0, 0, 0, 0]);
    expect(week.totalCompleted).toBe(2);
    // No idle time in the future: idle only up to now
    expect(week.idle.every((g) => g.end <= now)).toBe(true);
    const day = activitySeries(tasks, rangeAxis({ preset: 'day' }, now), now, undefined, PRESET_BUCKET_MS.day)!;
    expect(day.buckets).toHaveLength(24);
    expect(day.buckets.filter((b) => b.completed.length).map((b) => new Date(b.start).getHours())).toEqual([10, 12]);
  });

  it('steps back whole days, weeks and months, with readable labels', () => {
    expect(rangeWindow({ preset: 'day', offset: -1 }, now)).toEqual({
      start: new Date(2026, 2, 31).getTime(),
      end: new Date(2026, 3, 1).getTime(),
    });
    expect(rangeWindow({ preset: 'week', offset: -1 }, now)).toEqual({
      start: new Date(2026, 2, 23).getTime(),
      end: new Date(2026, 2, 30).getTime(),
    });
    expect(rangeWindow({ preset: 'month', offset: -2 }, now)!.start).toBe(new Date(2026, 1, 1).getTime());
    expect(rangeWindow({ preset: 'hour', offset: -2 }, now)).toEqual({ start: now - 3 * H, end: now - 2 * H });
    expect(rangeWindow({ preset: 'day', offset: 3 }, now)!.end).toBe(now); // never in the future
    const label = (preset: 'day' | 'week' | 'month' | 'hour', offset = 0) => rangeLabel({ preset, offset }, now);
    expect([label('day'), label('day', -1), label('week'), label('week', -1), label('month', -1)]).toEqual([
      'Today',
      'Yesterday',
      'This week',
      'Last week',
      'Last month',
    ]);
    expect(label('week', -2)).toMatch(/^Week of /);
    expect(label('hour')).toBe('Last hour');
    expect(rangeLabel({ preset: 'all' }, now)).toBe('All time');
  });

  it('a period limits the buckets and counts', () => {
    const tasks = [worked('a', 0, 0.5), worked('b', 1, 2.5), worked('c', 2, 2.7)];
    const s = activitySeries(tasks, { start: T0 + 2 * H, end: T0 + 3 * H })!;
    expect(s.unitLabel).toBe('5 minutes');
    expect(s.totalCompleted).toBe(2);
    expect(s.cycleTimes.map((c) => c.id)).toEqual(['b', 'c']);
  });

  it('each stepped period has its natural bucket: an hour per 5 minutes, a day per hour, a week or month per day', () => {
    const tasks = [worked('a', 0, 0.5), worked('b', 1, 2.5)];
    const bucketOf = (preset: 'hour' | 'day' | 'week' | 'month') => {
      const s = activitySeries(tasks, rangeWindow({ preset }, now), now, undefined, PRESET_BUCKET_MS[preset])!;
      return [s.unitLabel, s.buckets.length];
    };
    expect(bucketOf('hour')).toEqual(['5 minutes', 12]);
    expect(bucketOf('day')).toEqual(['hour', 16]); // midnight to 15:30
    expect(bucketOf('week')).toEqual(['day', 3]); // Monday to Wednesday (the bug: "per 6 hours")
    expect(bucketOf('month')).toEqual(['day', 1]);
    // All time and custom periods still pick the smallest size that fits
    expect(PRESET_BUCKET_MS.all).toBeUndefined();
    expect(PRESET_BUCKET_MS.custom).toBeUndefined();
  });

  it('periodPhrase names the period inside a heading', () => {
    const phrase = (preset: 'hour' | 'day' | 'week' | 'month', offset = 0) => periodPhrase({ preset, offset }, now);
    expect([phrase('hour'), phrase('day'), phrase('day', -1), phrase('week'), phrase('week', -1)]).toEqual([
      'in the last hour',
      'today',
      'yesterday',
      'this week',
      'last week',
    ]);
    expect([phrase('month'), phrase('month', -1)]).toEqual(['this month', 'last month']);
    expect(phrase('week', -2)).toMatch(/^in the week of /);
    expect(phrase('month', -2)).toMatch(/^in February 2026$/);
    expect(phrase('day', -3)).toMatch(/^on /);
    expect(phrase('hour', -1)).toMatch(/^from /);
    expect(periodPhrase({ preset: 'all' }, now)).toBe('over all time');
    expect(periodPhrase({ preset: 'custom', from: 1, to: 5 }, now)).toBe('in the chosen period');
  });
});

describe('idle stretches', () => {
  const M = 60_000;
  it('gaps of at least 10 minutes in the union of work intervals', () => {
    const work = [
      { start: 0, end: 20 * M },
      { start: 10 * M, end: 30 * M }, // overlaps: no gap
      { start: 35 * M, end: 40 * M }, // 5 min pause: too short
      { start: 70 * M, end: 80 * M }, // 30 min gap
      { start: 200 * M, end: 300 * M }, // starts after the end
    ];
    expect(gaps(work, 0, 120 * M)).toEqual([
      { start: 40 * M, end: 70 * M },
      { start: 80 * M, end: 120 * M },
    ]);
    expect(gaps([], 0, 5 * M)).toEqual([]);
  });

  it('activity reports idle time from the first start to now', () => {
    // a: 0–0.5 h, b: 2–2.5 h, now 3 h: idle 0.5–2 h and 2.5–3 h
    const s = activitySeries([worked('a', 0, 0.5), worked('b', 2, 2.5)], null, T0 + 3 * H)!;
    expect(s.idle).toEqual([
      { start: T0 + 0.5 * H, end: T0 + 2 * H },
      { start: T0 + 2.5 * H, end: T0 + 3 * H },
    ]);
    expect(s.idleMs).toBe(2 * H);
  });

  it('agents busy per period: each agent at most fully busy', () => {
    const s = activitySeries(
      [worked('a', 0, 0.5, { agent: 'x' }), worked('b', 0, 0.5, { agent: 'x' }), worked('c', 0, 1, { agent: 'y' })],
      null,
      T0 + H,
    )!;
    expect(s.agents).toEqual(['x', 'y']);
    expect(s.buckets[0].agentBusy.x).toBeLessThanOrEqual(1);
  });

  it('per bucket by kind: Claude sessions and unattributed work are agents, anyone else people, each capped (T-247)', () => {
    const s = activitySeries(
      [
        worked('a', 0, 1, { agent: 'claude:a1' }),
        worked('b', 0, 0.5, { agent: 'Hakan' }),
        worked('c', 0, 1),
        worked('d', 0, 1),
      ],
      null,
      T0 + H,
    )!;
    // Two unattributed tasks over the same hour are one agent, like a session on two tasks
    expect(s.buckets[0].agentBusy['']).toBe(1);
    expect(s.buckets[0].byKind.agents).toBeCloseTo(s.buckets[0].agentBusy['claude:a1'] + 1);
    expect(s.buckets[0].byKind.people).toBeCloseTo(s.buckets[0].agentBusy.Hakan);
    expect(s.buckets[0].byKind.people).toBeGreaterThan(0);
  });
});
