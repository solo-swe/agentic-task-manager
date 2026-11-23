// A project's stats, progress and forecast, as atm works them out (ATM-722: GET …/projects/<id>/stats, atm-lib's
// `stats`), for the whole project or a slice of it and a period. The dashboard draws them: this module loads them and
// gives them the shapes the charts read.
import type { Progress, Stats, TimeUse as ServedTimeUse } from '$generated/types';
import { browserFetch, statsUrl, type Fetch, type StatsRequest } from './api/http';

/** atm's stats for `request` (the whole project without one) */
export async function loadStats(
  project: string,
  request: StatsRequest = {},
  fetchFn: Fetch = browserFetch,
): Promise<Stats> {
  const resp = await fetchFn(statsUrl(project, request), { cache: 'no-store' });
  if (!resp.ok) throw new Error('HTTP ' + resp.status);
  return (await resp.json()) as Stats;
}

/** A group's completion: done of all but the skipped */
export interface Share {
  key: string;
  label: string;
  done: number;
  total: number;
  skipped: number;
  /** 0–1; 1 when every task was skipped */
  ratio: number;
}

/** How far along the tasks are, as the Progress widget shows it */
export interface ProgressBreakdown {
  overall: Share;
  /** How many of them are archived */
  archived: number;
  byStatus: { status: string; count: number }[];
  byType: Share[];
}

/** No progress yet (before atm has answered) */
export const NO_PROGRESS: ProgressBreakdown = {
  overall: { key: 'all', label: 'All tasks', done: 0, total: 0, skipped: 0, ratio: 0 },
  archived: 0,
  byStatus: [],
  byType: [],
};

/** atm's progress as the widget reads it */
export function progressOf(served: Progress | undefined): ProgressBreakdown {
  if (!served) return NO_PROGRESS;
  const share = (s: Progress['overall']): Share => ({ ...s, key: s.key ?? '' });
  return {
    overall: share(served.overall),
    archived: served.archived,
    byStatus: served.by_status,
    byType: served.by_type.map(share),
  };
}

export interface Interval {
  start: number;
  end: number;
}

export interface TimeSplit {
  workingMs: number;
  waitingMs: number;
  idleMs: number;
}

/** Where the time went (docs/activity-popup-review.md §3.1), atm's, with the same split for any part of it (a chart's
 * bucket): the part of each of atm's stretches inside it */
export interface TimeUse extends TimeSplit {
  from: number;
  to: number;
  agentMs: number;
  maxAgents: number;
  waiting: Interval[];
  idle: Interval[];
  split: (from: number, to: number) => TimeSplit;
}

/** The time `intervals` cover within [from, to] */
export function coveredMs(intervals: Interval[], from: number, to: number): number {
  return intervals.reduce((n, iv) => n + Math.max(0, Math.min(iv.end, to) - Math.max(iv.start, from)), 0);
}

/** atm's time use, with its split for a part of it; null when no work falls in the period */
export function timeUseOf(served: ServedTimeUse | null | undefined): TimeUse | null {
  if (!served) return null;
  const pairs = (list: number[][]): Interval[] => list.map(([start, end]) => ({ start, end }));
  const waiting = pairs(served.waiting);
  const idle = pairs(served.idle);
  const { from, to } = served;
  const split = (a: number, b: number): TimeSplit => {
    const waitingMs = coveredMs(waiting, a, b);
    const idleMs = coveredMs(idle, a, b);
    const spanMs = Math.max(0, Math.min(b, to) - Math.max(a, from));
    return { workingMs: Math.max(0, spanMs - waitingMs - idleMs), waitingMs, idleMs };
  };
  return {
    from,
    to,
    workingMs: served.working_ms,
    waitingMs: served.waiting_ms,
    idleMs: served.idle_ms,
    agentMs: served.agent_ms,
    maxAgents: served.max_agents,
    waiting,
    idle,
    split,
  };
}
