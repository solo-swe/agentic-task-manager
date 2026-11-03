// Tasks waiting on someone rather than on agent work (docs/v4/dashboard.md section 3): how long, and what was said.
import { formatMinutes } from './format';
import type { NeedsYouItem } from './generated/types';
import { activeSprint, type SprintModel } from './sprints';
import type { Note, Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, type Vocabulary } from './vocab';

export interface ReviewWait {
  /** When the review was asked for */
  since: number;
  minutes: number;
  /** The note left with the request, if any */
  note?: Note;
}

/** The newest note of `kind`, if any. */
export function latestNote(task: Task, kind: string): Note | undefined {
  return (task.notes ?? []).filter((n) => n.kind === kind).sort((a, b) => b.at.localeCompare(a.at))[0];
}

/**
 * How long a review-kind task has waited, from review_requested_at (else its last update, for tasks set to review by
 * hand); null for other tasks.
 */
export function reviewWait(task: Task, now: number, vocab: Vocabulary = DEFAULT_VOCABULARY): ReviewWait | null {
  if (kindOf(vocab, task.status) !== 'review') return null;
  const since = Date.parse(task.review_requested_at ?? task.updated_at ?? '');
  if (Number.isNaN(since)) return null;
  const note = latestNote(task, 'review');
  return { since, minutes: Math.max(0, (now - since) / 60_000), ...(note ? { note } : {}) };
}

/** "Waiting for review · 2 h 10 m" */
export function reviewSummary(wait: ReviewWait): string {
  return `Waiting for review · ${since(wait.minutes)}`;
}

const since = (minutes: number) => (minutes < 1 ? 'just now' : formatMinutes(minutes));

export interface BlockWait {
  /** Who decides: the user, an agent, or nobody said (a task set to blocked by hand) */
  on: 'user' | 'agent' | null;
  /** Minutes since blocked_at, when known */
  minutes: number | null;
  /** The open questions */
  questions: string[];
}

/**
 * What a blocked-kind task waits on (`atm block`); null for other tasks, and for a task blocked by hand with nothing
 * more to say (its status badge already says Blocked).
 */
export function blockWait(task: Task, now: number, vocab: Vocabulary = DEFAULT_VOCABULARY): BlockWait | null {
  if (kindOf(vocab, task.status) !== 'blocked') return null;
  const on = task.waiting_on === 'user' || task.waiting_on === 'agent' ? task.waiting_on : null;
  const at = Date.parse(task.blocked_at ?? '');
  const minutes = Number.isNaN(at) ? null : Math.max(0, (now - at) / 60_000);
  const questions = Array.isArray(task.questions) ? task.questions.filter((q) => typeof q === 'string') : [];
  if (on === null && minutes === null && questions.length === 0) return null;
  return { on, minutes, questions };
}

/** "Waiting on you · 2 h · 2 questions"; "Blocked · 3 d" for a task blocked by hand */
export function blockSummary(wait: BlockWait): string {
  const parts = [wait.on === 'user' ? 'Waiting on you' : wait.on === 'agent' ? 'Waiting on an agent' : 'Blocked'];
  if (wait.minutes !== null) parts.push(since(wait.minutes));
  const n = wait.questions.length;
  if (n) parts.push(`${n} question${n === 1 ? '' : 's'}`);
  return parts.join(' · ');
}

/** Review states that hand the task back to an agent: no longer waiting on the reviewer (SR-6.1) */
export const REWORK_STATES = ['changes_requested', 'approved'];

/**
 * Which to-do tasks marked `with: user` need the user now (T-347): those in the active sprint (any, without one) that
 * atm says are ready (their dependencies done, an archived one counting as done).
 */
export interface NeedsYouScope {
  model: SprintModel;
}

function togetherNow(task: Task, scope?: NeedsYouScope): boolean {
  if (!scope) return true;
  const active = activeSprint(scope.model);
  if (active && scope.model.sprintOf(task) !== active.id) return false;
  // Its dependencies done, as atm says it (T-366), for every task, a worktree's copy too (T-387)
  return task.derived?.ready === true;
}

/** Why a task needs the user (D-3.3, SR-6.6): in review and waiting for the reviewer, blocked on a question for the
 * user, or to do together with the user (`with: user`) now (`scope`, T-347); undefined when it doesn't. */
export function needsYouReason(
  task: Task,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  scope?: NeedsYouScope,
): NeedsYouItem['reason'] | undefined {
  const kind = kindOf(vocab, task.status);
  if (kind === 'review') return REWORK_STATES.includes(task.review_state ?? '') ? undefined : 'review';
  if (kind === 'blocked' && task.waiting_on === 'user') return 'blocked';
  if (kind === 'todo' && task.with === 'user' && togetherNow(task, scope)) return 'together';
  return undefined;
}

/** Tasks that need the user */
export function needingUser(tasks: Task[], vocab: Vocabulary = DEFAULT_VOCABULARY, scope?: NeedsYouScope): Task[] {
  return tasks.filter((t) => needsYouReason(t, vocab, scope));
}

/** A task that needs the user, and why: what the "need you" chips list (also atm serve's `needs_you_tasks`) */
export type { NeedsYouItem };

/** needingUser as the chips list it */
export function needsYouItems(
  tasks: Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  scope?: NeedsYouScope,
): NeedsYouItem[] {
  return tasks.flatMap((t) => {
    const reason = needsYouReason(t, vocab, scope);
    return reason ? [{ id: t.id, name: t.name, reason }] : [];
  });
}
