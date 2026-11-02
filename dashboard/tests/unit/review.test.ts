import { describe, expect, it } from 'vitest';
import { effectiveReview } from '../../src/lib/review';
import type { Task } from '../../src/lib/types';
import { needsYouItems, needsYouReason } from '../../src/lib/waiting';

const t = (id: string, status: string, extra: Partial<Task> = {}): Task => ({
  id,
  name: id,
  phase: 0,
  type: 'feature',
  description: '',
  status,
  ...extra,
});

describe('effectiveReview (SR-2.6)', () => {
  const meta = {
    review: 'async',
    components: [{ id: 'ui', review: 'batch' }, { id: 'docs' }, 'junk'],
  };

  it("the task's own mode, else its first component's that has one, else the project's, else none", () => {
    expect(effectiveReview(meta, t('a', 'todo', { review: 'blocker' }))).toBe('blocker');
    expect(effectiveReview(meta, t('a', 'todo', { components: ['docs', 'ui'] }))).toBe('batch');
    expect(effectiveReview(meta, t('a', 'todo', { components: ['docs'] }))).toBe('async');
    expect(effectiveReview({}, t('a', 'todo', { review: 'whatever' }))).toBe('none');
  });

  it('is atm’s to say when the task comes with atm’s values (T-371)', () => {
    const derived = { kind: 'todo', ready: true, waiting_for: [], priority: 'P2', archived: false };
    // atm says batch; the dashboard's own reading of the task would say blocker
    const task = t('a', 'todo', { review: 'blocker', derived: { ...derived, effective_review: 'batch' } });
    expect(effectiveReview(meta, task)).toBe('batch');
    // Empty values (a fixture's, a worktree copy's) leave it to the dashboard
    expect(effectiveReview(meta, t('a', 'todo', { review: 'blocker', derived: {} as Task['derived'] }))).toBe(
      'blocker',
    );
  });
});

describe('needs you (SR-6.6)', () => {
  it('in review and waiting for the reviewer, blocked on the user, or to do together with the user', () => {
    expect(needsYouReason(t('a', 'in_review'))).toBe('review');
    expect(needsYouReason(t('a', 'in_review', { review_state: 'waiting' }))).toBe('review');
    expect(needsYouReason(t('a', 'blocked', { waiting_on: 'user' }))).toBe('blocked');
    expect(needsYouReason(t('a', 'todo', { with: 'user' }))).toBe('together');
  });

  it("not: back from review (an agent's work again), blocked on an agent, finished work done together", () => {
    expect(needsYouReason(t('a', 'in_review', { review_state: 'approved' }))).toBeUndefined();
    expect(needsYouReason(t('a', 'in_review', { review_state: 'changes_requested' }))).toBeUndefined();
    expect(needsYouReason(t('a', 'blocked', { waiting_on: 'agent' }))).toBeUndefined();
    expect(needsYouReason(t('a', 'done', { with: 'user' }))).toBeUndefined();
  });

  it('lists them with their reasons', () => {
    expect(needsYouItems([t('a', 'todo', { with: 'user' }), t('b', 'todo'), t('c', 'in_review')])).toEqual([
      { id: 'a', name: 'a', reason: 'together' },
      { id: 'c', name: 'c', reason: 'review' },
    ]);
  });
});
