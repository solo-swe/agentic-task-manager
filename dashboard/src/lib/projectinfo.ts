// The Project tab's Information (docs/project-stats-proposal.md §3, T-252): who worked on the project, how it pushes,
// and its vocabularies with how many tasks use each entry. From the history (archived tasks too), whatever the filters.
import type { Stats } from '$generated/types';
import { NO_SPRINTS, type SprintModel, type SprintState } from './sprints';
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

/** The people and the agents, as atm counts them (ATM-722: the stats route's `actors`): the people with the tasks they
 * created, started and completed, most active first; the agents in one summary. None before atm has answered */
export function actorsOf(stats: Stats | null | undefined): { people: Person[]; agents: AgentsSummary } {
  const served = stats?.actors as
    | { people?: Person[]; agents?: { sessions: number; started: number; completed: number; agent_ms: number } }
    | undefined;
  const agents = served?.agents;
  return {
    people: served?.people ?? [],
    agents: {
      sessions: agents?.sessions ?? 0,
      started: agents?.started ?? 0,
      completed: agents?.completed ?? 0,
      agentMs: agents?.agent_ms ?? 0,
    },
  };
}

export interface VocabCounts {
  /** The sprints in the planning order (active, planned, closed most recent first) with their task counts (T-323):
   * atm's (ATM-720), its tasks but the skipped ones */
  sprints: { id: string; state: SprintState; count: number }[];
  /** Unfinished tasks in no sprint, as `atm sprint list` counts them */
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
  const backlog = model.backlog;
  const inState = (state: SprintState) => model.sprints.filter((s) => s.state === state);
  const sprints = [...inState('active'), ...inState('planned'), ...inState('closed').reverse()].map((s) => ({
    id: s.id,
    state: s.state,
    count: s.total,
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
