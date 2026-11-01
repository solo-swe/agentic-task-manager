import { describe, expect, it } from 'vitest';
import { projectStats, timeHistogram } from '../../src/lib/projectstats';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';

// Times in minutes from T0
const T0 = Date.parse('2026-05-01T08:00:00Z');
const at = (min: number) => new Date(T0 + min * 60_000).toISOString();
const t = (min: number) => T0 + min * 60_000;
const task = (id: string, fields: Partial<Task>): Task =>
  ({ id, name: id, phase: 0, type: 'feature', description: '', status: 'todo', ...fields }) as Task;

// A (a) 0–30 and B (b, interactive) 10–40 then in review until 60, both with a commit in PR #5; C skipped, D open
const TASKS = [
  task('A', {
    created_at: at(-60),
    started_by: 'claude:a',
    started_at: at(0),
    completed_at: at(30),
    status: 'done',
    commit: 'abc',
    pr: { number: 5 },
  }),
  task('B', {
    created_at: at(-30),
    started_by: 'claude:b',
    started_at: at(10),
    review_requested_at: at(40),
    completed_at: at(60),
    status: 'done',
    labels: ['interactive'],
    commit: 'def',
    pr: { number: 5 },
  }),
  task('C', { phase: 1, created_at: at(-10), status: 'skipped' }),
  task('D', { phase: 1 }),
];
const NOW = t(120);

describe('projectStats (T-253)', () => {
  const s = projectStats(TASKS, { workflow: { type: 'pull-request' } }, NOW, undefined, 2);

  it('started: the earliest created_at; duration from the first start, ongoing while tasks are open', () => {
    expect(s.started).toBe(t(-60));
    expect(s.duration).toEqual({ from: t(0), to: NOW, ongoing: true });
  });

  it('tasks: archived tasks not loaded count as done', () => {
    expect(s.tasks).toEqual({ total: 6, done: 4, open: 1, skipped: 1 });
  });

  it('sprints (T-323): how many, and how many closed; none without sprints', () => {
    expect(s.sprints).toEqual({ total: 0, closed: 0 });
    const meta = {
      sprints: [
        { id: 'One', state: 'closed' },
        { id: 'Two', state: 'active' },
      ],
    };
    const model = sprintModel(meta, TASKS, resolveVocabulary(meta));
    expect(projectStats(TASKS, {}, NOW, undefined, 0, model).sprints).toEqual({ total: 2, closed: 1 });
  });

  it('task commits and distinct pull requests', () => {
    expect([s.taskCommits, s.pullRequests]).toEqual([2, 1]);
  });

  it('agent time (alone and with you), most at once, and the average while any agent works', () => {
    expect([s.agentMs, s.aloneMs, s.withYouMs].map((v) => v / 60_000)).toEqual([60, 30, 30]);
    expect(s.maxAgents).toBe(2);
    // 60 m of agent time over 40 m (0–40) with someone at work
    expect(s.parallelism).toBe(1.5);
  });

  it('all finished: the duration ends at the last completion', () => {
    const done = TASKS.slice(0, 3);
    expect(projectStats(done, undefined, NOW).duration).toEqual({ from: t(0), to: t(60), ongoing: false });
  });

  it("without created_at, meta.created; a direct project's pull requests are –", () => {
    const s2 = projectStats([task('X', {})], { created: '2026-04-01', workflow: { type: 'direct' } }, NOW);
    expect(s2.started).toBe(Date.parse('2026-04-01'));
    expect(s2.pullRequests).toBeNull();
    expect(projectStats([task('X', { pr: { number: 3 } })], { workflow: { type: 'direct' } }, NOW).pullRequests).toBe(
      1,
    );
  });

  it("a person's own work isn't agent time", () => {
    const person = task('P', { started_by: 'Ana', started_at: at(0), completed_at: at(90), status: 'done' });
    expect(projectStats([...TASKS, person], undefined, NOW).agentMs / 60_000).toBe(60);
  });

  it('nothing started: no duration, no agent time, no parallelism', () => {
    const s3 = projectStats([task('X', {})], undefined, NOW);
    expect([s3.started, s3.duration, s3.agentMs, s3.maxAgents, s3.parallelism]).toEqual([null, null, 0, 0, null]);
  });
});

describe('timeHistogram (T-254)', () => {
  // Done tasks of 3, 7, 12, 50 m and 5 h alone, 90 m and 25 m with you (90 m until its review); an open and an
  // unstarted task are left out
  const done = (id: string, minutes: number, fields: Partial<Task> = {}) =>
    task(id, { started_at: at(0), completed_at: at(minutes), status: 'done', ...fields });
  const h = timeHistogram([
    done('a', 3),
    done('b', 7),
    done('c', 12),
    done('d', 50),
    done('e', 300),
    done('f', 120, { review_requested_at: at(90), labels: ['interactive'] }),
    done('g', 25, { labels: ['interactive'] }),
    task('open', { started_at: at(0) }),
    task('nostart', { status: 'done', completed_at: at(5) }),
  ]);
  const ids = (list: { id: string }[]) => list.map((t) => t.id);

  it('bins that widen with time, the last open-ended', () => {
    expect(h.bins.map((b) => b.label)).toEqual([
      '0 m–5 m',
      '5 m–10 m',
      '10 m–20 m',
      '20 m–30 m',
      '30 m–45 m',
      '45 m–1 h',
      '1 h–1.5 h',
      '1.5 h–2 h',
      '2 h–3 h',
      '3 h–4 h',
      '4 h+',
    ]);
    expect(h.bins.map((b) => [ids(b.alone), ids(b.withYou)])).toEqual([
      [['a'], []],
      [['b'], []],
      [['c'], []],
      [[], ['g']],
      [[], []],
      [['d'], []],
      [[], []],
      // 90 m exactly: the bin it starts
      [[], ['f']],
      [[], []],
      [[], []],
      [['e'], []],
    ]);
  });

  it('a median per kind, min, max and average, and the five slowest', () => {
    expect([h.medianAlone, h.medianWithYou]).toEqual([12, 57.5]);
    expect([h.min, h.max, h.average]).toEqual([3, 300, (3 + 7 + 12 + 50 + 300 + 90 + 25) / 7]);
    expect(ids(h.slowest)).toEqual(['e', 'f', 'd', 'g', 'c']);
  });

  it('no timed tasks: empty bins and no numbers', () => {
    const empty = timeHistogram([task('x', {})]);
    expect(empty.bins.every((b) => !b.alone.length && !b.withYou.length)).toBe(true);
    expect([empty.medianAlone, empty.min, empty.average, empty.slowest]).toEqual([null, null, null, []]);
  });
});
