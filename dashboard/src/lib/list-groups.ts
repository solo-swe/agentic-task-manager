// The List tab's groups (T-323): by sprint, the planning view (the active sprint, the planned ones in order, the
// backlog, then the closed ones, most recent first), by label or by component, the way /atm:plan-sprint groups the
// backlog, or none.
import { BACKLOG } from './filters';
import type { Sprint, SprintModel } from './sprints';
import type { Task } from './types';
import { labelOrder, labelsOf, labelText, type Vocabulary } from './vocab';

export type ListGroupBy = 'sprint' | 'label' | 'component' | 'none';
export const LIST_GROUP_BYS: readonly ListGroupBy[] = ['sprint', 'label', 'component', 'none'];

export interface ListGroup {
  /** A sprint's ID or BACKLOG, a label, a component; '' for the tasks with none, 'all' when not grouped */
  key: string;
  label: string;
  tasks: Task[];
  /** Grouped by sprint: the sprint (none for the backlog) */
  sprint?: Sprint;
}

const NONE = '';

function bySprint(tasks: Task[], model: SprintModel): ListGroup[] {
  const of = new Map<string, Task[]>();
  for (const t of tasks) {
    const key = model.sprintOf(t) ?? BACKLOG;
    of.set(key, [...(of.get(key) ?? []), t]);
  }
  const state = (s: Sprint['state']) => model.sprints.filter((x) => x.state === s);
  const ordered = [
    ...state('active'),
    ...state('planned'),
    undefined, // the backlog
    ...state('closed').reverse(),
  ];
  return ordered
    .map((sprint) => {
      const key = sprint?.id ?? BACKLOG;
      return { key, label: sprint?.id ?? 'Backlog', tasks: of.get(key) ?? [], sprint };
    })
    .filter((g) => g.tasks.length > 0);
}

/** Groups for a value a task has none or several of: a task with several is in each of their groups */
function byValues(tasks: Task[], valuesOf: (t: Task) => string[], order: (a: string, b: string) => number) {
  const of = new Map<string, Task[]>();
  for (const t of tasks) {
    const values = valuesOf(t);
    for (const v of values.length ? values : [NONE]) of.set(v, [...(of.get(v) ?? []), t]);
  }
  return [...of.keys()]
    .sort((a, b) => (a === NONE ? 1 : b === NONE ? -1 : order(a, b)))
    .map((key) => ({ key, tasks: of.get(key)! }));
}

export function groupList(tasks: Task[], by: ListGroupBy, model: SprintModel, vocab: Vocabulary): ListGroup[] {
  if (by === 'sprint') return bySprint(tasks, model);
  if (by === 'label') {
    return byValues(tasks, (t) => labelsOf(vocab, t), labelOrder(vocab)).map((g) => ({
      ...g,
      label: g.key === NONE ? 'No label' : labelText(vocab, g.key),
    }));
  }
  if (by === 'component') {
    const components = (t: Task) =>
      Array.isArray(t.components) ? t.components.filter((c): c is string => typeof c === 'string') : [];
    return byValues(tasks, components, (a, b) => a.localeCompare(b)).map((g) => ({
      ...g,
      label: g.key === NONE ? 'No component' : g.key,
    }));
  }
  return [{ key: 'all', label: 'All', tasks }];
}
