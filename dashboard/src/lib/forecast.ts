// Completion estimates (docs/dashboard-v2-requirements.md, section 3). Agent work is bursty, so the model measures
// working time per task instead of calendar throughput, and accumulates remaining work in the loop's order: the
// active sprint, then the planned ones, then the backlog (docs/sprints-and-releases-design.md SR-7.4).
import { BACKLOG } from './filters';
import { NO_SPRINTS, type Sprint, type SprintModel } from './sprints';
import { completedAt } from './timeline';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, type StatusKind, type Vocabulary } from './vocab';

/** A gap between completions longer than this is idle time, not work. */
export const GAP_CAP_MINUTES = 30;
/** Pace is based on this many most recent completions. */
export const HISTORY = 10;
/** Below this many timed completions there is no estimate. */
export const MIN_HISTORY = 3;
/** The label of tasks done together with the user: not part of the agents' pace */
export const INTERACTIVE_LABEL = 'interactive';

const MINUTE = 60_000;

export interface TimedCompletion {
  id: string;
  completedAt: number;
  minutes: number;
}

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
  /** Calendar time if the loop keeps running from `now` */
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
  /** Tasks that still need agent work (todo, active, unknown statuses); not blocked, in review or skipped */
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
}

/** The key of the sprint a task is in: its sprint's ID, or BACKLOG */
export const sprintKey = (model: SprintModel, task: Task): string => model.sprintOf(task) ?? BACKLOG;

/** Where the loop takes a sprint's work: the active sprint first, then the planned ones, then the backlog; a closed
 * sprint is not worked on */
const loopRank = (row: SprintForecast) =>
  !row.sprint ? 2 : row.sprint.state === 'active' ? 0 : row.sprint.state === 'planned' ? 1 : undefined;

const time = (iso: string | undefined) => {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? undefined : t;
};

/**
 * Duration of each completed task, oldest first: its work, started_at until it asked for a review or was completed,
 * when known; otherwise the gap since the previous completion, capped. The first completion without a start has
 * nothing to measure from and is left out.
 */
export function taskDurations(tasks: Task[], vocab: Vocabulary = DEFAULT_VOCABULARY): TimedCompletion[] {
  const finished = tasks
    .filter((t) => kindOf(vocab, t.status) === 'done')
    .map((t) => ({ task: t, end: time(completedAt(t)) }))
    .filter((x): x is { task: Task; end: number } => x.end !== undefined)
    .sort((a, b) => a.end - b.end);

  const result: TimedCompletion[] = [];
  finished.forEach(({ task, end }, i) => {
    const start = time(task.started_at);
    const workEnd = time(task.review_requested_at) ?? end;
    let minutes: number | undefined;
    if (start !== undefined && start <= workEnd) minutes = (workEnd - start) / MINUTE;
    else if (i > 0) minutes = Math.min((end - finished[i - 1].end) / MINUTE, GAP_CAP_MINUTES);
    if (minutes !== undefined) result.push({ id: task.id, completedAt: end, minutes });
  });
  return result;
}

/** Linear interpolation between closest ranks (p in 0..1) over a sorted array. */
export function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return NaN;
  const rank = p * (sorted.length - 1);
  const lo = Math.floor(rank);
  const hi = Math.ceil(rank);
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo);
}

export function pace(durations: TimedCompletion[]): Pace | null {
  const recent = durations.slice(-HISTORY).map((d) => d.minutes);
  if (recent.length < MIN_HISTORY) return null;
  const sorted = [...recent].sort((a, b) => a - b);
  return {
    likely: percentile(sorted, 0.5),
    low: percentile(sorted, 0.25),
    high: percentile(sorted, 0.75),
    basedOn: recent.length,
  };
}

/** Kinds that need no more agent work: finished, or waiting on someone else */
const NO_AGENT_WORK = new Set<StatusKind | undefined>(['done', 'skipped', 'blocked', 'review']);

/**
 * Per-sprint progress and estimates, the work accumulating in the loop's order (the active sprint, the planned ones
 * in order, the backlog), like /atm-loop-tasks --all. `history` is what the pace is measured on (all tasks), so
 * filtering the view doesn't change the pace.
 */
export function forecastSprints(
  tasks: Task[],
  now: number,
  history: Task[] = tasks,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): Forecast {
  // Tasks done together with the user (labelled interactive) include the time spent waiting on them: they would slow
  // the pace agents keep when they work alone (docs/progress-tab-review.md §1.4, T-240)
  const interactive = new Set(history.filter((t) => t.labels?.includes(INTERACTIVE_LABEL)).map((t) => t.id));
  const p = pace(taskDurations(history, vocab).filter((d) => !interactive.has(d.id)));
  const kind = (t: Task) => kindOf(vocab, t.status);
  const groups = new Map<string, Task[]>();
  for (const t of tasks) {
    const key = sprintKey(model, t);
    groups.set(key, [...(groups.get(key) ?? []), t]);
  }
  // The closed sprints in the project's order, then the loop's: the active one, the planned ones, the backlog (T-323:
  // a sprint started after others were planned is still first)
  const ofState = (state: string) => model.sprints.filter((s) => s.state === state).map((s) => s.id);
  const keys = [...ofState('closed'), ...ofState('active'), ...ofState('planned'), BACKLOG].filter((key) =>
    groups.has(key),
  );

  const rows = keys.map((key): SprintForecast => {
    const own = groups.get(key)!;
    const done = own.filter((t) => kind(t) === 'done');
    const skipped = own.filter((t) => kind(t) === 'skipped').length;
    const finished = own.every((t) => kind(t) === 'done' || kind(t) === 'skipped');
    // Started once something moved: done, active, in review or an unknown status. Blocked work hasn't begun.
    const started = own.some((t) => !['todo', 'skipped', 'blocked'].includes(kind(t) ?? 'unknown'));
    const row: SprintForecast = {
      key,
      sprint: model.sprints.find((s) => s.id === key),
      total: own.length - skipped,
      done: done.length,
      skipped,
      remaining: own.filter((t) => !NO_AGENT_WORK.has(kind(t))).length,
      blocked: own.filter((t) => kind(t) === 'blocked').length,
      inReview: own.filter((t) => kind(t) === 'review').length,
      state: finished ? 'done' : started ? 'in_progress' : 'not_started',
    };
    if (finished) {
      const ends = done.map((t) => time(completedAt(t))).filter((t): t is number => t !== undefined);
      if (ends.length) row.completedAt = Math.max(...ends);
    }
    return row;
  });

  if (p) {
    let cumulative = 0;
    const order = rows
      .filter((row) => loopRank(row) !== undefined && row.state !== 'done')
      .sort((a, b) => loopRank(a)! - loopRank(b)!);
    for (const row of order) {
      cumulative += row.remaining;
      if (row.remaining === 0) continue;
      const minutes = { likely: cumulative * p.likely, low: cumulative * p.low, high: cumulative * p.high };
      row.estimate = {
        minutes,
        finishAt: {
          likely: now + minutes.likely * MINUTE,
          low: now + minutes.low * MINUTE,
          high: now + minutes.high * MINUTE,
        },
      };
    }
  }
  return { pace: p, sprints: rows };
}

/** The row the loop works on first: the first unfinished one in the loop's order */
export function nextInLoop(forecast: Forecast): SprintForecast | undefined {
  return forecast.sprints
    .filter((row) => loopRank(row) !== undefined && row.state !== 'done')
    .sort((a, b) => loopRank(a)! - loopRank(b)!)[0];
}

/**
 * A filtered slice's sprints with the whole project's estimates (T-239): the loop works through whole sprints, so when
 * a sprint finishes doesn't depend on the filter. The slice keeps its own counts and state; a sprint the slice has
 * finished keeps no estimate.
 */
export function withWholeSprintEstimates(slice: Forecast, whole: Forecast): Forecast {
  const estimates = new Map(whole.sprints.map((s) => [s.key, s.estimate]));
  return {
    pace: whole.pace,
    sprints: slice.sprints.map((s) => {
      const estimate = s.state === 'done' ? undefined : estimates.get(s.key);
      const rest = { ...s };
      delete rest.estimate;
      return estimate ? { ...rest, estimate } : rest;
    }),
  };
}
