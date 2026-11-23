import { describe, expect, it } from 'vitest';
import type { ForecastRow } from '$generated/types';
import { burndown, rangeStart } from '../../src/lib/burndown';
import { fromServed, NO_FORECAST, type Forecast } from '../../src/lib/forecast';
import type { SprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';
import { progressTasks, sprintModel, taskWith, vocabulary, withKinds } from './views';

// Expected numbers: tests/fixtures/progress.md; atm's answers for its tasks (ATM-717): views.ts
const NOW = Date.parse('2026-04-06T12:00:00Z');
const tasks = progressTasks;
const at = (iso: string) => Date.parse(iso);
// Not migrated: its phases read as the sprints "Phase 0" (closed), "Phase 1" and "Phase 2"
const model = sprintModel(progress.meta, tasks, DEFAULT_VOCABULARY);

/** atm's forecast (ATM-722, atm-lib's rules) as a test gives it: each row's sprint (null: the backlog) and its estimate's
 * minutes (likely, low, high) from NOW, and the row the loop works on next */
function forecastWith(
  m: SprintModel,
  rows: [string | null, [number, number, number]?][],
  next: string | null = rows[0]?.[0] ?? null,
): Forecast {
  const row = ([sprint, minutes]: [string | null, [number, number, number]?]): ForecastRow => ({
    sprint,
    total: 1,
    done: 0,
    skipped: 0,
    remaining: 1,
    blocked: 0,
    in_review: 0,
    state: 'in_progress',
    ...(minutes
      ? {
          estimate: {
            minutes: { likely: minutes[0], low: minutes[1], high: minutes[2] },
            finish_at: {
              likely: NOW + minutes[0] * 60_000,
              low: NOW + minutes[1] * 60_000,
              high: NOW + minutes[2] * 60_000,
            },
          },
        }
      : {}),
  });
  return fromServed(
    { pace: { likely: 8.5, low: 7.5, high: 10.5, based_on: 8 }, sprints: rows.map(row), next: { sprint: next } },
    m,
  );
}

// The fixture's, as atm forecasts it: Phase 1's work first (8.5 minutes), then Phase 2's (to 51 in all)
const FIXTURE_FORECAST = forecastWith(
  model,
  [['Phase 0'], ['Phase 1', [8.5, 7.5, 10.5]], ['Phase 2', [51, 45, 63]]],
  'Phase 1',
);
const series = (b: ReturnType<typeof burndown>) =>
  b.points.map((p) => [new Date(p.t).toISOString().slice(5, 16), p.scope, p.remaining]);

describe('burndown', () => {
  it('steps scope and remaining at every creation and completion, ending now', () => {
    const b = burndown(tasks, FIXTURE_FORECAST, NOW);
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
    const b = burndown(tasks, FIXTURE_FORECAST, NOW);
    expect(b.forecast).toEqual({
      from: { t: NOW, remaining: 7 },
      to: { t: at('2026-04-06T12:51:00Z'), remaining: 1 },
      range: { low: at('2026-04-06T12:45:00Z'), high: at('2026-04-06T13:03:00Z') },
    });
  });

  it('can focus on one sprint', () => {
    const f = FIXTURE_FORECAST;
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
        // atm says when its due date has passed (ATM-720: the end of that day, the project's day)
        {
          id: 'Now',
          state: 'active',
          start: '2026-04-02T16:00:00Z',
          due: '2026-04-08',
          due_end: '2026-04-08T23:59:59Z',
        },
      ],
    };
    const inSprints = tasks.map((x) => ({ ...x, sprint: x.phase === 0 ? 'Old' : 'Now' }));
    const m = sprintModel(meta, inSprints, DEFAULT_VOCABULARY);
    const f = forecastWith(m, [['Old'], ['Now', [30, 20, 40]]], 'Now');
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
    const b = burndown(old, NO_FORECAST, NOW);
    expect(series(b)).toEqual([
      ['04-02T10:00', 2, 1],
      ['04-06T12:00', 2, 1],
    ]);
  });

  it('is empty without tasks', () => {
    expect(burndown([], NO_FORECAST, NOW)).toEqual({ points: [] });
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
    const b = burndown(all, forecastWith(model, [[null, [10, 10, 10]]]), NOW);
    expect(b.points.at(-1)).toMatchObject({ scope: 5, remaining: 2 });
    expect(b.forecast!.to.remaining).toBe(1);
  });
});

describe('burndown range (T-240)', () => {
  const forecast = FIXTURE_FORECAST;

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
    expect(burndown(untimed, NO_FORECAST, NOW).untimed).toBe(true);
    expect(burndown(tasks, forecast, NOW).untimed).toBeUndefined();
  });
});
