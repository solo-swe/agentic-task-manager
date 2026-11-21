import { describe, expect, it } from 'vitest';
import { burndown, rangeStart } from '../../src/lib/burndown';
import { forecastSprints } from '../../src/lib/forecast';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';
import { progressTasks, taskWith, vocabulary, withKinds } from './views';

// Expected numbers: tests/fixtures/progress.md; atm's answers for its tasks (ATM-717): views.ts
const NOW = Date.parse('2026-04-06T12:00:00Z');
const tasks = progressTasks;
const at = (iso: string) => Date.parse(iso);
// Not migrated: its phases read as the sprints "Phase 0" (closed), "Phase 1" and "Phase 2"
const model = sprintModel(progress.meta, tasks, DEFAULT_VOCABULARY);
const forecastAll = (list: Task[], now: number) => forecastSprints(list, now, list, model);
const series = (b: ReturnType<typeof burndown>) =>
  b.points.map((p) => [new Date(p.t).toISOString().slice(5, 16), p.scope, p.remaining]);

describe('burndown', () => {
  it('steps scope and remaining at every creation and completion, ending now', () => {
    const b = burndown(tasks, forecastAll(tasks, NOW), NOW);
    expect(series(b)).toEqual([
      ['04-01T08:00', 10, 10],
      ['04-01T09:06', 10, 9],
      ['04-01T09:14', 10, 8],
      ['04-01T09:24', 10, 7],
      ['04-01T09:30', 10, 6],
      ['04-02T14:12', 10, 5],
      ['04-02T14:20', 10, 4],
      ['04-02T14:29', 10, 3],
      ['04-02T15:00', 15, 8],
      ['04-03T10:00', 15, 7],
      ['04-06T12:00', 15, 7],
    ]);
  });

  it('forecasts from now to the estimated finish; blocked work stays', () => {
    const b = burndown(tasks, forecastAll(tasks, NOW), NOW);
    expect(b.forecast).toEqual({
      from: { t: NOW, remaining: 7 },
      to: { t: at('2026-04-06T12:51:00Z'), remaining: 1 },
      range: { low: at('2026-04-06T12:45:00Z'), high: at('2026-04-06T13:03:00Z') },
    });
  });

  it('can focus on one sprint', () => {
    const f = forecastAll(tasks, NOW);
    const p1 = burndown(tasks, f, NOW, 'Phase 1', undefined, model);
    expect(p1.points[0]).toMatchObject({ scope: 6, remaining: 6 });
    expect(p1.points.at(-1)).toMatchObject({ scope: 6, remaining: 2 });
    // Phase 1's estimate (8.5 min) only covers T-009; T-010 is blocked
    expect(p1.forecast!.to).toEqual({ t: NOW + 8.5 * 60_000, remaining: 1 });
    const p0 = burndown(tasks, f, NOW, 'Phase 0', undefined, model);
    expect(p0.points.at(-1)!.remaining).toBe(0);
    expect(p0.forecast).toBeUndefined();
  });

  it("a focused closed sprint ends where it closed; an open sprint's due date is marked", () => {
    const meta = {
      sprints: [
        { id: 'Old', state: 'closed', start: '2026-04-01T08:00:00Z', end: '2026-04-02T16:00:00Z' },
        { id: 'Now', state: 'active', start: '2026-04-02T16:00:00Z', due: '2026-04-08' },
      ],
    };
    const inSprints = tasks.map((x) => ({ ...x, sprint: x.phase === 0 ? 'Old' : 'Now' }));
    const m = sprintModel(meta, inSprints, DEFAULT_VOCABULARY);
    const f = forecastSprints(inSprints, NOW, inSprints, m);
    const old = burndown(inSprints, f, NOW, 'Old', undefined, m);
    expect(old.end).toBe(at('2026-04-02T16:00:00Z'));
    expect(old.points.at(-1)!.t).toBe(old.end);
    expect(old.forecast).toBeUndefined();
    expect(old.due).toBeUndefined();
    // Not focused: the sprint the loop works on gives the due date
    const all = burndown(inSprints, f, NOW, undefined, undefined, m);
    expect(all.due).toBe(at('2026-04-08T23:59:59Z'));
    expect(all.end).toBeUndefined();
    expect(rangeStart('sprint', inSprints, f, NOW, undefined, m)).toBe(at('2026-04-02T16:00:00Z'));
    expect(rangeStart('sprint', inSprints, f, NOW, 'Old', m)).toBe(at('2026-04-01T08:00:00Z'));
  });

  it('uses the earliest known time for tasks without created_at', () => {
    const t = taskWith;
    const old = [t('a', 'done', { updated_at: '2026-04-02T10:00:00Z' }, false), t('b', 'todo')];
    const b = burndown(old, forecastAll(old, NOW), NOW);
    expect(series(b)).toEqual([
      ['04-02T10:00', 2, 1],
      ['04-06T12:00', 2, 1],
    ]);
  });

  it('is empty without tasks', () => {
    expect(burndown([], forecastAll([], NOW), NOW)).toEqual({ points: [] });
  });

  it("follows a project's kinds: skipped-kind out of scope; review and blocked left for the forecast's end", () => {
    // A project's own statuses: atm gives each task its kind
    const vocab = vocabulary({
      statuses: [
        { id: 'backlog', label: 'Backlog', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
        { id: 'dropped', label: 'Dropped', kind: 'skipped' },
      ],
    });
    const t = (id: string, status: string, extra: Partial<Task> = {}, agentWork = true) =>
      withKinds([taskWith(id, status, extra, agentWork)], vocab)[0];
    const created = { created_at: '2026-04-01T09:00:00Z' };
    const shipped = (id: string, h: number) =>
      t(
        id,
        'shipped',
        { ...created, started_at: `2026-04-01T${h}:00:00Z`, completed_at: `2026-04-01T${h}:10:00Z` },
        false,
      );
    const all = [
      shipped('a', 10),
      shipped('b', 11),
      shipped('c', 12),
      t('d', 'backlog', created),
      t('e', 'awaiting_user', created, false),
      t('f', 'dropped', created, false),
    ];
    const b = burndown(all, forecastSprints(all, NOW, all), NOW);
    expect(b.points.at(-1)).toMatchObject({ scope: 5, remaining: 2 });
    expect(b.forecast!.to.remaining).toBe(1);
  });
});

describe('burndown range (T-240)', () => {
  const forecast = forecastAll(tasks, NOW);

  it('"sprint" starts when the sprint the loop works on started (without a start, its first task); a focused one, when it did', () => {
    const starts = (phase: number) =>
      Math.min(...tasks.filter((t) => t.phase === phase && t.started_at).map((t) => at(t.started_at!)));
    expect(rangeStart('sprint', tasks, forecast, NOW, undefined, model)).toBe(starts(1));
    expect(rangeStart('sprint', tasks, forecast, NOW, 'Phase 0', model)).toBe(starts(0));
    expect(rangeStart('14d', tasks, forecast, NOW)).toBe(NOW - 14 * 24 * 3600_000);
    expect(rangeStart('all', tasks, forecast, NOW)).toBeUndefined();
  });

  it('a range starts the chart at its start, with the state then; nothing before it', () => {
    const from = rangeStart('sprint', tasks, forecast, NOW, undefined, model)!;
    const whole = burndown(tasks, forecast, NOW);
    const ranged = burndown(tasks, forecast, NOW, undefined, from);
    expect(ranged.points[0].t).toBe(from);
    expect(ranged.points.every((p) => p.t >= from)).toBe(true);
    const before = [...whole.points].reverse().find((p) => p.t <= from)!;
    expect(ranged.points[0]).toMatchObject({ scope: before.scope, remaining: before.remaining });
    expect(ranged.points.at(-1)).toEqual(whole.points.at(-1));
  });

  it('without any time on any task, there is no timeline', () => {
    const untimed = withKinds(
      tasks.map(({ id, name, phase, type, status, description }) => ({ id, name, phase, type, status, description })),
    );
    expect(burndown(untimed, forecastSprints(untimed, NOW), NOW).untimed).toBe(true);
    expect(burndown(tasks, forecast, NOW).untimed).toBeUndefined();
  });
});
