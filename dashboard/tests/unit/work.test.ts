import { describe, expect, it } from 'vitest';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { isDone, startedBy, taskTimes, workSpan } from '../../src/lib/work';
import { vocabulary, withKinds } from './views';

const H = 3_600_000;
const T0 = Date.parse('2026-04-01T08:00:00Z');
const iso = (ms: number) => new Date(ms).toISOString();
/** A task as atm serves it: with its kind in `vocab` */
const task = (status: string, fields: Partial<Task> = {}, vocab: Vocabulary = DEFAULT_VOCABULARY): Task =>
  withKinds(
    [{ id: 'T-1', name: 'x', phase: 0, type: 'feature', description: '', status, started_at: iso(T0), ...fields }],
    vocab,
  )[0];
const NOW = T0 + 10 * H;

describe('startedBy', () => {
  it('is started_by, or agent in files written before v4', () => {
    expect(startedBy(task('done', { started_by: 'alice', agent: 'old' }))).toBe('alice');
    expect(startedBy(task('done', { agent: 'old' }))).toBe('old');
    expect(startedBy(task('done'))).toBeUndefined();
  });
});

describe('workSpan', () => {
  it('runs from start to completion for a done task, and to now while active', () => {
    expect(workSpan(task('done', { completed_at: iso(T0 + 2 * H) }), NOW)).toEqual({
      start: T0,
      end: T0 + 2 * H,
      live: false,
    });
    expect(workSpan(task('in_progress'), NOW)).toEqual({ start: T0, end: NOW, live: true });
  });

  it('ends when the task asked for a review: the review is not work', () => {
    const reviewed = { review_requested_at: iso(T0 + H), completed_at: iso(T0 + 5 * H) };
    expect(workSpan(task('done', reviewed), NOW)).toEqual({ start: T0, end: T0 + H, live: false });
    expect(workSpan(task('in_review', { review_requested_at: iso(T0 + H) }), NOW)).toEqual({
      start: T0,
      end: T0 + H,
      live: false,
    });
    // An in-review task without the timestamp falls back to its last update
    expect(workSpan(task('in_review', { updated_at: iso(T0 + 3 * H) }), NOW)?.end).toBe(T0 + 3 * H);
  });

  it('holds no work when not started, or todo, blocked, skipped or unknown', () => {
    expect(workSpan(task('done', { started_at: undefined }), NOW)).toBeNull();
    for (const status of ['todo', 'blocked', 'skipped', 'weird']) expect(workSpan(task(status), NOW)).toBeNull();
  });

  it('gives a finished task without a known end a zero-length span', () => {
    expect(workSpan(task('done'), NOW)).toEqual({ start: T0, end: T0, live: false });
  });

  it("follows a project's statuses by kind", () => {
    const vocab = vocabulary({
      statuses: [
        { id: 'backlog', label: 'Backlog', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
      ],
    });
    expect(workSpan(task('doing', {}, vocab), NOW)?.live).toBe(true);
    expect(workSpan(task('shipped', { completed_at: iso(T0 + H) }, vocab), NOW)?.end).toBe(T0 + H);
    expect(isDone(task('shipped', {}, vocab))).toBe(true);
    // A status nobody defines has no kind: not done
    expect(isDone(task('shipped'))).toBe(false);
  });
});

describe('taskTimes', () => {
  const rows = (t: Task) => taskTimes(t, NOW).map((r) => [r.label, r.at ?? r.text]);
  const created = iso(T0 - H);

  it('a to-do task: only when it was created and last updated', () => {
    const t = task('todo', { started_at: undefined, created_at: created, updated_at: iso(T0) });
    expect(rows(t)).toEqual([
      ['Created', created],
      ['Updated', iso(T0)],
    ]);
  });

  it('in progress: started, and the work so far', () => {
    expect(rows(task('in_progress', { created_at: created }))).toEqual([
      ['Created', created],
      ['Started', iso(T0)],
      ['Duration', '10 h so far'],
    ]);
  });

  it('in review: the work ends when the review was asked for', () => {
    const t = task('in_review', { review_requested_at: iso(T0 + H / 2), updated_at: iso(T0 + H) });
    expect(rows(t)).toEqual([
      ['Started', iso(T0)],
      ['Sent for review', iso(T0 + H / 2)],
      ['Duration', '30 m'],
      ['Updated', iso(T0 + H)],
    ]);
  });

  it('done: finished, and how long the work took', () => {
    const t = task('done', { completed_at: iso(T0 + 2 * H + 5 * 60_000) });
    expect(rows(t)).toEqual([
      ['Started', iso(T0)],
      ['Finished', iso(T0 + 2 * H + 5 * 60_000)],
      ['Duration', '2 h 5 m'],
    ]);
  });

  it('skipped: when it was skipped, no duration', () => {
    const t = task('skipped', { started_at: undefined, skipped_at: iso(T0) });
    expect(rows(t)).toEqual([['Skipped', iso(T0)]]);
  });
});
