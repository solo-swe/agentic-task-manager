// Tasks waiting on someone rather than on agent work (docs/v4/dashboard.md section 3): the words for what atm says.
// Who a task waits on, since when, whether it needs the user and whether agent work is left are atm's to decide
// (`derived.waiting`, `needs_you`, `agent_work`; atm-lib's `waiting`, ATM-717): nothing here works them out.
import { formatMinutes } from './format';
import type { NeedsYouItem as NeedsYouEntry, TaskWaiting } from '$generated/types';
import type { Note, Task } from './types';

/** The newest note of `kind`, if any. */
export function latestNote(task: Task, kind: string): Note | undefined {
  return (task.notes ?? []).filter((n) => n.kind === kind).sort((a, b) => b.at.localeCompare(a.at))[0];
}

const since = (minutes: number) => (minutes < 1 ? 'just now' : formatMinutes(minutes));

/** Minutes from `iso` to `now`; null without a time */
function minutesSince(iso: string | undefined, now: number): number | null {
  const at = Date.parse(iso ?? '');
  return Number.isNaN(at) ? null : Math.max(0, (now - at) / 60_000);
}

/** What a card's waiting line says, and how it is styled */
export interface WaitLine {
  /** "Waiting for review · 2 h", "Back with its agent · 10 m", "Waiting on you · 1 h · 2 questions", "Blocked · 3 d" */
  text: string;
  /** A review's wait (`review-wait`) or a block's (`block-wait`) */
  kind: 'review' | 'block';
  /** It waits on the user */
  onUser: boolean;
  /** The questions, for the line's tooltip */
  questions: string[];
  /** The note left with the review request, for the line's tooltip */
  note?: Note;
}

/** The words for atm's `waiting`, for a task of kind `kind` (atm's too): its line's text */
export function waitText(
  waiting: Pick<TaskWaiting, 'on' | 'since' | 'questions'>,
  kind: string | undefined,
  now: number,
) {
  const label =
    waiting.on === 'reviewer'
      ? 'Waiting for review'
      : waiting.on === 'agent'
        ? kind === 'review'
          ? 'Back with its agent'
          : 'Waiting on an agent'
        : waiting.on === 'user'
          ? 'Waiting on you'
          : 'Blocked';
  const parts = [label];
  const minutes = minutesSince(waiting.since, now);
  if (minutes !== null) parts.push(since(minutes));
  const n = waiting.questions?.length ?? 0;
  if (n) parts.push(`${n} question${n === 1 ? '' : 's'}`);
  return parts.join(' · ');
}

/**
 * The waiting line of a task in review or blocked, as atm says who it waits on (`derived.waiting`); null for any other
 * task (to do and waiting on its dependencies, or with you, has its own line), and for one atm says nothing of.
 */
export function waitLine(task: Task, now: number): WaitLine | null {
  const kind = task.derived?.kind;
  const waiting = task.derived?.waiting;
  if (!waiting || (kind !== 'review' && kind !== 'blocked')) return null;
  const note = kind === 'review' && waiting.on === 'reviewer' ? latestNote(task, 'review') : undefined;
  return {
    text: waitText(waiting, kind, now),
    kind: kind === 'review' ? 'review' : 'block',
    onUser: waiting.on === 'user',
    questions: waiting.questions ?? [],
    ...(note ? { note } : {}),
  };
}

/** Tasks that need the user, as atm says (`derived.needs_you`) */
export function needingUser(tasks: Task[]): Task[] {
  return tasks.filter((t) => t.derived?.needs_you);
}

/** A task that needs the user, as the chips list it: with its key when it has one (ADR-007), as a project's own tasks
 * and a summary's (the overview's cards, T-549) do */
export type NeedsYouItem = NeedsYouEntry & Partial<Pick<Task, 'key'>>;

/** needingUser as the chips list it. A task in review whose code review is one of `reviews` (those waiting on the user,
 * listed in the same chip) is waited on through it, so it isn't listed again, as atm's summary counts it (ATM-640) */
export function needsYouItems(tasks: Task[], reviews: readonly { id: string }[] = []): NeedsYouItem[] {
  const waiting = new Set(reviews.map((r) => r.id));
  return tasks.flatMap((t) => {
    const reason = t.derived?.needs_you;
    if (!reason) return [];
    if (reason === 'review' && (t.code_reviews ?? []).some((l) => waiting.has(l.id))) return [];
    return [t.key ? { id: t.id, key: t.key, name: t.name, reason } : { id: t.id, name: t.name, reason }];
  });
}
