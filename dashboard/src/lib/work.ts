// What counts as agent work (docs/v4/data-model.md DM-4.1, dashboard D-2.2, D-3.4): a task's work runs from
// started_at until it asked for a review or was completed, or until now while an agent works on it. Time in review or
// blocked is not work. Every chart that measures work (agents, utilisation, activity, forecast) uses this.
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
 * Duration (the work time workSpan counts, "so far" while an agent works on it) and Updated. Only what the task has.
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
  const span = workSpan(task, now);
  if (span)
    rows.push({
      label: 'Duration',
      text: formatMinutes((span.end - span.start) / 60_000) + (span.live ? ' so far' : ''),
    });
  at('Updated', task.updated_at);
  return rows;
}

/**
 * The task's work, or null if it holds none (not started, or todo/blocked/skipped again). A finished task without a
 * known end gets a zero-length span at its start.
 */
export function workSpan(task: Task, now: number): WorkSpan | null {
  const start = ms(task.started_at);
  if (start === null) return null;
  const kind = taskKind(task);
  if (kind === 'active') return { start, end: Math.max(now, start), live: true };
  if (kind !== 'done' && kind !== 'review') return null;
  const end = ms(task.review_requested_at) ?? ms(kind === 'done' ? task.completed_at : task.updated_at) ?? start;
  return { start, end: Math.max(end, start), live: false };
}
