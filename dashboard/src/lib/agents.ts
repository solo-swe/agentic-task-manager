// Who did what, when: one lane per agent with a bar per stretch of a task's work (atm's, work.ts: every attempt and
// round, to now while active). The agent is who did the stretch (atm says it) or, for work in progress, its live claim.
import type { ClaimMap } from './claims';
import { median } from './insights';
import type { Task } from './types';
import { taskKind } from './vocab';
import { startedBy, workSpans } from './work';

export const UNATTRIBUTED = '';

export interface AgentBar {
  task: Task;
  start: number;
  end: number;
  /** Still in progress: the bar runs to now */
  live: boolean;
  /** An earlier stretch of the task's work (an attempt taken back, a round before a review), not its last */
  earlier?: boolean;
  /** On the overview's Agents tab (T-333): the task's project, and a key unique across projects */
  project?: { id: string; name: string };
  key?: string;
}

export interface AgentLane {
  /** The agent's identity, or UNATTRIBUTED for tasks from before agents were recorded */
  agent: string;
  bars: AgentBar[];
  /** Whether any of its tasks matches the search and filters */
  matches: boolean;
  done: number;
  medianMinutes: number | null;
  /** Tasks the agent is working on now */
  current: Task[];
  /** When the agent last started or finished something */
  lastActive: number;
  /** On the overview's Agents tab (T-333): the names of the projects it worked in */
  projects?: string[];
}

export interface AgentTimeline {
  start: number;
  end: number;
  lanes: AgentLane[];
}

/**
 * The timeline of all `tasks`. With `matching`, the tasks outside it stay on the timeline (to be greyed out) but the
 * lane stats count only matching tasks.
 */
export function agentTimeline(
  tasks: Task[],
  claims: ClaimMap,
  now: number,
  matching?: ReadonlySet<string>,
): AgentTimeline | null {
  const byAgent = new Map<string, AgentBar[]>();
  for (const task of tasks) {
    // A bar per stretch of its work, as atm counts it (ATM-721): each attempt and round, in the lane of who did it
    const pieces = workSpans(task, now);
    pieces.forEach((piece, i) => {
      const last = i === pieces.length - 1;
      const live = !!task.derived?.work?.live && last;
      const agent = (live ? claims.get(task.id)?.agent : undefined) ?? piece.by ?? startedBy(task) ?? UNATTRIBUTED;
      const bar = { task, start: piece.start, end: piece.end, live, ...(last ? {} : { earlier: true }) };
      byAgent.set(agent, [...(byAgent.get(agent) ?? []), bar]);
    });
  }
  if (byAgent.size === 0) return null;

  const lanes: AgentLane[] = [...byAgent].map(([agent, all]) => {
    const bars = matching ? all.filter((b) => matching.has(b.task.id)) : all;
    // Work handed over for review is finished work, but the task isn't done yet. A task's stretches in a lane count as
    // one task, their minutes together
    const finished = new Map<Task, number>();
    for (const b of bars)
      if (!bars.some((o) => o.task === b.task && o.live))
        finished.set(b.task, (finished.get(b.task) ?? 0) + (b.end - b.start) / 60_000);
    return {
      agent,
      bars: all.sort((a, b) => a.start - b.start),
      matches: bars.length > 0,
      // Done in the lane that worked its last stretch, not in one whose attempt it took over
      done: [...finished.keys()].filter((t) => taskKind(t) === 'done' && bars.some((b) => b.task === t && !b.earlier))
        .length,
      medianMinutes: median([...finished.values()]),
      current: [...new Set(bars.filter((b) => b.live).map((b) => b.task))],
      lastActive: Math.max(...all.map((b) => b.end)),
    };
  });
  // Most recently active first; tasks without an agent last
  lanes.sort(
    (a, b) => Number(a.agent === UNATTRIBUTED) - Number(b.agent === UNATTRIBUTED) || b.lastActive - a.lastActive,
  );
  const all = lanes.flatMap((l) => l.bars);
  return { start: Math.min(...all.map((b) => b.start)), end: Math.max(...all.map((b) => b.end)), lanes };
}

/** About `count` evenly spaced tick times on round minutes, hours or days across [start, end]. */
export function timeTicks(start: number, end: number, count = 6): number[] {
  const span = Math.max(1, end - start);
  const steps = [1, 2, 5, 10, 15, 30, 60, 120, 180, 360, 720, 1440, 2880, 10080].map((m) => m * 60_000);
  const step = steps.find((s) => span / s <= count) ?? steps[steps.length - 1];
  const offset = new Date(start).getTimezoneOffset() * 60_000;
  const first = Math.ceil((start - offset) / step) * step + offset;
  const ticks = [];
  for (let t = first; t <= end; t += step) ticks.push(t);
  return ticks;
}
