// The words for who a task waits on. Who it waits on, whether it needs you and whether agent work is left are atm's
// (atm-lib's `waiting`, its own table of cases, ATM-717): these tests give the tasks atm's values and check what shows.
import { describe, expect, it } from 'vitest';
import type { TaskDerived, TaskWaiting } from '$generated/types';
import type { Task } from '../../src/lib/types';
import { latestNote, needingUser, needsYouItems, waitLine, waitText } from '../../src/lib/waiting';

const M = 60_000;
const T0 = Date.parse('2026-04-01T08:00:00Z');
const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z');
const derived = (kind: string, more: Partial<TaskDerived> = {}): TaskDerived => ({
  kind,
  ready: false,
  waiting_for: [],
  priority: 'P2',
  archived: false,
  ...more,
});
const task = (status: string, fields: Partial<Task> = {}): Task => ({
  id: 'T-1',
  name: 'x',
  phase: 0,
  type: 'feature',
  description: '',
  status,
  ...fields,
});
const waiting = (on: TaskWaiting['on'], minutesAgo?: number, questions?: string[]): TaskWaiting => ({
  on,
  ...(minutesAgo === undefined ? {} : { since: iso(T0 - minutesAgo * M) }),
  ...(questions ? { questions } : {}),
});

describe('waitLine', () => {
  it('says who a task in review or blocked waits on, as atm says it, for how long and how many questions', () => {
    const line = (kind: string, w: TaskWaiting) => waitLine(task('x', { derived: derived(kind, { waiting: w }) }), T0);
    for (const [kind, w, text] of [
      ['review', waiting('reviewer', 130), 'Waiting for review · 2 h 10 m'],
      ['review', waiting('reviewer', 0.2), 'Waiting for review · just now'],
      // ATM-711's card: approved, or sent back, it is back with its agent, not waiting for the review
      ['review', waiting('agent', 10), 'Back with its agent · 10 m'],
      ['review', waiting('user', 5), 'Waiting on you · 5 m'],
      ['blocked', waiting('user', 125, ['a', 'b']), 'Waiting on you · 2 h 5 m · 2 questions'],
      ['blocked', waiting('agent', undefined, ['x']), 'Waiting on an agent · 1 question'],
      ['blocked', waiting('unknown', 3), 'Blocked · 3 m'],
    ] as const)
      expect(line(kind, w)?.text, `${kind} ${w.on}`).toBe(text);
    expect(line('review', waiting('reviewer'))).toMatchObject({ kind: 'review', onUser: false });
    expect(line('blocked', waiting('user', 1, ['a', 'b']))).toMatchObject({
      kind: 'block',
      onUser: true,
      questions: ['a', 'b'],
    });
  });

  it('comes with the review note while the review waits for the reviewer', () => {
    const notes = [
      { at: iso(T0 - 9 * M), kind: 'review', text: 'Old' },
      { at: iso(T0 - 2 * M), kind: 'review', text: 'Check the toolbar' },
    ];
    const at = (on: TaskWaiting['on']) =>
      waitLine(task('in_review', { notes, derived: derived('review', { waiting: waiting(on, 2) }) }), T0);
    expect(at('reviewer')?.note?.text).toBe('Check the toolbar');
    expect(at('agent')?.note).toBeUndefined();
  });

  it('is none for a task atm says waits on nobody, on its dependencies or on work with you, or says nothing of', () => {
    expect(waitLine(task('todo', { derived: derived('todo') }), T0)).toBeNull();
    expect(waitLine(task('todo', { derived: derived('todo', { waiting: waiting('dependencies') }) }), T0)).toBeNull();
    expect(waitLine(task('todo', { derived: derived('todo', { waiting: waiting('user') }) }), T0)).toBeNull();
    expect(waitLine(task('in_review', { review_requested_at: iso(T0) }), T0)).toBeNull();
  });
});

describe('waitText', () => {
  it('the All projects board’s cards, from a summary’s card: the same words', () => {
    expect(waitText({ on: 'reviewer', since: iso(T0 - 60 * M) }, 'review', T0)).toBe('Waiting for review · 1 h');
    expect(waitText({ on: 'user', since: iso(T0 - 60 * M), questions: ['', ''] }, 'blocked', T0)).toBe(
      'Waiting on you · 1 h · 2 questions',
    );
    expect(waitText({ on: 'reviewer' }, 'review', T0)).toBe('Waiting for review');
  });
});

describe('needingUser', () => {
  const needs = (id: string, reason?: TaskDerived['needs_you'], more: Partial<Task> = {}) =>
    task('x', { id, name: id.toLowerCase(), derived: derived('review', reason ? { needs_you: reason } : {}), ...more });

  it('lists the tasks atm says need you, with why, for the chips', () => {
    const tasks = [needs('A', 'review'), needs('B', 'blocked'), needs('C'), needs('D', 'together'), task('todo')];
    expect(needingUser(tasks).map((t) => t.id)).toEqual(['A', 'B', 'D']);
    expect(needsYouItems(tasks)).toEqual([
      { id: 'A', name: 'a', reason: 'review' },
      { id: 'B', name: 'b', reason: 'blocked' },
      { id: 'D', name: 'd', reason: 'together' },
    ]);
    expect(needsYouItems([needs('K', 'review', { key: 'ATM-7' })])).toEqual([
      { id: 'K', key: 'ATM-7', name: 'k', reason: 'review' },
    ]);
  });

  // ATM-640: listed once, as its review, in the same chip; matched by the review's ID (its number can change)
  it('a task in review through a review waiting on you is left out; through another, or blocked, it stays', () => {
    const cr = (id: string) => ({ id, ref: 'CR-1' });
    const tasks = [
      needs('A', 'review', { code_reviews: [cr('r-old'), cr('r1')] }),
      needs('C', 'review', { code_reviews: [cr('r2')] }),
      needs('D', 'review'),
      needs('B', 'blocked', { code_reviews: [cr('r1')] }),
    ];
    expect(needsYouItems(tasks, [{ id: 'r1' }]).map((t) => t.id)).toEqual(['C', 'D', 'B']);
    expect(needsYouItems(tasks, []).map((t) => t.id)).toEqual(['A', 'C', 'D', 'B']);
  });
});

describe('latestNote', () => {
  it('finds no note when there is none of that kind', () => {
    expect(latestNote(task('todo'), 'review')).toBeUndefined();
  });
});
