// Where the time went (docs/activity-popup-review.md §3.1, §5): from the first work on, time is split three ways,
// shared by the Activity popup and the Progress tab.
// - agents working: at least one agent at work (work.ts), and the short pauses between tasks;
// - waiting on you: no agent working while a task is in review or blocked on the user;
// - idle: the rest, in stretches of at least MIN_IDLE_MS.
// Agent time adds up parallel agents: each agent's own overlapping spans merged, then summed.
import { MIN_IDLE_MS, gaps } from './insights';
import type { Task } from './types';
import { taskKind } from './vocab';
import { ms, startedBy, workSpan } from './work';

export interface Interval {
  start: number;
  end: number;
}

export interface TimeSplit {
  workingMs: number;
  waitingMs: number;
  idleMs: number;
}

export interface TimeUse extends TimeSplit {
  /** The span measured: from the first work (or the period's start, if later) to the period's end */
  from: number;
  to: number;
  /** Work time with parallel agents added up */
  agentMs: number;
  /** The most agents working at the same time */
  maxAgents: number;
  /** Waiting on you, with no agent working (sorted, disjoint) */
  waiting: Interval[];
  /** Idle stretches (sorted, disjoint) */
  idle: Interval[];
}

/** A period's time use, with the same split for any part of it (a chart's bucket) */
export type TimeUseWithSplit = TimeUse & { split: (from: number, to: number) => TimeSplit };

/** The union of `intervals`: sorted, disjoint, touching ones joined. */
export function mergeIntervals(intervals: Interval[]): Interval[] {
  const out: Interval[] = [];
  for (const iv of [...intervals].sort((a, b) => a.start - b.start)) {
    const last = out.at(-1);
    if (last && iv.start <= last.end) last.end = Math.max(last.end, iv.end);
    else if (iv.end > iv.start) out.push({ start: iv.start, end: iv.end });
  }
  return out;
}

/** `a` minus `b`, both sorted and disjoint. */
function subtract(a: Interval[], b: Interval[]): Interval[] {
  const out: Interval[] = [];
  for (const iv of a) {
    let cursor = iv.start;
    for (const cut of b) {
      if (cut.end <= cursor || cut.start >= iv.end) continue;
      if (cut.start > cursor) out.push({ start: cursor, end: cut.start });
      cursor = Math.max(cursor, cut.end);
    }
    if (cursor < iv.end) out.push({ start: cursor, end: iv.end });
  }
  return out;
}

const clip = <T extends Interval>(intervals: T[], from: number, to: number): T[] =>
  intervals
    .map((iv) => ({ ...iv, start: Math.max(iv.start, from), end: Math.min(iv.end, to) }))
    .filter((iv) => iv.end > iv.start);

/** The time `intervals` cover within [from, to]. */
export function coveredMs(intervals: Interval[], from: number, to: number): number {
  return clip(intervals, from, to).reduce((n, iv) => n + iv.end - iv.start, 0);
}

/**
 * When the task waited on you: each review (the `reviews` atm keeps; the current one to now; for older files, a done
 * task's `review_requested_at` to its completion) and each block on the user (`blocks`; the current one to now).
 */
export function waitingIntervals(task: Task, now: number): Interval[] {
  const kind = taskKind(task);
  const out: Interval[] = [];
  const add = (from: string | undefined, to: number | null) => {
    const start = ms(from);
    if (start !== null && to !== null && to > start) out.push({ start, end: to });
  };
  for (const r of task.reviews ?? []) add(r.from, ms(r.to));
  for (const b of task.blocks ?? []) if (b.on === 'user') add(b.from, ms(b.to));
  const recorded = (from: string | undefined) => (task.reviews ?? []).some((r) => r.from === from);
  // Waiting on you now only while atm says it waits on its reviewer or you (ATM-717): approved or sent back, it is
  // back with its agent
  const onYou = task.derived?.waiting?.on === 'reviewer' || task.derived?.waiting?.on === 'user';
  if (kind === 'review' && onYou) add(task.review_requested_at, now);
  else if (kind === 'done' && !recorded(task.review_requested_at)) add(task.review_requested_at, ms(task.completed_at));
  if (kind === 'blocked' && task.waiting_on === 'user') add(task.blocked_at, now);
  return out;
}

/**
 * Where the time went in `period` (null: all time, up to now, like the Activity charts' idle hatching), or null when
 * no work falls in it.
 * `split` gives the same three-way split for part of it (a chart's bucket).
 */
export function timeUse(tasks: Task[], period: Interval | null, now: number): TimeUseWithSplit | null {
  const work = tasks.flatMap((t) => {
    const span = workSpan(t, now);
    return span && span.end > span.start ? [{ agent: startedBy(t) ?? '', start: span.start, end: span.end }] : [];
  });
  const waitingAll = mergeIntervals(tasks.flatMap((t) => waitingIntervals(t, now)));
  const firstWork = Math.min(...work.map((w) => w.start));
  const from = Math.max(period?.start ?? -Infinity, firstWork);
  const to = period ? Math.min(period.end, now) : now;
  if (!work.length || to <= from) return null;

  const working = mergeIntervals(clip(work, from, to));
  if (!working.length) return null;
  const waiting = subtract(mergeIntervals(clip(waitingAll, from, to)), working);
  const idle = gaps(mergeIntervals([...working, ...waiting]), from, to, MIN_IDLE_MS);

  // Agent time and parallelism: each agent's spans merged first (an agent on two tasks counts once); spans without
  // an agent are each their own
  const perAgent = new Map<string, Interval[]>();
  let n = 0;
  for (const w of clip(work, from, to)) {
    const key = w.agent || `\u0000${n++}`;
    perAgent.set(key, [...(perAgent.get(key) ?? []), w]);
  }
  const agentSpans = [...perAgent.values()].flatMap(mergeIntervals);
  const edges = agentSpans.flatMap((s) => [
    [s.start, 1],
    [s.end, -1],
  ]);
  edges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let current = 0;
  let maxAgents = 0;
  for (const [, d] of edges) maxAgents = Math.max(maxAgents, (current += d));

  const split = (a: number, b: number): TimeSplit => {
    const waitingMs = coveredMs(waiting, a, b);
    const idleMs = coveredMs(idle, a, b);
    const spanMs = Math.max(0, Math.min(b, to) - Math.max(a, from));
    return { workingMs: Math.max(0, spanMs - waitingMs - idleMs), waitingMs, idleMs };
  };
  return {
    from,
    to,
    ...split(from, to),
    agentMs: agentSpans.reduce((sum, s) => sum + s.end - s.start, 0),
    maxAgents,
    waiting,
    idle,
    split,
  };
}
