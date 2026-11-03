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

describe('effectiveReview (SR-2.6): atm’s to say, for every task (T-371, T-387)', () => {
  const derived = { kind: 'todo', ready: true, waiting_for: [], priority: 'P2', archived: false };

  it('the mode atm worked out (its own, else its component’s, else the project’s), not the dashboard’s reading', () => {
    // atm says batch; the task's own field would say blocker
    const task = t('a', 'todo', { review: 'blocker', derived: { ...derived, effective_review: 'batch' } });
    expect(effectiveReview(task)).toBe('batch');
  });

  it('none without atm’s values, or with one that isn’t a mode', () => {
    expect(effectiveReview(t('a', 'todo', { review: 'blocker' }))).toBe('none');
    expect(effectiveReview(t('a', 'todo', { derived: { ...derived, effective_review: 'whatever' } }))).toBe('none');
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
