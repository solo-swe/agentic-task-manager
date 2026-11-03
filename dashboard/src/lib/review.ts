// Review modes and states (docs/sprints-and-releases-design.md SR-2.6, SR-6.1): how a task is reviewed, as atm
// decides it, and the words the cards and the detail panel use.
import type { Task } from './types';

export const REVIEW_MODES = ['blocker', 'async', 'batch', 'none'] as const;
export type ReviewMode = (typeof REVIEW_MODES)[number];

const isMode = (v: unknown): v is ReviewMode => REVIEW_MODES.includes(v as ReviewMode);

/**
 * A task's review mode, as atm says it (`derived.effective_review`, T-371: its own, else its first component's that
 * has one, else the project's), for every task: the working ones, the archived ones and a linked worktree's copies
 * (T-387) all come with atm's values. None when atm gives none.
 */
export function effectiveReview(task: Task): ReviewMode {
  return isMode(task.derived?.effective_review) ? task.derived.effective_review : 'none';
}

/** The review state as a short label: "Changes requested", "Approved", "Waiting for review" */
export const REVIEW_STATE_LABELS: Record<string, string> = {
  waiting: 'Waiting for review',
  changes_requested: 'Changes requested',
  approved: 'Approved',
};
