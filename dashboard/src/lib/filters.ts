import { shortAgent } from './claims';
import { activeSprint, effectiveRelease, NO_SPRINTS, type SprintModel, type SprintState } from './sprints';
import { STAGE_LABELS, STAGES, stageOf, type Stage } from './stages';
import type { Task } from './types';
import { startedBy } from './work';
import {
  DEFAULT_VOCABULARY,
  labelOrder,
  labelsOf,
  labelText,
  priorityLabel,
  priorityOf,
  priorityRank,
  statusLabel,
  statusRank,
  typeLabel,
  type Vocabulary,
} from './vocab';

export type FilterGroup =
  'sprint' | 'release' | 'component' | 'type' | 'status' | 'priority' | 'stage' | 'label' | 'agent';
export const FILTER_GROUPS: FilterGroup[] = [
  'sprint',
  'release',
  'component',
  'type',
  'status',
  'priority',
  'stage',
  'label',
  'agent',
];
/** Each group's name, as the filter popup and the scope bar's "filtered" note show it */
export const FILTER_LABELS: Record<FilterGroup, string> = {
  sprint: 'Sprint',
  release: 'Release',
  component: 'Component',
  type: 'Type',
  status: 'Status',
  priority: 'Priority',
  stage: 'Stage',
  label: 'Labels',
  agent: 'Started by',
};
/** The Sprint filter's value for the tasks with no sprint (a colon keeps it apart from any sprint's name) */
export const BACKLOG = ':backlog';
/** The Sprint filter's value for the active sprint, whichever it is: the selection follows when the next one starts */
export const ACTIVE = ':active';

/** Selected values per group; an empty set means "All". */
export type Filters = Record<FilterGroup, ReadonlySet<string>>;

export interface FilterOption {
  value: string;
  label: string;
  /** A sprint's state, for the Sprint filter's markers (▶ active, ✓ closed) */
  state?: SprintState;
}

// The active sprint's ID per model: valuesOf asks for it once per task
const activeIds = new WeakMap<SprintModel, string | undefined>();
function activeId(model: SprintModel): string | undefined {
  if (!activeIds.has(model)) activeIds.set(model, activeSprint(model)?.id);
  return activeIds.get(model);
}

/** The Sprint filter's selection as it applies: ACTIVE stays selected while no sprint is active, but selects nothing */
export function sprintSelection(selected: ReadonlySet<string>, model: SprintModel): ReadonlySet<string> {
  if (!selected.has(ACTIVE) || activeId(model) !== undefined) return selected;
  const applied = new Set(selected);
  applied.delete(ACTIVE);
  return applied;
}

function applied(filters: Filters, model: SprintModel): Filters {
  const sprint = sprintSelection(filters.sprint, model);
  return sprint === filters.sprint ? filters : { ...filters, sprint };
}

/** Up to this many sprints, the Sprint filter lists them all, with no toggle for the closed ones */
export const SPRINTS_ALWAYS_SHOWN = 6;

/**
 * The sprint options the filter panel lists: all of them when there are few, or when `showClosed`; otherwise the open
 * ones (active, planned, the backlog) and any selected one, so a selection never hides. `hidden` counts the closed
 * sprints left out (SR-7.1).
 */
export function shownSprintOptions(
  options: FilterOption[],
  closed: ReadonlySet<string>,
  selected: ReadonlySet<string>,
  showClosed: boolean,
): { shown: FilterOption[]; closed: number; hidden: number } {
  const closedCount = options.filter((o) => closed.has(o.value)).length;
  if (options.length <= SPRINTS_ALWAYS_SHOWN || closedCount === 0) return { shown: options, closed: 0, hidden: 0 };
  const shown = showClosed ? options : options.filter((o) => !closed.has(o.value) || selected.has(o.value));
  return { shown, closed: closedCount, hidden: options.length - shown.length };
}

export function emptyFilters(): Filters {
  return {
    sprint: new Set(),
    release: new Set(),
    component: new Set(),
    type: new Set(),
    status: new Set(),
    priority: new Set(),
    stage: new Set(),
    label: new Set(),
    agent: new Set(),
  };
}

/** Filters from plain lists of values (as saved per project). */
export function filtersFrom(values: Record<FilterGroup, string[]>): Filters {
  return {
    sprint: new Set(values.sprint),
    release: new Set(values.release),
    component: new Set(values.component),
    type: new Set(values.type),
    status: new Set(values.status),
    priority: new Set(values.priority),
    stage: new Set(values.stage),
    label: new Set(values.label),
    agent: new Set(values.agent),
  };
}

/** Position in a vocabulary list; values it doesn't list come after, alphabetically. */
function byVocabulary(ids: string[]) {
  const rank = (v: string) => (ids.includes(v) ? ids.indexOf(v) : ids.length);
  return (a: string, b: string) => rank(a) - rank(b) || a.localeCompare(b);
}

/**
 * The values a task has in each group: one, except labels (none or several). A task's priority is its effective one
 * (missing or unknown: the default).
 */
export function filterValues(group: FilterGroup, t: Task, vocab: Vocabulary, model: SprintModel): string[] {
  return valuesOf(group, t, vocab, model);
}

function valuesOf(group: FilterGroup, t: Task, vocab: Vocabulary, model: SprintModel): string[] {
  if (group === 'sprint') {
    const sprint = model.sprintOf(t);
    return sprint === undefined ? [BACKLOG] : sprint === activeId(model) ? [sprint, ACTIVE] : [sprint];
  }
  if (group === 'release') {
    const release = effectiveRelease(model, t);
    return release ? [release] : [];
  }
  if (group === 'component')
    return Array.isArray(t.components) ? t.components.filter((c) => typeof c === 'string') : [];
  if (group === 'label') return labelsOf(vocab, t);
  if (group === 'stage') {
    const stage = stageOf(t, vocab);
    return stage ? [stage] : [];
  }
  if (group === 'agent') {
    const actor = startedBy(t);
    return actor ? [actor] : [];
  }
  if (group === 'priority') return [priorityOf(vocab, t)];
  return [group === 'type' ? t.type : t.status];
}

/**
 * The Sprint filter's options: the active sprint, the planned ones in order, the backlog, then the closed ones, most
 * recent first; only sprints that have tasks here, and the backlog when some task has no sprint. The active one's
 * value is ACTIVE, so selecting it follows the project to its next active sprint.
 */
function sprintOptions(tasks: Task[], model: SprintModel): FilterOption[] {
  const used = new Set(tasks.map((t) => model.sprintOf(t) ?? BACKLOG));
  const rank = { active: 0, planned: 1, closed: 3 } as const;
  const sprints = model.sprints
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => used.has(s.id))
    .sort((a, b) => rank[a.s.state] - rank[b.s.state] || (a.s.state === 'closed' ? b.i - a.i : a.i - b.i));
  const options: FilterOption[] = sprints.map(({ s }) => ({
    value: s.state === 'active' ? ACTIVE : s.id,
    label: s.id,
    state: s.state,
  }));
  if (used.has(BACKLOG)) {
    const at = sprints.findIndex(({ s }) => s.state === 'closed');
    options.splice(at === -1 ? options.length : at, 0, { value: BACKLOG, label: 'Backlog' });
  }
  return options;
}

/**
 * Filter buttons for the values in the data: sprints (sprintOptions), releases and components alphabetically, types,
 * statuses and priorities in the vocabulary's order and with its labels. Statuses list the whole vocabulary, so a
 * status no task has yet (To Do once everything is done) can still be chosen.
 */
export function filterOptions(
  tasks: Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): Record<FilterGroup, FilterOption[]> {
  const values = (group: FilterGroup) =>
    [...new Set(tasks.flatMap((t) => valuesOf(group, t, vocab, model)))].sort((a, b) => a.localeCompare(b));
  const types = [...new Set(tasks.map((t) => t.type))].sort(byVocabulary(vocab.types.map((t) => t.id)));
  // Every status of the project, also those no task has now, then any unknown one a task uses
  const statuses = [...new Set([...vocab.statuses.map((s) => s.id), ...tasks.map((t) => t.status)])].sort(
    (a, b) => statusRank(vocab, a) - statusRank(vocab, b) || a.localeCompare(b),
  );
  const priorities = [...new Set(tasks.map((t) => priorityOf(vocab, t)))].sort(
    (a, b) => priorityRank(vocab, a) - priorityRank(vocab, b),
  );
  const labels = [...new Set(tasks.flatMap((t) => labelsOf(vocab, t)))].sort(labelOrder(vocab));
  const agents = values('agent');
  const stages = new Set(tasks.flatMap((t) => valuesOf('stage', t, vocab, model)));
  return {
    sprint: sprintOptions(tasks, model),
    release: values('release').map((r) => ({ value: r, label: r })),
    component: values('component').map((c) => ({ value: c, label: c })),
    type: types.map((t) => ({ value: t, label: typeLabel(vocab, t) })),
    status: statuses.map((s) => ({ value: s, label: statusLabel(vocab, s) })),
    priority: priorities.map((p) => ({ value: p, label: `${p} ${priorityLabel(vocab, p)}` })),
    stage: STAGES.filter((s: Stage) => stages.has(s)).map((s) => ({ value: s, label: STAGE_LABELS[s] })),
    label: labels.map((l) => ({ value: l, label: labelText(vocab, l) })),
    agent: agents.map((a) => ({ value: a, label: shortAgent(a) })),
  };
}

/** OR within a group (for labels: any of the selected ones), AND across groups. */
export function filterTasks(
  tasks: Task[],
  filters: Filters,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): Task[] {
  filters = applied(filters, model);
  return tasks.filter((t) =>
    FILTER_GROUPS.every(
      (group) => filters[group].size === 0 || valuesOf(group, t, vocab, model).some((v) => filters[group].has(v)),
    ),
  );
}

/** "all" clears the group; any other value toggles. Returns a new Filters object. */
export function toggleFilter(filters: Filters, group: FilterGroup, value: string): Filters {
  const next = new Set(filters[group]);
  if (value === 'all') next.clear();
  else if (next.has(value)) next.delete(value);
  else next.add(value);
  return { ...filters, [group]: next };
}

export function isActive(filters: Filters, group: FilterGroup, value: string): boolean {
  return value === 'all' ? filters[group].size === 0 : filters[group].has(value);
}

export interface FacetCounts {
  /** Tasks matching the other groups' selections: what "All" in this group would show. */
  all: number;
  /** Per value of this group, among those tasks. Values with no tasks are absent (count 0). */
  values: ReadonlyMap<string, number>;
}

/**
 * Faceted counts for the filter buttons: for each group, how many tasks each value would show given the selections
 * in the other groups (the group's own selection is ignored, so a count says what clicking that value adds).
 */
export function facetCounts(
  tasks: Task[],
  filters: Filters,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): Record<FilterGroup, FacetCounts> {
  const result = {} as Record<FilterGroup, FacetCounts>;
  filters = applied(filters, model);
  for (const group of FILTER_GROUPS) {
    const others = filterTasks(tasks, { ...filters, [group]: new Set<string>() }, vocab, model);
    const values = new Map<string, number>();
    for (const t of others) {
      for (const v of valuesOf(group, t, vocab, model)) values.set(v, (values.get(v) ?? 0) + 1);
    }
    result[group] = { all: others.length, values };
  }
  return result;
}

/**
 * Keyword search: every word of `query` must appear (case-insensitively) in the task's ID, name, description, type
 * or labels. An empty query matches everything.
 */
export function searchTasks(tasks: Task[], query: string): Task[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return tasks;
  return tasks.filter((t) => {
    const labels = Array.isArray(t.labels) ? t.labels.join(' ') : '';
    const text = `${t.id} ${t.name} ${t.description ?? ''} ${t.type} ${labels}`.toLowerCase();
    return terms.every((term) => text.includes(term));
  });
}

export type ListSort = 'file' | 'priority';

/**
 * The List tab's order: as in the file, or by priority (most urgent first), then phase, then ID — the order agents
 * pick tasks in within a phase.
 */
export function sortTasks(tasks: Task[], sort: ListSort, vocab: Vocabulary = DEFAULT_VOCABULARY): Task[] {
  if (sort === 'file') return tasks;
  return [...tasks].sort(
    (a, b) =>
      priorityRank(vocab, priorityOf(vocab, a)) - priorityRank(vocab, priorityOf(vocab, b)) ||
      a.phase - b.phase ||
      a.id.localeCompare(b.id, undefined, { numeric: true }),
  );
}
