// Burndown series (FR-4.1): remaining tasks and total scope over time, plus a forecast from now to the estimated
// finish. Skipped-kind tasks are not part of the scope; blocked and in-review tasks stay "remaining" (they are not
// done), and the forecast doesn't cover them (they wait on someone, not on agent work).
import { sprintKey, type Forecast } from './forecast';
import { NO_SPRINTS, type SprintModel } from './sprints';
import { completedAt } from './timeline';
import type { Task } from './types';
import { taskKind } from './vocab';

export interface BurndownPoint {
  t: number;
  scope: number;
  remaining: number;
}

export interface BurndownForecast {
  from: { t: number; remaining: number };
  /** Likely finish; `remaining` there is what the estimate doesn't cover (blocked and in-review tasks) */
  to: { t: number; remaining: number };
  range: { low: number; high: number };
}

export interface Burndown {
  /** Step series, oldest first; the last point is "now" */
  points: BurndownPoint[];
  forecast?: BurndownForecast;
  /** No task has any time: there is no timeline to draw yet */
  untimed?: boolean;
  /** A closed sprint's end: the chart stops there, with no "now" and no forecast */
  end?: number;
  /** The focused sprint's due date (the end of that day, UTC), marked on the chart */
  due?: number;
}

const time = (iso: string | undefined) => {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? undefined : t;
};

/** Earliest timestamp anywhere in the tasks: the fallback creation time for tasks without created_at. */
function earliest(tasks: Task[], now: number): number {
  const all = tasks.flatMap((t) => [t.created_at, t.updated_at, t.started_at, t.completed_at].map(time));
  const known = all.filter((t): t is number => t !== undefined);
  return known.length ? Math.min(...known) : now;
}

/**
 * The burndown's time range (T-240, SR-7.4): the focused sprint's, or else the one the loop works on (from its start;
 * the default: on a long project, all time squeezes the forecast into a sliver), the last 14 days, or all time.
 */
export type BurndownRange = 'sprint' | '14d' | 'all';
export const BURNDOWN_RANGES: BurndownRange[] = ['sprint', '14d', 'all'];
const DAY = 24 * 60 * 60_000;

/** The sprint (its key) the "sprint" range and the chart's markers are about: the focused one, else the loop's next,
 * as atm says it (ATM-722) */
export function rangeSprint(forecast: Forecast, focus?: string): string | undefined {
  return focus ?? forecast.next;
}

/** Where the range starts, or undefined for all time (also when the sprint has no times to start from): the sprint's
 * start, else when its first task started, else when its first task was created */
export function rangeStart(
  range: BurndownRange,
  tasks: Task[],
  forecast: Forecast,
  now: number,
  focus?: string,
  model: SprintModel = NO_SPRINTS,
): number | undefined {
  if (range === 'all') return undefined;
  if (range === '14d') return now - 14 * DAY;
  const key = rangeSprint(forecast, focus);
  const start = time(forecast.sprints.find((s) => s.key === key)?.sprint?.start);
  if (start !== undefined) return start < now ? start : undefined;
  const own = tasks.filter((t) => sprintKey(model, t) === key);
  const starts = own.map((t) => time(t.started_at)).filter((t): t is number => t !== undefined);
  const created = own.map((t) => time(t.created_at)).filter((t): t is number => t !== undefined);
  const from = starts.length ? Math.min(...starts) : created.length ? Math.min(...created) : undefined;
  return from !== undefined && from < now ? from : undefined;
}

export function burndown(
  allTasks: Task[],
  forecast: Forecast,
  now: number,
  focus?: string,
  from?: number,
  model: SprintModel = NO_SPRINTS,
): Burndown {
  const kind = taskKind;
  const tasks = allTasks.filter((t) => kind(t) !== 'skipped' && (focus === undefined || sprintKey(model, t) === focus));
  if (tasks.length === 0) return { points: [] };
  // A focused closed sprint is drawn from its start to its end; an open one gets its due date marked
  const sprint = forecast.sprints.find((s) => s.key === rangeSprint(forecast, focus))?.sprint;
  const closedAt = focus !== undefined && sprint?.state === 'closed' ? time(sprint.end) : undefined;
  const end = closedAt !== undefined && closedAt < now ? closedAt : undefined;
  // When its due date has passed, by atm's one rule (ATM-720: the end of that day, the project's day)
  const due = time(sprint?.due_end);
  const until = end ?? now;
  const untimed = !allTasks.some((t) =>
    [t.created_at, t.started_at, t.completed_at].some((x) => time(x) !== undefined),
  );

  const fallback = earliest(allTasks, now);
  const created = tasks.map((t) => time(t.created_at) ?? fallback);
  const completed = tasks.filter((t) => kind(t) === 'done').map((t) => time(completedAt(t)) ?? fallback);

  // With a range, the chart starts at `from` with the state then, and shows the changes after it
  const times = [...new Set([...created, ...completed])]
    .filter((t) => t <= until && (from === undefined || t > from))
    .sort((a, b) => a - b);
  const at = (t: number): BurndownPoint => {
    const scope = created.filter((c) => c <= t).length;
    return { t, scope, remaining: scope - completed.filter((c) => c <= t).length };
  };
  const points = [...(from === undefined ? [] : [from]), ...times, until].map(at);

  // The estimate that finishes last among the shown sprints: the loop's last
  const estimates = forecast.sprints
    .filter((s) => s.estimate && (focus === undefined || s.key === focus))
    .map((s) => s.estimate!);
  const last = estimates.reduce<(typeof estimates)[number] | undefined>(
    (latest, e) => (!latest || e.finishAt.likely > latest.finishAt.likely ? e : latest),
    undefined,
  );
  const current = points[points.length - 1];
  // What the estimate doesn't cover: open tasks waiting on someone rather than on agent work, as atm says (ATM-717:
  // `derived.agent_work`; an approved or sent-back task is agent work)
  const waiting = tasks.filter((t) => kind(t) !== 'done' && t.derived?.agent_work === false).length;
  return {
    points,
    ...(untimed ? { untimed } : {}),
    ...(end !== undefined ? { end } : {}),
    ...(due !== undefined ? { due } : {}),
    ...(end === undefined && last && current.remaining > waiting
      ? {
          forecast: {
            from: { t: now, remaining: current.remaining },
            to: { t: last.finishAt.likely, remaining: waiting },
            range: { low: last.finishAt.low, high: last.finishAt.high },
          },
        }
      : {}),
  };
}
