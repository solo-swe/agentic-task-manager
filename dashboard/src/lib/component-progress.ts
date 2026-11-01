// The Progress tab's "By component" table (T-323): how far each component's tasks are. A task in several components
// counts in each; tasks in none get their own row, last.
import { componentLabel, componentOrder, taskComponents, type Component } from './components';
import type { Task } from './types';
import { kindOf, type Vocabulary } from './vocab';

export interface ComponentProgress {
  /** The component's ID; '' for the tasks in no component */
  id: string;
  label: string;
  /** Tasks not skipped */
  total: number;
  done: number;
  skipped: number;
  /** To do or in progress: work agents can still do */
  open: number;
  inReview: number;
  blocked: number;
}

export function componentProgress(
  tasks: readonly Task[],
  components: readonly Component[],
  vocab: Vocabulary,
): ComponentProgress[] {
  const rows = new Map<string, ComponentProgress>();
  const row = (id: string) => {
    let r = rows.get(id);
    if (!r) {
      const label = id === '' ? 'No component' : componentLabel(components, id);
      r = { id, label, total: 0, done: 0, skipped: 0, open: 0, inReview: 0, blocked: 0 };
      rows.set(id, r);
    }
    return r;
  };
  for (const t of tasks) {
    const kind = kindOf(vocab, t.status);
    const ids = taskComponents(t, components);
    for (const id of ids.length ? ids : ['']) {
      const r = row(id);
      if (kind === 'skipped') {
        r.skipped++;
        continue;
      }
      r.total++;
      if (kind === 'done') r.done++;
      else if (kind === 'review') r.inReview++;
      else if (kind === 'blocked') r.blocked++;
      else r.open++;
    }
  }
  const order = componentOrder(components);
  return [...rows.values()].sort((a, b) => (a.id === '' ? 1 : b.id === '' ? -1 : order(a.id, b.id)));
}
