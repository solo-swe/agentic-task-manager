// The Project tab's Stats (docs/project-stats-proposal.md §4, T-253): the project's numbers over its whole history, and
// the time per task, as atm works them out (ATM-722: the stats route's `stats` and `histogram`, atm-lib's
// `stats::time`). No git calls: commits and PRs are what atm recorded on tasks.
import type { Stats } from '$generated/types';

export interface ProjectStats {
  /** The earliest created_at, else meta.created; null when neither is known */
  started: number | null;
  /** From the first task start to the last completion, or to now while tasks are open; null without a start */
  duration: { from: number; to: number; ongoing: boolean } | null;
  /** Archived tasks counted as what they are */
  tasks: { total: number; done: number; open: number; skipped: number };
  /** The project's sprints, and how many are closed (T-323) */
  sprints: { total: number; closed: number };
  /** Tasks with a recorded commit: one per task, the commit atm made when finishing it */
  taskCommits: number;
  /** Distinct pull requests recorded on tasks; null for a project that pushes straight to its branch */
  pullRequests: number | null;
  /** Work time, parallel agents adding up; split into tasks agents did alone and those done with you */
  agentMs: number;
  aloneMs: number;
  withYouMs: number;
  /** The most agents working at the same moment */
  maxAgents: number;
  /** Agent time ÷ time with at least one agent working; null without work */
  parallelism: number | null;
}

/** No numbers yet (before atm has answered) */
export const NO_STATS: ProjectStats = {
  started: null,
  duration: null,
  tasks: { total: 0, done: 0, open: 0, skipped: 0 },
  sprints: { total: 0, closed: 0 },
  taskCommits: 0,
  pullRequests: null,
  agentMs: 0,
  aloneMs: 0,
  withYouMs: 0,
  maxAgents: 0,
  parallelism: null,
};

type Served = Record<string, unknown>;
const num = (v: unknown, or = 0) => (typeof v === 'number' ? v : or);

/** atm's numbers as the Stats panel reads them */
export function numbersOf(stats: Stats | null | undefined): ProjectStats {
  const s = stats?.stats as Served | undefined;
  if (!s) return NO_STATS;
  const duration = s.duration as { from: number; to: number; ongoing: boolean } | null;
  return {
    started: typeof s.started === 'number' ? s.started : null,
    duration: duration ?? null,
    tasks: s.tasks as ProjectStats['tasks'],
    sprints: s.sprints as ProjectStats['sprints'],
    taskCommits: num(s.task_commits),
    pullRequests: typeof s.pull_requests === 'number' ? s.pull_requests : null,
    agentMs: num(s.agent_ms),
    aloneMs: num(s.alone_ms),
    withYouMs: num(s.with_you_ms),
    maxAgents: num(s.max_agents),
    parallelism: typeof s.parallelism === 'number' ? s.parallelism : null,
  };
}

// ---- time per task: a histogram (§4.1, T-254) ----

export interface TimedTask {
  id: string;
  /** Its key, with task keys (lib/keys.ts) */
  key?: string;
  name: string;
  minutes: number;
  /** Done with you (`with: user`): its time includes working with you */
  withYou: boolean;
}

export interface TimeBin {
  /** Lower bound in minutes; the last bin has no upper bound */
  from: number;
  to: number;
  label: string;
  alone: TimedTask[];
  withYou: TimedTask[];
}

export interface TimeHistogram {
  bins: TimeBin[];
  /** Median minutes of each kind, null without tasks of that kind */
  medianAlone: number | null;
  medianWithYou: number | null;
  /** Over all timed tasks; null without any */
  min: number | null;
  max: number | null;
  average: number | null;
  /** The five slowest, slowest first */
  slowest: TimedTask[];
}

const timed = (t: Served): TimedTask => ({
  id: String(t.id),
  ...(typeof t.key === 'string' ? { key: t.key } : {}),
  name: typeof t.name === 'string' ? t.name : '',
  minutes: num(t.minutes),
  withYou: t.with_you === true,
});

/** atm's time per task as the histogram reads it; none before atm has answered */
export function histogramOf(stats: Stats | null | undefined): TimeHistogram | null {
  const h = stats?.histogram as Served | undefined;
  if (!h) return null;
  const list = (v: unknown) => (Array.isArray(v) ? (v as Served[]).map(timed) : []);
  const orNull = (v: unknown) => (typeof v === 'number' ? v : null);
  return {
    bins: (h.bins as Served[]).map((b) => ({
      from: num(b.from),
      // The last bin has no upper bound
      to: typeof b.to === 'number' ? b.to : Infinity,
      label: String(b.label),
      alone: list(b.alone),
      withYou: list(b.with_you),
    })),
    medianAlone: orNull(h.median_alone),
    medianWithYou: orNull(h.median_with_you),
    min: orNull(h.min),
    max: orNull(h.max),
    average: orNull(h.average),
    slowest: list(h.slowest),
  };
}
