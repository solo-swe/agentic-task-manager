import { describe, expect, it } from 'vitest';
import { effectiveReview } from '../../src/lib/review';
import type { Task } from '../../src/lib/types';

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
