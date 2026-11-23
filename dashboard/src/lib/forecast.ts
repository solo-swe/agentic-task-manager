// Completion estimates (docs/dashboard-v2-requirements.md, section 3), as atm works them out (ATM-722: the stats
// route's `forecast`, atm-lib's `stats::forecast`): the agents' pace from their recent work, each sprint's tasks counted,
// and the work left accumulating in the loop's order (the active sprint, the planned ones, the backlog). The dashboard
// draws them; these are the shapes its charts read.
import type { Forecast as ServedForecast } from '$generated/types';
import { BACKLOG } from './filters';
import type { Sprint, SprintModel } from './sprints';
import type { Task } from './types';

export interface Pace {
  /** Minutes per task */
  likely: number;
  low: number;
  high: number;
  basedOn: number;
}

export interface Estimate {
  /** Working minutes until the sprint is done */
  minutes: { likely: number; low: number; high: number };
  /** Calendar time if the loop keeps running from now */
  finishAt: { likely: number; low: number; high: number };
}

/** How far along a sprint's tasks are (not the sprint's own state: a planned sprint can have started tasks) */
export type ProgressState = 'done' | 'in_progress' | 'not_started';

export interface SprintForecast {
  /** The sprint's ID, or BACKLOG for the tasks in no sprint */
  key: string;
  /** The sprint; undefined for the backlog */
  sprint?: Sprint;
  /** Tasks that count: all but the skipped ones */
  total: number;
  done: number;
  /** Skipped tasks: neither done nor left to do, shown apart */
  skipped: number;
  /** Tasks that still need agent work */
  remaining: number;
  blocked: number;
  /** Unfinished, but waiting for a reviewer rather than for agent work */
  inReview: number;
  state: ProgressState;
  /** Last completion, once all its tasks are finished */
  completedAt?: number;
  /** Absent when it is done or closed, nothing is left to work on, or there is too little history */
  estimate?: Estimate;
}

export interface Forecast {
  pace: Pace | null;
  /** The sprints with tasks: the closed ones in the project's order, then the active one, the planned ones, the backlog */
  sprints: SprintForecast[];
  /** The row the loop works on next (its key), by the whole project's; undefined when nothing is left */
  next?: string;
}

/** No forecast yet (before atm has answered) */
export const NO_FORECAST: Forecast = { pace: null, sprints: [] };

/** The key of the sprint a task is in: its sprint's ID, or BACKLOG */
export const sprintKey = (model: SprintModel, task: Task): string => model.sprintOf(task) ?? BACKLOG;

/** atm's forecast as the charts read it: the backlog's row keyed BACKLOG, each sprint's row with its sprint */
export function fromServed(served: ServedForecast | undefined, model: SprintModel): Forecast {
  if (!served) return NO_FORECAST;
  const key = (sprint: string | null | undefined) => sprint ?? BACKLOG;
  return {
    pace: served.pace
      ? { likely: served.pace.likely, low: served.pace.low, high: served.pace.high, basedOn: served.pace.based_on }
      : null,
    sprints: served.sprints.map((r) => ({
      key: key(r.sprint),
      ...(r.sprint ? { sprint: model.sprints.find((s) => s.id === r.sprint) } : {}),
      total: r.total,
      done: r.done,
      skipped: r.skipped,
      remaining: r.remaining,
      blocked: r.blocked,
      inReview: r.in_review,
      state: r.state,
      ...(r.completed_at !== undefined ? { completedAt: r.completed_at } : {}),
      ...(r.estimate ? { estimate: { minutes: r.estimate.minutes, finishAt: r.estimate.finish_at } } : {}),
    })),
    ...(served.next ? { next: key(served.next.sprint) } : {}),
  };
}
