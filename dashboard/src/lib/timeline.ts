import { keyOf } from './keys';
import type { Task } from './types';
import { taskKind } from './vocab';

export interface Completion {
  id: string;
  /** Its key, with task keys (lib/keys.ts) */
  key?: string;
  name: string;
  time: number;
}

/** When a done task was finished: completed_at, or updated_at for files written before completed_at existed. */
export function completedAt(task: Task): string | undefined {
  return task.completed_at ?? task.updated_at;
}

/** Done-kind tasks with a valid completion time, oldest first. */
export function completions(tasks: Task[]): Completion[] {
  return tasks
    .filter((t) => taskKind(t) === 'done' && completedAt(t))
    .map((t) => ({ id: t.id, ...keyOf(t), name: t.name, time: new Date(completedAt(t)!).getTime() }))
    .filter((c) => !isNaN(c.time))
    .sort((a, b) => a.time - b.time);
}

/** "Apr 3" in the viewer's locale */
export function formatDate(ts: number): string {
  return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/** "Apr 3 11:00 AM" in the viewer's locale */
export function formatDateTime(ts: number): string {
  const d = new Date(ts);
  return (
    d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) +
    ' ' +
    d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
  );
}
