// What counts as agent work (docs/v4/data-model.md DM-4.1, dashboard D-2.2, D-3.4) is atm's one rule (ATM-721,
// atm-lib's `work`): every attempt and every round, less its review rounds and blocks, to now while an agent is at it.
// atm serves it on each task (`derived.work`); every chart that measures work (agents, utilisation, activity,
// forecast, the time per task) reads it from here.
import { formatMinutes } from './format';
import type { Task } from './types';
import { taskKind } from './vocab';

export interface WorkSpan {
  start: number;
  end: number;
  /** An agent is still working on it (active kind): the span runs to now */
  live: boolean;
}

export const ms = (iso: string | undefined): number | null => {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? null : t;
};

/** Who started (and so worked on) the task: started_by, or `agent` in files written before v4 (DM-4.4). */
export function startedBy(task: Task): string | undefined {
  return task.started_by ?? task.agent;
}

/** Work done together with the user (ATM-721, decision 1): `with: user` alone says it (atm db migrate moved the old
 * `interactive` label to it). Its time includes waiting on them, so the agents' pace leaves it out */
export function withYou(task: Task): boolean {
  return task.with === 'user';
}

export function isDone(task: Task): boolean {
  return taskKind(task) === 'done';
}

/** One line of the details panel's timeline: a moment (`at`, ISO 8601) or a duration (`text`) */
export interface TimeRow {
  label: string;
  at?: string;
  text?: string;
}

/**
 * The task's timeline for the details panel, in order: Created, Started, Sent for review, Finished (or Skipped),
 * Duration (atm's work time, "so far" while an agent works on it) and Updated. Only what the task has.
 */
export function taskTimes(task: Task, now: number): TimeRow[] {
  const kind = taskKind(task);
  const rows: TimeRow[] = [];
  const at = (label: string, iso: string | undefined) => {
    if (ms(iso) !== null) rows.push({ label, at: iso });
  };
  at('Created', task.created_at);
  at('Started', task.started_at);
  at('Sent for review', task.review_requested_at);
  if (kind === 'done') at('Finished', task.completed_at);
  if (kind === 'skipped') at('Skipped', task.skipped_at);
  const minutes = workMinutes(task, now);
  if (minutes !== null)
    rows.push({ label: 'Duration', text: formatMinutes(minutes) + (task.derived?.work?.live ? ' so far' : '') });
  at('Updated', task.updated_at);
  return rows;
}

/** A stretch of the task's work, as atm says it (`derived.work.spans`), and who did it */
export interface WorkPiece {
  start: number;
  end: number;
  by?: string;
}

/**
 * The task's work as atm counts it (ATM-721, `derived.work`: every attempt and every round, less its reviews and
 * blocks), its stretches in time order; none when it holds none. While an agent is at it, the last one runs to `now`
 * (atm said it when it served the task).
 */
export function workSpans(task: Task, now: number): WorkPiece[] {
  const work = task.derived?.work;
  if (!work) return [];
  const pieces = work.spans.flatMap((s) => {
    const [start, end] = [ms(s.from), ms(s.to)];
    return start === null || end === null ? [] : [{ start, end, ...(s.by ? { by: s.by } : {}) }];
  });
  // A live stretch runs to the page's now, not to when atm served it
  const last = pieces.at(-1);
  if (work.live && last) last.end = Math.max(last.start, now);
  return pieces;
}

/** The task's work in minutes, as atm counts it (to `now` while an agent is at it); null when it holds none */
export function workMinutes(task: Task, now: number): number | null {
  const work = task.derived?.work;
  if (!work) return null;
  const pieces = workSpans(task, now);
  return pieces.length ? pieces.reduce((n, p) => n + (p.end - p.start), 0) / 60_000 : work.minutes;
}

/**
 * The task's work as one stretch, from its first piece to its last (a card's "took", a timeline's bar), or null when it
 * holds none. `live` while an agent is at it.
 */
export function workSpan(task: Task, now: number): WorkSpan | null {
  const pieces = workSpans(task, now);
  if (!pieces.length) return null;
  return { start: pieces[0].start, end: pieces.at(-1)!.end, live: !!task.derived?.work?.live };
}
