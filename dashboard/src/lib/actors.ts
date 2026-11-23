// Who created, started and completed a task (docs/v4/data-model.md §4.1, dashboard D-4.5). An actor whose name starts
// with an agent harness's prefix is an agent; anyone else is a person.
import { taskNotes } from './notes';
import type { Task } from './types';
import { startedBy } from './work';

/** The identities atm gives agents, one prefix per harness (ATM-660): atm-common's `AGENT_PREFIXES` */
export const AGENT_PREFIXES = ['claude:', 'codex:', 'gemini:', 'opencode:', 'agent:'] as const;

export function isAgent(actor: string): boolean {
  return AGENT_PREFIXES.some((prefix) => actor.startsWith(prefix));
}

/** Who completed the task when that isn't who started it: a reviewer who approved it, or a person who finished it. */
export function approvedBy(task: Task): string | undefined {
  const started = startedBy(task);
  const completed = task.completed_by;
  return completed && started && completed !== started ? completed : undefined;
}

/** "Created by X · started by Y · completed by Z", with what the task records; empty when it records none. */
export function actorsLine(task: Task): string {
  const parts: string[] = [];
  if (task.created_by) parts.push(`created by ${task.created_by}`);
  const started = startedBy(task);
  if (started) parts.push(`started by ${started}`);
  if (task.completed_by) parts.push(`completed by ${task.completed_by}`);
  const line = parts.join(' · ');
  return line.charAt(0).toUpperCase() + line.slice(1);
}

/**
 * Who worked on the task before the current attempt: the authors of its handoff and failure notes, oldest first,
 * once each, leaving out whoever holds it now.
 */
export function earlierAttempts(task: Task): string[] {
  const current = startedBy(task);
  const authors = taskNotes(task)
    .filter((n) => (n.kind === 'handoff' || n.kind === 'failure') && n.by && n.by !== current)
    .reverse()
    .map((n) => n.by as string);
  return [...new Set(authors)];
}
