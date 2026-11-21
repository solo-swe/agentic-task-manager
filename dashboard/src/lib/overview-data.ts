// The overview's views across projects: Agents (T-333), Activity (T-328), Progress (T-329). Each project's tasks, its
// archive (T-355) and claims come from the routes a project's page uses (api/v1/projects/<id>/tasks, with
// ?archived=true for the archive, and p/<id>/claims.json), loaded while the overview is shown; a refresh asks
// conditionally, so an unchanged answer costs a 304. The charts a project's page has then run over every project's
// tasks (mergedTasks), or per project and combined (combinedTimeline).
import { agentTimeline, UNATTRIBUTED, type AgentBar, type AgentLane, type AgentTimeline } from './agents';
import { loadClaims, type Claim, type ClaimMap } from './claims';
import { median } from './insights';
import { browserFetch, type Fetch } from './api/http';
import { loadArchive, loadTasks, type ProjectEntry, type Validators } from './load';
import { splitReleases, type Release } from './releases';
import { activeSprint, type Sprint, type SprintModel } from './sprints';
import type { Task, TasksFile } from './types';
import { taskKind, type Vocabulary } from './vocab';

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

/** One project's row on the overview's Progress tab (T-329) */
export interface ProjectProgress {
  project: ProjectEntry;
  /** Done-kind tasks, and every task but the skipped ones */
  done: number;
  total: number;
  /** Its active sprint, with atm's figures, if one is active */
  sprint?: Sprint;
  /** Its releases still open, in its order, with atm's figures */
  releases: { id: string; name?: string; done: number; total: number }[];
}

export function projectProgress({ project, tasks, sprints, releases }: ProjectSource): ProjectProgress {
  const kinds = tasks.map(taskKind);
  const active = activeSprint(sprints);
  return {
    project,
    done: kinds.filter((k) => k === 'done').length,
    total: kinds.filter((k) => k !== 'skipped').length,
    ...(active ? { sprint: active } : {}),
    releases: splitReleases(releases).open.map((r) => ({
      id: r.id,
      ...(r.name ? { name: r.name } : {}),
      done: r.done,
      total: r.total,
    })),
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
 * project so the chart stacks them, and the average per period.
 */
export function velocityByPeriod(
  sources: readonly ProjectSource[],
  now: number,
  unit: VelocityUnit = 'week',
): { bars: VelocityBar[]; average: number } {
  const { ms, count } = VELOCITY_UNITS.find((u) => u.unit === unit)!;
  const done = sources.map(({ tasks }) =>
    tasks
      .filter((t) => taskKind(t) === 'done')
      .map((t) => Date.parse(t.completed_at ?? ''))
      .filter((t) => !Number.isNaN(t)),
  );
  const bars = Array.from({ length: count }, (_, i) => {
    const start = now - (count - i) * ms;
    const byProject = done.map((times) => times.filter((t) => t > start && t <= start + ms).length);
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
