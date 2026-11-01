import { describe, expect, it } from 'vitest';
import { BACKLOG } from '../../src/lib/filters';
import {
  forecastSprints,
  nextInLoop,
  pace,
  percentile,
  sprintKey,
  taskDurations,
  withWholeSprintEstimates,
} from '../../src/lib/forecast';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, resolveVocabulary } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';

// Expected numbers: tests/fixtures/progress.md
const NOW = Date.parse('2026-04-06T12:00:00Z');
const tasks = progress.tasks as Task[];
// Not migrated: its phases read as the sprints "Phase 0" (closed: all done), "Phase 1" and "Phase 2" (planned)
const model = sprintModel(progress.meta, tasks, resolveVocabulary(progress.meta));
const forecast = (list: Task[], history: Task[] = list) =>
  forecastSprints(list, NOW, history, DEFAULT_VOCABULARY, model);
const t = (id: string, status: string, extra: Partial<Task> = {}): Task => ({
  id,
  name: id,
  phase: 0,
  type: 'feature',
  description: '',
  status,
  ...extra,
});

describe('taskDurations', () => {
  it('uses start/complete times, falls back to the gap since the previous completion, and caps idle gaps', () => {
    expect(taskDurations(tasks).map((d) => [d.id, d.minutes])).toEqual([
      ['T-001', 6],
      ['T-002', 8],
      ['T-003', 10],
      ['T-004', 6],
      ['T-005', 12],
      ['T-006', 8],
      ['T-007', 9],
      ['T-008', 30],
    ]);
  });

  it('leaves out a first completion with nothing to measure from', () => {
    expect(taskDurations([t('a', 'done', { updated_at: '2026-04-01T10:00:00Z' })])).toEqual([]);
  });

  it('ignores a started_at after the completion and unusable timestamps', () => {
    const d = taskDurations([
      t('a', 'done', { completed_at: '2026-04-01T10:00:00Z', started_at: '2026-04-01T09:50:00Z' }),
      t('b', 'done', { completed_at: '2026-04-01T10:05:00Z', started_at: '2026-04-01T11:00:00Z' }),
      t('c', 'done', { completed_at: 'nonsense' }),
      t('d', 'todo', { started_at: '2026-04-01T09:00:00Z' }),
    ]);
    expect(d.map((x) => [x.id, x.minutes])).toEqual([
      ['a', 10],
      ['b', 5],
    ]);
  });
});

describe('percentile and pace', () => {
  it('interpolates between ranks', () => {
    const s = [6, 6, 8, 8, 9, 10, 12, 30];
    expect([percentile(s, 0.25), percentile(s, 0.5), percentile(s, 0.75)]).toEqual([7.5, 8.5, 10.5]);
    expect(percentile([], 0.5)).toBeNaN();
  });

  it('is the median of recent durations with an interquartile range', () => {
    expect(pace(taskDurations(tasks))).toEqual({ likely: 8.5, low: 7.5, high: 10.5, basedOn: 8 });
  });

  it('uses only the last 10 completions', () => {
    const many = Array.from({ length: 15 }, (_, i) => ({ id: String(i), completedAt: i, minutes: i < 5 ? 100 : 1 }));
    expect(pace(many)).toMatchObject({ likely: 1, basedOn: 10 });
  });

  it('needs at least 3 timed completions', () => {
    expect(
      pace([
        { id: 'a', completedAt: 0, minutes: 5 },
        { id: 'b', completedAt: 1, minutes: 5 },
      ]),
    ).toBeNull();
  });
});

describe('forecastSprints', () => {
  const f = forecast(tasks);

  it('reports per-sprint progress and state; skipped tasks count apart, not in the total', () => {
    expect(f.sprints.map((s) => [s.key, s.done, s.total, s.skipped, s.remaining, s.blocked, s.state])).toEqual([
      ['Phase 0', 4, 4, 0, 0, 0, 'done'],
      ['Phase 1', 4, 6, 0, 1, 1, 'in_progress'],
      ['Phase 2', 0, 5, 1, 5, 0, 'not_started'],
    ]);
    expect(f.sprints.map((s) => s.sprint?.state)).toEqual(['closed', 'planned', 'planned']);
  });

  it('dates finished sprints by their last completion', () => {
    expect(f.sprints[0].completedAt).toBe(Date.parse('2026-04-01T09:30:00Z'));
    expect(f.sprints[0].estimate).toBeUndefined();
  });

  it("accumulates remaining work in the loop's order", () => {
    expect(f.sprints[1].estimate!.minutes).toEqual({ likely: 8.5, low: 7.5, high: 10.5 });
    expect(f.sprints[2].estimate!.minutes).toEqual({ likely: 51, low: 45, high: 63 });
    expect(f.sprints[2].estimate!.finishAt.likely).toBe(Date.parse('2026-04-06T12:51:00Z'));
    expect(nextInLoop(f)?.key).toBe('Phase 1');
  });

  it('works the active sprint first, then the planned ones in order, then the backlog; a closed one not at all', () => {
    const meta = {
      sprints: [
        { id: 'Old', state: 'closed' },
        { id: 'Next', state: 'planned' },
        { id: 'Now', state: 'active' },
      ],
    };
    const timed = tasks.filter((x) => x.status === 'done');
    const work = [
      ...timed.map((x) => ({ ...x, sprint: 'Old' })),
      t('a', 'todo', { sprint: 'Old' }),
      t('b', 'todo', { sprint: 'Next' }),
      t('c', 'todo', { sprint: 'Now' }),
      t('d', 'todo'),
    ];
    const m = sprintModel(meta, work, resolveVocabulary(meta));
    const rows = forecastSprints(work, NOW, work, DEFAULT_VOCABULARY, m).sprints;
    // Listed in that order too, the closed ones first (T-323): Now was started after Next was planned
    expect(rows.map((s) => [s.key, s.estimate?.minutes.likely])).toEqual([
      ['Old', undefined],
      ['Now', 8.5],
      ['Next', 2 * 8.5],
      [BACKLOG, 3 * 8.5],
    ]);
    expect(sprintKey(m, work.at(-1)!)).toBe(BACKLOG);
  });

  it('without sprints, every task is in the backlog', () => {
    expect(forecastSprints(tasks, NOW).sprints.map((s) => [s.key, s.total])).toEqual([[BACKLOG, 15]]);
  });

  it('gives no estimate without enough history', () => {
    const few = forecastSprints([t('a', 'done', { completed_at: '2026-04-01T10:00:00Z' }), t('b', 'todo')], NOW);
    expect(few.pace).toBeNull();
    expect(few.sprints[0].estimate).toBeUndefined();
  });

  it('gives no estimate for a sprint where only blocked work is left', () => {
    const blockedOnly = [...tasks.filter((x) => x.phase !== 1), t('x', 'blocked', { phase: 1 })];
    const p1 = forecast(blockedOnly).sprints[1];
    expect([p1.key, p1.state, p1.remaining, p1.blocked, p1.estimate]).toEqual([
      'Phase 1',
      'not_started',
      0,
      1,
      undefined,
    ]);
  });

  it('can measure the pace on other tasks than the ones it estimates', () => {
    const phase2 = tasks.filter((x) => x.phase === 2);
    expect(forecast(phase2).pace).toBeNull();
    const withHistory = forecast(phase2, tasks);
    expect(withHistory.pace?.likely).toBe(8.5);
    expect(withHistory.sprints[0].estimate!.minutes.likely).toBe(5 * 8.5);
  });

  it('leaves tasks labelled interactive out of the pace: they include waiting on the user (T-240)', () => {
    const base = forecast(tasks).pace!;
    // The slowest timed task, done with the user: without it, the pace may only get faster
    const slowest = taskDurations(tasks).reduce((a, b) => (b.minutes > a.minutes ? b : a));
    const labelled = tasks.map((x) => (x.id === slowest.id ? { ...x, labels: ['interactive'] } : x));
    const withInteractive = forecast(labelled).pace!;
    expect(withInteractive.basedOn).toBe(base.basedOn - 1);
    expect(withInteractive.likely).toBeLessThanOrEqual(base.likely);
  });

  it("a slice keeps its own counts, but each sprint's finish is the whole sprint's (T-239)", () => {
    const whole = forecast(tasks);
    // One task of phase 2: alone it would be estimated as 1 task after phase 1's, not phase 2's five
    const slice = forecast(
      tasks.filter((x) => x.phase !== 2 || x.id === tasks.find((y) => y.phase === 2 && y.status === 'todo')!.id),
      tasks,
    );
    const merged = withWholeSprintEstimates(slice, whole);
    expect(merged.sprints.map((s) => [s.key, s.total])).toEqual(slice.sprints.map((s) => [s.key, s.total]));
    expect(merged.sprints[2].estimate).toEqual(whole.sprints[2].estimate);
    expect(slice.sprints[2].estimate).not.toEqual(whole.sprints[2].estimate);
    expect(merged.sprints[0].estimate).toBeUndefined(); // finished
  });
});

describe('status kinds', () => {
  const vocab = resolveVocabulary({
    statuses: [
      { id: 'backlog', kind: 'todo' },
      { id: 'doing', kind: 'active' },
      { id: 'awaiting_user', kind: 'review' },
      { id: 'shipped', kind: 'done' },
      { id: 'on_hold', kind: 'blocked' },
    ],
  });
  const done = (id: string, start: string, end: string, extra: Partial<Task> = {}) =>
    t(id, 'shipped', { started_at: start, completed_at: end, ...extra });
  const history = [
    done('a', '2026-04-01T10:00:00Z', '2026-04-01T10:10:00Z'),
    done('b', '2026-04-01T11:00:00Z', '2026-04-01T11:10:00Z'),
    // Its review took 5 hours: not work, so it measures 10 minutes too
    done('c', '2026-04-01T12:00:00Z', '2026-04-01T17:10:00Z', { review_requested_at: '2026-04-01T12:10:00Z' }),
  ];

  it('measures work until the review was requested', () => {
    expect(taskDurations(history, vocab).map((d) => d.minutes)).toEqual([10, 10, 10]);
  });

  it('counts only tasks that need agent work as remaining: in review and blocked are waiting on someone', () => {
    const phase = [
      ...history,
      t('d', 'backlog'),
      t('e', 'doing'),
      t('f', 'awaiting_user'),
      t('g', 'on_hold'),
      t('h', 'mystery'),
    ];
    const [row] = forecastSprints(phase, NOW, phase, vocab).sprints;
    expect(row).toMatchObject({ total: 8, done: 3, remaining: 3, blocked: 1, inReview: 1, state: 'in_progress' });
    expect(row.estimate!.minutes.likely).toBe(3 * 10);
  });

  it('a sprint with only reviews left is not finished, and has nothing to estimate', () => {
    const [row] = forecastSprints([...history, t('f', 'awaiting_user')], NOW, history, vocab).sprints;
    expect(row).toMatchObject({ remaining: 0, inReview: 1, state: 'in_progress' });
    expect(row.estimate).toBeUndefined();
  });
});
