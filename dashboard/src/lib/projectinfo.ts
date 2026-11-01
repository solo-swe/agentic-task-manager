// The Project tab's Information (docs/project-stats-proposal.md §3, T-252): who worked on the project, how it pushes,
// and its vocabularies with how many tasks use each entry. From the history (archived tasks too), whatever the filters.
import { isAgent } from './actors';
import { NO_SPRINTS, type SprintModel, type SprintState } from './sprints';
import { timeUse } from './timeuse';
import type { Task } from './types';
import {
  DEFAULT_VOCABULARY,
  kindOf,
  labelOrder,
  labelText,
  labelsOf,
  priorityOf,
  statusLabel,
  typeLabel,
  type StatusKind,
  type Vocabulary,
} from './vocab';
import { isDone, startedBy } from './work';

export interface Person {
  name: string;
  created: number;
  started: number;
  completed: number;
}

export interface AgentsSummary {
  /** Distinct agent sessions (claude:…) */
  sessions: number;
  started: number;
  completed: number;
  /** Work time of the tasks agents started (or nobody recorded), parallel agents adding up */
  agentMs: number;
}

const count = <K>(map: Map<K, number>, key: K) => map.set(key, (map.get(key) ?? 0) + 1);

/**
 * The people (anyone who isn't a Claude Code session) with the tasks they created, started and completed, most
 * active first; and the agents in one summary. Tasks from before atm recorded who started them (`agent`, or nobody)
 * count as agent work, as on the Agents tab.
 */
export function actors(
  tasks: Task[],
  now: number,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
): { people: Person[]; agents: AgentsSummary } {
  const people = new Map<string, Person>();
  const person = (name: string) => {
    if (!people.has(name)) people.set(name, { name, created: 0, started: 0, completed: 0 });
    return people.get(name)!;
  };
  const sessions = new Set<string>();
  const agents = { sessions: 0, started: 0, completed: 0, agentMs: 0 };
  const agentWork: Task[] = [];
  for (const t of tasks) {
    const starter = startedBy(t);
    if (t.created_by && !isAgent(t.created_by)) person(t.created_by).created++;
    if (starter && !isAgent(starter)) person(starter).started++;
    else if (t.started_at || starter) {
      agents.started++;
      if (starter) sessions.add(starter);
      agentWork.push(t);
    }
    // Before v4 nobody recorded who completed a task: a finished task an agent started counts as the agent's
    if (t.completed_by && !isAgent(t.completed_by)) person(t.completed_by).completed++;
    else if (t.completed_by) {
      agents.completed++;
      sessions.add(t.completed_by);
    } else if (isDone(t, vocab) && agentWork.at(-1) === t) agents.completed++;
    if (t.created_by && isAgent(t.created_by)) sessions.add(t.created_by);
  }
  agents.sessions = sessions.size;
  agents.agentMs = timeUse(agentWork, null, now, vocab)?.agentMs ?? 0;
  const total = (p: Person) => p.created + p.started + p.completed;
  return {
    people: [...people.values()].sort((a, b) => total(b) - total(a) || a.name.localeCompare(b.name)),
    agents,
  };
}

export interface Workflow {
  type: string;
  base?: string;
  verify: boolean;
  commitTemplate?: string;
  /** Trailer lines every commit must carry */
  trailers: string[];
}

/** meta.workflow, or null when the project has none yet */
export function workflowOf(meta: { workflow?: unknown } | undefined): Workflow | null {
  const w = meta?.workflow as Record<string, unknown> | undefined;
  if (!w || typeof w !== 'object' || typeof w.type !== 'string') return null;
  const trailers = (w.trailers as { require?: unknown } | undefined)?.require;
  return {
    type: w.type,
    base: typeof w.base === 'string' ? w.base : undefined,
    verify: w.verify !== false,
    commitTemplate: typeof w.commit_template === 'string' ? w.commit_template : undefined,
    trailers: Array.isArray(trailers) ? trailers.filter((l): l is string => typeof l === 'string') : [],
  };
}

export interface VocabCounts {
  /** The sprints in the planning order (active, planned, closed most recent first) with their task counts (T-323) */
  sprints: { id: string; state: SprintState; count: number }[];
  /** Tasks in no sprint */
  backlog: number;
  /** meta.labels first, in its order (also unused ones), then labels only the tasks use */
  labels: { id: string; text: string; count: number; listed: boolean }[];
  types: { id: string; label: string; count: number }[];
  statuses: { id: string; label: string; kind: StatusKind | undefined; count: number }[];
  priorities: { id: string; label: string; count: number; isDefault: boolean }[];
}

/** Each vocabulary entry with how many tasks use it; entries the tasks use but the vocabulary lacks come last */
export function vocabCounts(
  tasks: Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): VocabCounts {
  const sprintUse = new Map<string, number>();
  let backlog = 0;
  for (const t of tasks) {
    const sprint = model.sprintOf(t);
    if (sprint === undefined) backlog++;
    else count(sprintUse, sprint);
  }
  const inState = (state: SprintState) => model.sprints.filter((s) => s.state === state);
  const sprints = [...inState('active'), ...inState('planned'), ...inState('closed').reverse()].map((s) => ({
    id: s.id,
    state: s.state,
    count: sprintUse.get(s.id) ?? 0,
  }));

  const labelUse = new Map<string, number>();
  for (const t of tasks) for (const l of labelsOf(vocab, t)) count(labelUse, l);
  const labels = [...new Set([...vocab.labels.map((l) => l.id), ...labelUse.keys()])]
    .sort(labelOrder(vocab))
    .map((id) => ({
      id,
      text: labelText(vocab, id),
      count: labelUse.get(id) ?? 0,
      listed: vocab.labels.some((l) => l.id === id),
    }));

  const typeUse = new Map<string, number>();
  const statusUse = new Map<string, number>();
  const priorityUse = new Map<string, number>();
  for (const t of tasks) {
    count(typeUse, t.type);
    count(statusUse, t.status);
    count(priorityUse, priorityOf(vocab, t));
  }
  // The vocabulary's IDs, then the ones only tasks use
  const ids = (defined: { id: string }[], use: Map<string, number>) => [
    ...defined.map((d) => d.id),
    ...[...use.keys()].filter((id) => !defined.some((d) => d.id === id)).sort(),
  ];
  return {
    sprints,
    backlog,
    labels,
    types: ids(vocab.types, typeUse).map((id) => ({ id, label: typeLabel(vocab, id), count: typeUse.get(id) ?? 0 })),
    statuses: ids(vocab.statuses, statusUse).map((id) => ({
      id,
      label: statusLabel(vocab, id),
      kind: kindOf(vocab, id),
      count: statusUse.get(id) ?? 0,
    })),
    priorities: vocab.priorities.map((p) => ({
      ...p,
      count: priorityUse.get(p.id) ?? 0,
      isDefault: p.id === vocab.defaultPriority,
    })),
  };
}
