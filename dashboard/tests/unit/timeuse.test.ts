import { describe, expect, it } from 'vitest';
import { mergeIntervals, timeUse, waitingIntervals } from '../../src/lib/timeuse';
import type { Task } from '../../src/lib/types';
import { viewed } from './views';

// Times in minutes from T0
const T0 = Date.parse('2026-05-01T08:00:00Z');
const at = (min: number) => new Date(T0 + min * 60_000).toISOString();
const t = (min: number) => T0 + min * 60_000;
const minutes = (ms: number) => ms / 60_000;
const task = (id: string, fields: Partial<Task>): Task =>
  ({ id, name: id, phase: 0, type: 'feature', description: '', status: 'done', ...fields }) as Task;
const NOW = t(1000);

// a: T1 0–60, T3 65–100, T4 200–260 (blocked on you 150–200 first), T5 300–305; b: T2 30–90 then in review until
// 150, T6 300–330. Nothing between 260 and 300.
const PROJECT = [
  task('T1', { started_by: 'a', started_at: at(0), completed_at: at(60) }),
  task('T2', {
    started_by: 'b',
    started_at: at(30),
    review_requested_at: at(90),
    completed_at: at(150),
    reviews: [{ from: at(90), to: at(150) }],
  }),
  task('T3', { started_by: 'a', started_at: at(65), completed_at: at(100) }),
  task('T4', {
    started_by: 'a',
    started_at: at(200),
    completed_at: at(260),
    blocks: [{ from: at(150), to: at(200), on: 'user' }],
  }),
  task('T5', { started_by: 'a', started_at: at(300), completed_at: at(305) }),
  task('T6', { started_by: 'b', started_at: at(300), completed_at: at(330) }),
];

describe('timeUse (T-245)', () => {
  it('splits all time into agents working, waiting on you and idle; agent time adds up parallel agents', () => {
    const use = timeUse(PROJECT, null, t(330))!;
    expect([use.from, use.to]).toEqual([t(0), t(330)]);
    // Waiting: the review (90–150) and the block (150–200), less the work until 100
    expect(minutes(use.waitingMs)).toBe(100);
    expect(use.waiting).toEqual([{ start: t(100), end: t(200) }]);
    expect(use.idle).toEqual([{ start: t(260), end: t(300) }]);
    expect(minutes(use.idleMs)).toBe(40);
    expect(minutes(use.workingMs)).toBe(190);
    // a: 60 + 35 + 60 + 5, b: 60 + 30
    expect(minutes(use.agentMs)).toBe(250);
    expect(use.maxAgents).toBe(2);
  });

  it('all time runs up to now: the time since the last work is idle', () => {
    const use = timeUse(PROJECT, null, NOW)!;
    expect(use.to).toBe(NOW);
    expect(use.idle.at(-1)).toEqual({ start: t(330), end: NOW });
    expect(minutes(use.idleMs)).toBe(40 + 670);
  });

  it('a pause under 10 minutes counts as working, not idle', () => {
    const use = timeUse(
      [
        task('T1', { started_by: 'a', started_at: at(0), completed_at: at(30) }),
        task('T2', { started_by: 'a', started_at: at(35), completed_at: at(60) }),
      ],
      null,
      t(60),
    )!;
    expect(use.idle).toEqual([]);
    expect(minutes(use.workingMs)).toBe(60);
    expect(minutes(use.agentMs)).toBe(55);
    expect(use.maxAgents).toBe(1);
  });

  it('an agent on two tasks at once counts once', () => {
    const use = timeUse(
      [
        task('T1', { started_by: 'a', started_at: at(0), completed_at: at(30) }),
        task('T2', { started_by: 'a', started_at: at(10), completed_at: at(40) }),
      ],
      null,
      NOW,
    )!;
    expect([minutes(use.agentMs), use.maxAgents]).toEqual([40, 1]);
  });

  it('a period measures only its part, from its start or the first work', () => {
    const use = timeUse(PROJECT, { start: t(120), end: t(280) }, NOW)!;
    expect([use.from, use.to]).toEqual([t(120), t(280)]);
    expect([minutes(use.workingMs), minutes(use.waitingMs), minutes(use.idleMs)]).toEqual([60, 80, 20]);
    expect([minutes(use.agentMs), use.maxAgents]).toEqual([60, 1]);
    // A period before any work has nothing to show
    expect(timeUse(PROJECT, { start: t(-100), end: t(-10) }, NOW)).toBeNull();
  });

  it('split gives the same three parts for a bucket', () => {
    const use = timeUse(PROJECT, null, t(330))!;
    const inMinutes = (s: ReturnType<typeof use.split>) => [s.workingMs, s.waitingMs, s.idleMs].map(minutes);
    expect(inMinutes(use.split(t(0), t(60)))).toEqual([60, 0, 0]);
    expect(inMinutes(use.split(t(90), t(150)))).toEqual([10, 50, 0]);
    expect(inMinutes(use.split(t(250), t(310)))).toEqual([20, 0, 40]);
    // Outside the measured span: nothing
    expect(inMinutes(use.split(t(400), t(500)))).toEqual([0, 0, 0]);
  });

  it('no work: null', () => {
    expect(timeUse([task('T1', { status: 'todo' })], null, NOW)).toBeNull();
  });
});

describe('waitingIntervals', () => {
  it('a current review or block on you runs to now; a block on an agent is not waiting on you', () => {
    // In review, as atm says who it waits on (ATM-717): its reviewer, or, approved or sent back, its agent
    const inReview = (on: 'reviewer' | 'agent') =>
      viewed(task('T1', { status: 'in_review', review_requested_at: at(10) }), { waiting: { on } });
    expect(waitingIntervals(inReview('reviewer'), NOW)).toEqual([{ start: t(10), end: NOW }]);
    expect(waitingIntervals(inReview('agent'), NOW)).toEqual([]);
    expect(waitingIntervals(task('T1', { status: 'blocked', waiting_on: 'user', blocked_at: at(20) }), NOW)).toEqual([
      { start: t(20), end: NOW },
    ]);
    expect(waitingIntervals(task('T1', { status: 'blocked', waiting_on: 'agent', blocked_at: at(20) }), NOW)).toEqual(
      [],
    );
    expect(
      waitingIntervals(task('T1', { status: 'todo', blocks: [{ from: at(0), to: at(5), on: 'agent' }] }), NOW),
    ).toEqual([]);
  });

  it("files from before atm kept reviews: a done task's review runs from its request to its completion, once", () => {
    const old = task('T1', { review_requested_at: at(10), completed_at: at(40) });
    expect(waitingIntervals(old, NOW)).toEqual([{ start: t(10), end: t(40) }]);
    const recorded = { ...old, reviews: [{ from: at(10), to: at(40) }] };
    expect(waitingIntervals(recorded, NOW)).toEqual([{ start: t(10), end: t(40) }]);
  });
});

describe('mergeIntervals', () => {
  it('sorts, joins overlapping and touching intervals, and drops empty ones', () => {
    expect(
      mergeIntervals([
        { start: 5, end: 8 },
        { start: 0, end: 2 },
        { start: 2, end: 3 },
        { start: 6, end: 7 },
        { start: 9, end: 9 },
      ]),
    ).toEqual([
      { start: 0, end: 3 },
      { start: 5, end: 8 },
    ]);
  });
});
