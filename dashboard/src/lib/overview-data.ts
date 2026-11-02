// The overview's views across projects: Agents (T-333), Activity (T-328), Progress (T-329). Each project's tasks and
// claims come from the routes a project's page uses (p/<id>/tasks.json, p/<id>/claims.json), loaded while the overview
// is shown; a refresh asks conditionally, so an unchanged project costs a 304. The charts a project's page has then run
// over every project's tasks (mergedTasks), or per project and combined (combinedTimeline).
import { agentTimeline, UNATTRIBUTED, type AgentBar, type AgentLane, type AgentTimeline } from './agents';
import { loadClaims, type Claim, type ClaimMap } from './claims';
import { median } from './insights';
import { loadTasks, type ProjectEntry, type Validators } from './load';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, resolveVocabulary, type StatusKind, type Vocabulary } from './vocab';

export interface ProjectSource {
  project: ProjectEntry;
  tasks: Task[];
  claims: ClaimMap;
  vocab: Vocabulary;
  validators?: Validators;
}

/**
 * The sources for `projects` (missing ones left out), refreshed from `previous`: a project whose tasks.json is
 * unchanged keeps its tasks (its claims are fetched again); one that fails to load keeps what it had, or is left out.
 */
export async function loadProjectSources(
  projects: readonly ProjectEntry[],
  previous: readonly ProjectSource[] = [],
  fetchFn: typeof fetch = fetch,
): Promise<ProjectSource[]> {
  const before = new Map(previous.map((s) => [s.project.id, s]));
  const loaded = await Promise.all(
    projects
      .filter((p) => !p.missing)
      .map(async (project): Promise<ProjectSource | null> => {
        const old = before.get(project.id);
        try {
          const [result, claims] = await Promise.all([
            loadTasks(project.id, fetchFn, old?.validators),
            loadClaims(project.id, fetchFn),
          ]);
          if (result.unchanged) return old ? { ...old, project, claims } : null;
          const { tasks, meta } = result.loaded;
          return { project, tasks, claims, vocab: resolveVocabulary(meta), validators: result.validators };
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
  /** Each task with a key for an ID ("Project: T-1"), unique across projects, and its status as the default
   * vocabulary's of the same kind (projects name their statuses differently) */
  tasks: Task[];
  /** The live claims, by those keys */
  claims: ClaimMap;
  /** Where each key's task lives, to open it in its project */
  origin: ReadonlyMap<string, { project: ProjectEntry; id: string }>;
}

const DEFAULT_STATUS = new Map<StatusKind, string>(
  [...DEFAULT_VOCABULARY.statuses].reverse().map((s) => [s.kind, s.id]),
);

export function mergedTasks(sources: readonly ProjectSource[]): MergedTasks {
  const tasks: Task[] = [];
  const claims = new Map<string, Claim>();
  const origin = new Map<string, { project: ProjectEntry; id: string }>();
  for (const { project, tasks: own, claims: live, vocab } of sources) {
    for (const task of own) {
      const key = `${project.name}: ${task.id}`;
      const kind = kindOf(vocab, task.status);
      tasks.push({ ...task, id: key, status: (kind && DEFAULT_STATUS.get(kind)) ?? task.status });
      origin.set(key, { project, id: task.id });
      const claim = live.get(task.id);
      if (claim) claims.set(key, { ...claim, task: key });
    }
  }
  return { tasks, claims, origin };
}

/** How many tasks each source finished in `period` (all time without one), most first; none left out */
export function completionsByProject(
  sources: readonly ProjectSource[],
  period: { start: number; end: number } | null,
): { project: ProjectEntry; completed: number }[] {
  return sources
    .map(({ project, tasks, vocab }) => ({
      project,
      completed: tasks.filter((t) => {
        if (kindOf(vocab, t.status) !== 'done') return false;
        const at = Date.parse(t.completed_at ?? '');
        return !Number.isNaN(at) && (!period || (at >= period.start && at <= period.end));
      }).length,
    }))
    .sort((a, b) => b.completed - a.completed || a.project.name.localeCompare(b.project.name));
}

/** One timeline of every source's agents: a lane per agent across projects, most recently active first */
export function combinedTimeline(sources: readonly ProjectSource[], now: number): AgentTimeline | null {
  const lanes = new Map<string, AgentLane & { names: Set<string> }>();
  for (const { project, tasks, claims, vocab } of sources) {
    const timeline = agentTimeline(tasks, claims, now, undefined, vocab);
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
