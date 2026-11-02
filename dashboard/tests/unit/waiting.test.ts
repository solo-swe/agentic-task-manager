import { describe, expect, it } from 'vitest';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import {
  blockSummary,
  blockWait,
  latestNote,
  needingUser,
  needsYouItems,
  reviewSummary,
  reviewWait,
} from '../../src/lib/waiting';

const M = 60_000;
const T0 = Date.parse('2026-04-01T08:00:00Z');
const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z');
const task = (status: string, fields: Partial<Task> = {}): Task => ({
  id: 'T-1',
  name: 'x',
  phase: 0,
  type: 'feature',
  description: '',
  status,
  ...fields,
});

describe('reviewWait', () => {
  it('measures from the review request, with the latest review note', () => {
    const t = task('in_review', {
      review_requested_at: iso(T0),
      notes: [
        { at: iso(T0 - 90 * M), kind: 'review', text: 'first try' },
        { at: iso(T0), by: 'claude:1', kind: 'review', text: 'please check the API names' },
        { at: iso(T0 + M), kind: 'note', text: 'unrelated' },
      ],
    });
    expect(reviewWait(t, T0 + 130 * M)).toEqual({
      since: T0,
      minutes: 130,
      note: { at: iso(T0), by: 'claude:1', kind: 'review', text: 'please check the API names' },
    });
  });

  it('falls back to the last update for a task set to review by hand, and is null for other kinds', () => {
    expect(reviewWait(task('in_review', { updated_at: iso(T0) }), T0 + 5 * M)).toEqual({ since: T0, minutes: 5 });
    expect(reviewWait(task('in_review'), T0)).toBeNull();
    expect(reviewWait(task('in_progress', { review_requested_at: iso(T0) }), T0)).toBeNull();
  });

  it("follows a project's statuses by kind", () => {
    const vocab = resolveVocabulary({
      statuses: [
        { id: 'todo', kind: 'todo' },
        { id: 'doing', kind: 'active' },
        { id: 'awaiting_user', kind: 'review' },
        { id: 'done', kind: 'done' },
      ],
    });
    expect(reviewWait(task('awaiting_user', { review_requested_at: iso(T0) }), T0, vocab)?.minutes).toBe(0);
  });
});

describe('blockWait', () => {
  it('says who decides, since when, and the questions', () => {
    const t = task('blocked', {
      waiting_on: 'user',
      blocked_at: iso(T0),
      questions: ['REST or GraphQL?', 'Which DB?'],
    });
    const wait = blockWait(t, T0 + 125 * M)!;
    expect(wait).toEqual({ on: 'user', minutes: 125, questions: ['REST or GraphQL?', 'Which DB?'] });
    expect(blockSummary(wait)).toBe('Waiting on you · 2 h 5 m · 2 questions');
    expect(blockSummary(blockWait(task('blocked', { waiting_on: 'agent', questions: ['x'] }), T0)!)).toBe(
      'Waiting on an agent · 1 question',
    );
  });

  it('a task blocked by hand says only what it has; with nothing to add, or of another kind, null', () => {
    expect(blockSummary(blockWait(task('blocked', { blocked_at: iso(T0) }), T0 + 3 * M)!)).toBe('Blocked · 3 m');
    expect(blockWait(task('blocked', { waiting_on: 'boss' }), T0)).toBeNull();
    expect(blockWait(task('todo', { waiting_on: 'user' }), T0)).toBeNull();
  });
});

describe('needingUser', () => {
  it('counts tasks in review and tasks blocked on the user, not those blocked on an agent', () => {
    const tasks = [
      task('in_review', { id: 'A' }),
      task('blocked', { id: 'B', waiting_on: 'user' }),
      task('blocked', { id: 'C', waiting_on: 'agent' }),
      task('blocked', { id: 'D' }),
      task('todo', { id: 'E', waiting_on: 'user' }),
    ];
    expect(needingUser(tasks).map((t) => t.id)).toEqual(['A', 'B']);
  });

  it('lists them for the chips with why, as atm serve does', () => {
    const tasks = [
      task('in_review', { id: 'A', name: 'Alpha' }),
      task('blocked', { id: 'B', name: 'Beta', waiting_on: 'user' }),
      task('todo', { id: 'E', name: 'Eps' }),
    ];
    expect(needsYouItems(tasks)).toEqual([
      { id: 'A', name: 'Alpha', reason: 'review' },
      { id: 'B', name: 'Beta', reason: 'blocked' },
    ]);
  });

  it('with you: only the active sprint’s, with their dependencies done (T-347)', () => {
    const tasks = [
      task('todo', { id: 'W1', with: 'user', sprint: 'Now' }),
      task('todo', { id: 'W2', with: 'user', sprint: 'Later' }),
      task('todo', { id: 'W3', with: 'user' }),
      task('todo', { id: 'W4', with: 'user', sprint: 'Now', dependencies: ['D1'] }),
      task('todo', { id: 'W5', with: 'user', sprint: 'Now', dependencies: ['D2', 'ARCHIVED'] }),
      task('in_progress', { id: 'D1', sprint: 'Now' }),
      task('done', { id: 'D2', sprint: 'Now' }),
      task('in_review', { id: 'R', sprint: 'Later' }),
    ];
    const meta = {
      sprints: [
        { id: 'Now', state: 'active' },
        { id: 'Later', state: 'planned' },
      ],
    };
    const vocab = resolveVocabulary(meta);
    const scope = { model: sprintModel(meta, tasks, vocab), tasks: new Map(tasks.map((t) => [t.id, t])) };
    // Reviews count wherever they are; W2 is in a planned sprint, W3 in the backlog, W4 waits on D1
    expect(needingUser(tasks, vocab, scope).map((t) => t.id)).toEqual(['W1', 'W5', 'R']);
    // Without an active sprint, any sprint's, still only when ready
    const none = { sprints: [{ id: 'Now', state: 'planned' }] };
    const idle = { model: sprintModel(none, tasks, resolveVocabulary(none)), tasks: scope.tasks };
    expect(needingUser(tasks, vocab, idle).map((t) => t.id)).toEqual(['W1', 'W2', 'W3', 'W5', 'R']);
    // Without a scope (older callers), every with-you task, as before
    expect(needingUser(tasks, vocab).map((t) => t.id)).toEqual(['W1', 'W2', 'W3', 'W4', 'W5', 'R']);
  });

  it('with you: whether its dependencies are done is atm’s to say when the API gives it (T-366)', () => {
    const derived = (ready: boolean) => ({ kind: 'todo', ready, waiting_for: [], priority: 'P2', archived: false });
    const tasks = [
      // Waits on a task this page can't see, which atm counts as done (archived)
      task('todo', { id: 'W1', with: 'user', sprint: 'Now', dependencies: ['GONE'], derived: derived(true) }),
      // Its dependency looks done here, but atm says it isn't ready
      task('todo', { id: 'W2', with: 'user', sprint: 'Now', dependencies: ['D'], derived: derived(false) }),
      task('done', { id: 'D', sprint: 'Now' }),
    ];
    const meta = { sprints: [{ id: 'Now', state: 'active' }] };
    const vocab = resolveVocabulary(meta);
    const scope = { model: sprintModel(meta, tasks, vocab), tasks: new Map(tasks.map((t) => [t.id, t])) };
    expect(needingUser(tasks, vocab, scope).map((t) => t.id)).toEqual(['W1']);
  });
});

describe('summaries', () => {
  it('says how long', () => {
    expect(reviewSummary({ since: T0, minutes: 130 })).toBe('Waiting for review · 2 h 10 m');
    expect(reviewSummary({ since: T0, minutes: 0.2 })).toBe('Waiting for review · just now');
  });

  it('finds no note when there is none of that kind', () => {
    expect(latestNote(task('todo'), 'review')).toBeUndefined();
  });
});
