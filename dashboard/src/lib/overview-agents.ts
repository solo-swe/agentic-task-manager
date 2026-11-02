// The overview's Agents tab (T-333): every listed project's agents on one timeline. Each project's tasks and claims come
// from the routes a project's page uses (p/<id>/tasks.json, p/<id>/claims.json); a refresh asks conditionally, so an
// unchanged project costs a 304. Lanes are by agent across projects (a person often works in several), each bar
// naming its project.
import { agentTimeline, UNATTRIBUTED, type AgentBar, type AgentLane, type AgentTimeline } from './agents';
import { loadClaims, type ClaimMap } from './claims';
import { median } from './insights';
import { loadTasks, type ProjectEntry, type Validators } from './load';
import type { Task } from './types';
import { resolveVocabulary, type Vocabulary } from './vocab';

export interface AgentSource {
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
export async function loadAgentSources(
  projects: readonly ProjectEntry[],
  previous: readonly AgentSource[] = [],
  fetchFn: typeof fetch = fetch,
): Promise<AgentSource[]> {
  const before = new Map(previous.map((s) => [s.project.id, s]));
  const loaded = await Promise.all(
    projects
      .filter((p) => !p.missing)
      .map(async (project): Promise<AgentSource | null> => {
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
          console.error(`Loading ${project.id}'s tasks for the Agents tab failed:`, e);
          return old ? { ...old, project } : null;
        }
      }),
  );
  return loaded.filter((s): s is AgentSource => s !== null);
}

/** One timeline of every source's agents: a lane per agent across projects, most recently active first */
export function combinedTimeline(sources: readonly AgentSource[], now: number): AgentTimeline | null {
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
