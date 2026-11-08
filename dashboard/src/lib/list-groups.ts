// The List tab's groups (T-323): by sprint, the planning view (the active sprint, the planned ones in order, the
// backlog, then the closed ones, most recent first), by label or by component, the way /atm-plan-sprint groups the
// backlog, or none.
import { componentLabel, componentOrder, taskComponents, type Component } from './components';
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

export function groupList(
  tasks: Task[],
  by: ListGroupBy,
  model: SprintModel,
  vocab: Vocabulary,
  projectComponents: readonly Component[] = [],
): ListGroup[] {
  if (by === 'sprint') return bySprint(tasks, model);
  if (by === 'label') {
    return byValues(tasks, (t) => labelsOf(vocab, t), labelOrder(vocab)).map((g) => ({
      ...g,
      label: g.key === NONE ? 'No label' : labelText(vocab, g.key),
    }));
  }
  if (by === 'component') {
    // In the project's order (meta.components), by their labels; IDs it doesn't define after them, alphabetically
    return byValues(tasks, (t) => taskComponents(t, projectComponents), componentOrder(projectComponents)).map((g) => ({
      ...g,
      label: g.key === NONE ? 'No component' : componentLabel(projectComponents, g.key),
    }));
  }
  return [{ key: 'all', label: 'All', tasks }];
}
