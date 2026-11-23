// The overview's views across projects: Agents (T-333), Activity (T-328), Progress (T-329). Progress is drawn from the
// projects' summaries (ATM-722: atm's figures, no project's tasks loaded for it). For the others, each project's
// tasks, its archive (T-355) and claims come from the routes a project's page uses (api/v1/projects/<id>/tasks, with
// ?archived=true for the archive, and p/<id>/claims.json), loaded while the overview is shown; a refresh asks
// conditionally, so an unchanged answer costs a 304. The charts a project's page has then run over every project's
// tasks (mergedTasks), or per project and combined (combinedTimeline).
import type { ActiveSprint } from '$generated/types';
import { agentTimeline, UNATTRIBUTED, type AgentBar, type AgentLane, type AgentTimeline } from './agents';
import type { Burndown } from './burndown';
import { loadClaims, type Claim, type ClaimMap } from './claims';
import { median } from './insights';
import { browserFetch, type Fetch } from './api/http';
import { loadArchive, loadTasks, type ProjectEntry, type Validators } from './load';
import type { Release } from './releases';
import type { SprintModel } from './sprints';
import type { Task, TasksFile } from './types';
import type { Vocabulary } from './vocab';

export interface ProjectSource {
  project: ProjectEntry;
  /** Its history: tasks.json's tasks, then the archived ones tasks.json doesn't have (T-355), as a project's own
   * history views count them */
  tasks: Task[];
  claims: ClaimMap;
  vocab: Vocabulary;
  /** Its sprints and releases as atm serves them with the tasks (T-329, ATM-720) */
  sprints: SprintModel;
  releases: Release[];
  /** tasks.json's meta, for its ID (a reference to another project's task names it by it, ADR-007 §7) */
  meta?: TasksFile['meta'];
  /** The two files as last loaded, and their validators, for the next conditional load */
  live?: Task[];
  archived?: Task[];
  validators?: Validators;
  archiveValidators?: Validators;
}

/** A project's history: the live tasks, then the archived ones not among them (a task in both is the live one) */
export function withArchive(live: Task[], archived: Task[]): Task[] {
  if (!archived.length) return live;
  const ids = new Set(live.map((t) => t.id));
  return [...live, ...archived.filter((t) => !ids.has(t.id))];
}

/**
 * The sources for `projects` (missing ones left out), refreshed from `previous`: each project's tasks.json and its
 * archive (a missing one is empty), each kept as it was when unchanged (304), and its claims fetched again; a project
 * that fails to load keeps what it had, or is left out.
 */
export async function loadProjectSources(
  projects: readonly ProjectEntry[],
  previous: readonly ProjectSource[] = [],
  fetchFn: Fetch = browserFetch,
): Promise<ProjectSource[]> {
  const before = new Map(previous.map((s) => [s.project.id, s]));
  const loaded = await Promise.all(
    projects
      .filter((p) => !p.missing)
      .map(async (project): Promise<ProjectSource | null> => {
        const old = before.get(project.id);
        try {
          const [result, archive, claims] = await Promise.all([
            loadTasks(project.id, fetchFn, old?.validators),
            loadArchive(project.id, fetchFn, old?.archiveValidators),
            loadClaims(project.id, fetchFn),
          ]);
          if (result.unchanged && archive.unchanged) return old ? { ...old, project, claims } : null;
          if (result.unchanged && !old) return null;
          const live = result.unchanged ? (old!.live ?? old!.tasks) : result.loaded.tasks;
          const archived = archive.unchanged ? (old?.archived ?? []) : archive.tasks;
          return {
            project,
            tasks: withArchive(live, archived),
            claims,
            vocab: result.unchanged ? old!.vocab : result.loaded.vocab,
            sprints: result.unchanged ? old!.sprints : result.loaded.sprints,
            releases: result.unchanged ? old!.releases : result.loaded.releases,
            meta: result.unchanged ? old!.meta : result.loaded.meta,
            live,
            archived,
            validators: result.unchanged ? old!.validators : result.validators,
            archiveValidators: archive.unchanged ? old?.archiveValidators : archive.validators,
          };
        } catch (e) {
          console.error(`Loading ${project.id}'s tasks for the overview failed:`, e);
          return old ? { ...old, project } : null;
        }
      }),
  );
  return loaded.filter((s): s is ProjectSource => s !== null);
}

/** Every source's tasks as one project's, for the charts a project's page has (Activity, Progress) */
export interface MergedTasks {
  /** Each task with a key for an ID ("Project: T-1"), unique across projects; the charts read its kind, atm's
   * (projects name their statuses differently) */
  tasks: Task[];
  /** The live claims, by those keys */
  claims: ClaimMap;
  /** Where each key's task lives, to open it in its project */
  origin: ReadonlyMap<string, { project: ProjectEntry; id: string }>;
}

export function mergedTasks(sources: readonly ProjectSource[]): MergedTasks {
  const tasks: Task[] = [];
  const claims = new Map<string, Claim>();
  const origin = new Map<string, { project: ProjectEntry; id: string }>();
  for (const { project, tasks: own, claims: live } of sources) {
    for (const task of own) {
      const key = `${project.name}: ${task.id}`;
      // Shown by its project's name and its key, with task keys (ADR-007)
      const shown = task.key ? { key: `${project.name}: ${task.key}` } : {};
      tasks.push({ ...task, id: key, ...shown });
      origin.set(key, { project, id: task.id });
      const claim = live.get(task.id);
      if (claim) claims.set(key, { ...claim, task: key });
    }
  }
  return { tasks, claims, origin };
}

/** One project's row on the overview's Progress tab (T-329), from its summary (ATM-722: atm's figures, no project's
 * tasks loaded) */
export interface ProjectProgress {
  project: ProjectEntry;
  /** Done-kind tasks, and every task but the skipped ones, the archived ones too */
  done: number;
  total: number;
  /** Its active sprint, with atm's figures, if one is active */
  sprint?: ActiveSprint;
  /** Its releases still open, in its order, with atm's figures */
  releases: { id: string; name?: string; done: number; total: number }[];
}

/** The rows of the projects that have a summary */
export function progressRows(projects: readonly ProjectEntry[]): ProjectProgress[] {
  return projects.flatMap((project) => {
    const summary = project.summary;
    if (!summary?.progress) return [];
    const { progress } = summary;
    return [
      {
        project,
        done: progress.done,
        total: progress.total,
        ...(summary.active_sprint ? { sprint: summary.active_sprint } : {}),
        releases: progress.open_releases.map((r) => ({
          id: r.id,
          ...(r.name ? { name: r.name } : {}),
          done: r.done,
          total: r.total,
        })),
      },
    ];
  });
}

const DAY = 86_400_000;

/** Every project's burndown as one (T-329), from their summaries' daily series: the scope and what was left at the end
 * of each day, added up, from `from` (or the first day) to now; and the forecast from the agent work left in all of
 * them, one after another, at each one's pace. The chart draws it */
export function overviewBurndown(projects: readonly ProjectEntry[], now: number, from?: number): Burndown {
  const series = projects.flatMap((p) => (p.summary?.progress ? [p.summary.progress] : []));
  const byDay = new Map<number, { scope: number; remaining: number }>();
  for (const s of series)
    for (const [day, scope, remaining] of s.burndown) {
      const at = byDay.get(day) ?? { scope: 0, remaining: 0 };
      byDay.set(day, { scope: at.scope + scope, remaining: at.remaining + remaining });
    }
  if (byDay.size === 0) return { points: [], untimed: true };
  const days = [...byDay.keys()].sort((a, b) => a - b);
  // A project's series ends today: days before its first count none of it
  const points = days
    .map((day) => ({ t: Math.min((day + 1) * DAY, now), ...byDay.get(day)! }))
    .filter((p) => from === undefined || p.t > from);
  const forecasts = series.flatMap((s) => (s.forecast ? [s.forecast] : []));
  const current = points.at(-1);
  const waiting = forecasts.reduce((n, f) => n + f.waiting, 0);
  const minutes = (key: 'likely' | 'low' | 'high') => forecasts.reduce((n, f) => n + f.minutes[key], 0);
  return {
    points,
    ...(current && forecasts.length && current.remaining > waiting
      ? {
          forecast: {
            from: { t: now, remaining: current.remaining },
            to: { t: now + minutes('likely') * 60_000, remaining: waiting },
            range: { low: now + minutes('low') * 60_000, high: now + minutes('high') * 60_000 },
          },
        }
      : {}),
  };
}

/** The velocity's period (T-329): a bar per day, week or month, the last few, the last one ending now */
export type VelocityUnit = 'day' | 'week' | 'month';
export const VELOCITY_UNITS: { unit: VelocityUnit; label: string; ms: number; count: number }[] = [
  { unit: 'day', label: 'Day', ms: 86_400_000, count: 14 },
  { unit: 'week', label: 'Week', ms: 7 * 86_400_000, count: 10 },
  { unit: 'month', label: 'Month', ms: 30 * 86_400_000, count: 6 },
];

export interface VelocityBar {
  start: number;
  /** Tasks done in it, per source (in the sources' order) */
  byProject: number[];
  total: number;
}

/**
 * Tasks done per period across projects, whose sprints don't line up (T-329): a bar per period, oldest first, split by
 * project so the chart stacks them (in `projects`' order), and the average per period. From the summaries' days with
 * completions (ATM-722, atm's count), each day's counted at its middle.
 */
export function velocityOf(
  projects: readonly ProjectEntry[],
  now: number,
  unit: VelocityUnit = 'week',
): { bars: VelocityBar[]; average: number } {
  const { ms, count } = VELOCITY_UNITS.find((u) => u.unit === unit)!;
  const done = projects.map((p) =>
    (p.summary?.progress?.done_days ?? []).map(([day, n]) => ({ at: day * DAY + DAY / 2, n })),
  );
  const bars = Array.from({ length: count }, (_, i) => {
    const start = now - (count - i) * ms;
    const byProject = done.map((days) =>
      days.filter((d) => d.at > start && d.at <= start + ms).reduce((n, d) => n + d.n, 0),
    );
    return { start, byProject, total: byProject.reduce((n, c) => n + c, 0) };
  });
  return { bars, average: bars.reduce((n, b) => n + b.total, 0) / count };
}

/** Between a project and its worker in projectTasks' `started_by` */
const PROJECT_SEP = ' · ';

/**
 * mergedTasks by project (T-328 review): each task's `type` is its project's name, and its `started_by` its project's
 * name before its worker's, so the Activity charts that split by type or by worker split by project instead (one agent
 * on two projects' tasks counted per project).
 */
export function projectTasks(merged: MergedTasks): Task[] {
  return merged.tasks.map((t) => {
    const name = merged.origin.get(t.id)?.project.name ?? '';
    return { ...t, type: name, started_by: name + PROJECT_SEP + (t.started_by ?? t.agent ?? ''), agent: undefined };
  });
}

/** The project of a projectTasks worker */
export function projectOfWorker(worker: string): string {
  const at = worker.indexOf(PROJECT_SEP);
  return at < 0 ? worker : worker.slice(0, at);
}

/** One timeline of every source's agents: a lane per agent across projects, most recently active first */
export function combinedTimeline(sources: readonly ProjectSource[], now: number): AgentTimeline | null {
  const lanes = new Map<string, AgentLane & { names: Set<string> }>();
  for (const { project, tasks, claims } of sources) {
    const timeline = agentTimeline(tasks, claims, now);
    if (!timeline) continue;
    const named = { id: project.id, name: project.name };
    for (const lane of timeline.lanes) {
      const bars: AgentBar[] = lane.bars.map((b) => ({ ...b, project: named, key: `${project.id}/${b.task.id}` }));
      const into = lanes.get(lane.agent);
      if (!into) {
        lanes.set(lane.agent, { ...lane, bars, names: new Set([project.name]) });
        continue;
      }
      into.bars.push(...bars);
      into.done += lane.done;
      into.current.push(...lane.current);
      into.lastActive = Math.max(into.lastActive, lane.lastActive);
      into.names.add(project.name);
    }
  }
  if (lanes.size === 0) return null;
  const merged: AgentLane[] = [...lanes.values()].map(({ names, ...lane }) => ({
    ...lane,
    bars: lane.bars.sort((a, b) => a.start - b.start),
    medianMinutes: median(lane.bars.filter((b) => !b.live).map((b) => (b.end - b.start) / 60_000)),
    projects: [...names].sort((a, b) => a.localeCompare(b)),
  }));
  merged.sort(
    (a, b) => Number(a.agent === UNATTRIBUTED) - Number(b.agent === UNATTRIBUTED) || b.lastActive - a.lastActive,
  );
  const all = merged.flatMap((l) => l.bars);
  return { start: Math.min(...all.map((b) => b.start)), end: Math.max(...all.map((b) => b.end)), lanes: merged };
}
