// Velocity (T-323, SR-7.4): how many tasks each closed sprint got done, the last few in order, from each sprint's
// report (what /atm-close-sprint recorded) or, without one, its tasks done.
import type { SprintModel } from './sprints';
import type { Task } from './types';
import { kindOf, type Vocabulary } from './vocab';

export interface SprintVelocity {
  id: string;
  done: number;
  /** Carried over to the next sprint (or the backlog) when it closed */
  carried: number;
  /** Pulled in after it started */
  added: number;
  /** When it closed, ISO 8601, if known */
  end?: string;
}

export const VELOCITY_SPRINTS = 10;

const count = (list: unknown) => (Array.isArray(list) ? list.length : 0);

/** The last `limit` closed sprints, oldest first, and their average done */
export function velocity(
  model: SprintModel,
  tasks: Task[],
  vocab: Vocabulary,
  limit = VELOCITY_SPRINTS,
): { sprints: SprintVelocity[]; average: number } {
  const sprints = model.sprints
    .filter((s) => s.state === 'closed')
    .slice(-limit)
    .map((s) => ({
      id: s.id,
      done: s.report?.done
        ? count(s.report.done)
        : tasks.filter((t) => model.sprintOf(t) === s.id && kindOf(vocab, t.status) === 'done').length,
      carried: count(s.report?.carried),
      added: count(s.report?.added),
      ...(s.end ? { end: s.end } : {}),
    }));
  const average = sprints.length ? sprints.reduce((n, s) => n + s.done, 0) / sprints.length : 0;
  return { sprints, average };
}
