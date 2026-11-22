// The Project tab's Stats (docs/project-stats-proposal.md §4, T-253): the project's numbers over its whole history.
// Work time as everywhere else (work.ts, timeuse.ts); no git calls: commits and PRs are what atm recorded on tasks.
import { keyOf } from './keys';
import { median } from './insights';
import { completions } from './timeline';
import { NO_SPRINTS, type SprintModel } from './sprints';
import { mergeIntervals, timeUse } from './timeuse';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, taskKind, type Vocabulary } from './vocab';
import { ms, startedBy, withYou, workMinutes, workSpans } from './work';
import { isAgent } from './actors';

export interface ProjectStats {
  /** The earliest created_at, else meta.created; null when neither is known */
  started: number | null;
  /** From the first task start to the last completion, or to now while tasks are open; null without a start */
  duration: { from: number; to: number; ongoing: boolean } | null;
  /** Archived tasks count as done: they are finished, and only finished tasks are archived */
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

export function projectStats(
  tasks: Task[],
  meta: { created?: unknown } | undefined,
  now: number,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  /** Archived tasks not in `tasks` (the archive isn't loaded yet): meta.archived */
  unloadedArchived = 0,
  model: SprintModel = NO_SPRINTS,
): ProjectStats {
  const kind = taskKind;
  const min = (values: (number | null)[]) => {
    const known = values.filter((v): v is number => v !== null);
    return known.length ? Math.min(...known) : null;
  };

  const created = min(tasks.map((t) => ms(t.created_at)));
  const metaCreated = typeof meta?.created === 'string' ? ms(meta.created) : null;
  const firstStart = min(tasks.map((t) => ms(t.started_at)));
  const open = tasks.filter((t) => kind(t) !== 'done' && kind(t) !== 'skipped').length;
  const lastDone = completions(tasks).at(-1)?.time ?? null;
  const duration =
    firstStart === null
      ? null
      : open > 0
        ? { from: firstStart, to: now, ongoing: true }
        : { from: firstStart, to: Math.max(firstStart, lastDone ?? firstStart), ongoing: false };

  const prs = new Set(tasks.map((t) => t.pr?.number).filter((n): n is number => typeof n === 'number'));
  const workflow = vocab.workflow?.type;

  // Agent work: tasks an agent started, or nobody recorded (before v4), as in the Information's Agents line
  const agentTasks = tasks.filter((t) => {
    const starter = startedBy(t);
    return !starter || isAgent(starter);
  });
  const use = timeUse(agentTasks, null, now);
  const together = agentTasks.filter(withYou);
  const alone = agentTasks.filter((t) => !withYou(t));
  // Time with at least one agent at work: the union of the work's stretches, atm's (short pauses not included)
  const working = mergeIntervals(agentTasks.flatMap((t) => workSpans(t, now))).reduce(
    (sum, iv) => sum + iv.end - iv.start,
    0,
  );

  return {
    started: created ?? metaCreated,
    duration,
    tasks: {
      total: tasks.length + unloadedArchived,
      done: tasks.filter((t) => kind(t) === 'done').length + unloadedArchived,
      open,
      skipped: tasks.filter((t) => kind(t) === 'skipped').length,
    },
    sprints: { total: model.sprints.length, closed: model.sprints.filter((s) => s.state === 'closed').length },
    taskCommits: tasks.filter((t) => typeof t.commit === 'string' && t.commit).length,
    pullRequests: prs.size === 0 && workflow === 'direct' ? null : prs.size,
    agentMs: use?.agentMs ?? 0,
    aloneMs: timeUse(alone, null, now)?.agentMs ?? 0,
    withYouMs: timeUse(together, null, now)?.agentMs ?? 0,
    maxAgents: use?.maxAgents ?? 0,
    parallelism: use && working > 0 ? use.agentMs / working : null,
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

/** Bins that widen with time, so short and long tasks both read: 0–5 m … 3–4 h, 4 h+ */
export const TIME_BINS = [0, 5, 10, 20, 30, 45, 60, 90, 120, 180, 240];

const binLabel = (m: number) => (m < 60 ? `${m} m` : `${m / 60} h`);

/**
 * Every done task's work time, binned: atm's (ATM-721), every attempt and round, less its reviews and blocks. Tasks
 * without any work are left out.
 */
export function timeHistogram(tasks: Task[]): TimeHistogram {
  const timed: TimedTask[] = tasks.flatMap((t) => {
    const minutes = taskKind(t) === 'done' ? workMinutes(t, 0) : null;
    return minutes === null ? [] : [{ id: t.id, ...keyOf(t), name: t.name, minutes, withYou: withYou(t) }];
  });
  const bins: TimeBin[] = TIME_BINS.map((from, i) => {
    const to = TIME_BINS[i + 1] ?? Infinity;
    const inBin = timed.filter((t) => t.minutes >= from && t.minutes < to);
    return {
      from,
      to,
      label: to === Infinity ? `${binLabel(from)}+` : `${binLabel(from)}–${binLabel(to)}`,
      alone: inBin.filter((t) => !t.withYou),
      withYou: inBin.filter((t) => t.withYou),
    };
  });
  const minutes = timed.map((t) => t.minutes);
  return {
    bins,
    medianAlone: median(timed.filter((t) => !t.withYou).map((t) => t.minutes)),
    medianWithYou: median(timed.filter((t) => t.withYou).map((t) => t.minutes)),
    min: minutes.length ? Math.min(...minutes) : null,
    max: minutes.length ? Math.max(...minutes) : null,
    average: minutes.length ? minutes.reduce((a, b) => a + b, 0) / minutes.length : null,
    slowest: [...timed].sort((a, b) => b.minutes - a.minutes).slice(0, 5),
  };
}
