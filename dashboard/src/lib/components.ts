// A project's components (meta.components, written by atm components): the parts of the project with the globs of
// their files, as atm serves them with the vocabulary (ATM-718). Tasks name them by ID; the dashboard shows their
// labels (T-323).
import type { Task } from './types';

export interface Component {
  id: string;
  label: string;
  paths: string[];
  review?: string;
}

/** A component's label by its ID; an ID the project doesn't define shows as itself */
export function componentLabel(components: readonly Component[], id: string): string {
  return components.find((c) => c.id === id)?.label ?? id;
}

/** Compares component IDs: in the project's order, IDs it doesn't define after them, alphabetically */
export function componentOrder(components: readonly Component[]): (a: string, b: string) => number {
  const rank = (id: string) => {
    const i = components.findIndex((c) => c.id === id);
    return i < 0 ? components.length : i;
  };
  return (a, b) => rank(a) - rank(b) || a.localeCompare(b);
}

/** A task's component IDs, in the project's order */
export function taskComponents(task: Task, components: readonly Component[]): string[] {
  const ids = Array.isArray(task.components) ? task.components.filter((c): c is string => typeof c === 'string') : [];
  return [...ids].sort(componentOrder(components));
}
