// Who did what, when: one lane per agent with a bar per task, for the task's work (work.ts: from start to completion
// or review request, or to now while active). The agent comes from the task (written by `atm start`) or, for work in
// progress, from its live claim.
import type { ClaimMap } from './claims';
import { median } from './insights';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, type Vocabulary } from './vocab';
import { startedBy, workSpan } from './work';

export const UNATTRIBUTED = '';

export interface AgentBar {
  task: Task;
  start: number;
  end: number;
  /** Still in progress: the bar runs to now */
  live: boolean;
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
  vocab: Vocabulary = DEFAULT_VOCABULARY,
): AgentTimeline | null {
  const byAgent = new Map<string, AgentBar[]>();
  for (const task of tasks) {
    const span = workSpan(task, now, vocab);
    if (!span) continue;
    const agent = (span.live ? claims.get(task.id)?.agent : undefined) ?? startedBy(task) ?? UNATTRIBUTED;
    byAgent.set(agent, [...(byAgent.get(agent) ?? []), { task, ...span }]);
  }
  if (byAgent.size === 0) return null;

  const lanes: AgentLane[] = [...byAgent].map(([agent, all]) => {
    const bars = matching ? all.filter((b) => matching.has(b.task.id)) : all;
    // Work handed over for review is finished work, but the task isn't done yet
    const finished = bars.filter((b) => !b.live);
    return {
      agent,
      bars: all.sort((a, b) => a.start - b.start),
      matches: bars.length > 0,
      done: finished.filter((b) => kindOf(vocab, b.task.status) === 'done').length,
      medianMinutes: median(finished.map((b) => (b.end - b.start) / 60_000)),
      current: bars.filter((b) => b.live).map((b) => b.task),
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
