// How a finished task ended and what it is linked to without blocking (docs/sprints-and-releases-design.md SR-2.8),
// as atm says them (ATM-719): its resolution with the default filled in, and the links pointing at it
// (`derived.resolution`, `related_by`, `duplicated_by`), as `atm task details` prints them.
import type { Task } from './types';

export const RESOLUTION_LABELS: Record<string, string> = {
  done: 'done',
  wont_do: "won't do",
  duplicate: 'duplicate',
  obsolete: 'obsolete',
};

/** How a finished task ended, as atm says it: its own resolution, else the default (done, or won't do for a skipped
 * task); undefined while it isn't finished */
export function resolutionOf(task: Task): string | undefined {
  return task.derived?.resolution;
}

export interface Relations {
  /** Its own related tasks, then those that name it */
  related: string[];
  duplicateOf?: string;
  duplicatedBy: string[];
}

/** A task's links that don't block, both ways: its own, and those atm finds pointing at it */
export function relationsOf(task: Task): Relations {
  const own = (task.related ?? []).filter((id) => typeof id === 'string' && id !== task.id);
  return {
    related: [...own, ...(task.derived?.related_by ?? [])],
    duplicateOf: task.duplicate_of || undefined,
    duplicatedBy: task.derived?.duplicated_by ?? [],
  };
}

const NOT_DONE_ORDER = ['wont_do', 'obsolete', 'duplicate'];

/** The finished tasks that weren't done, by resolution (won't do, obsolete, duplicate), for the Progress panel,
 * which counts them apart from done; resolutions it doesn't know come last */
export function notDone(tasks: Task[]): { resolution: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const t of tasks) {
    const resolution = resolutionOf(t);
    if (resolution && resolution !== 'done') counts.set(resolution, (counts.get(resolution) ?? 0) + 1);
  }
  const rank = (r: string) => (NOT_DONE_ORDER.includes(r) ? NOT_DONE_ORDER.indexOf(r) : NOT_DONE_ORDER.length);
  return [...counts]
    .map(([resolution, count]) => ({ resolution, count }))
    .sort((a, b) => rank(a.resolution) - rank(b.resolution) || a.resolution.localeCompare(b.resolution));
}
