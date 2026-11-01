// A project's components (meta.components, written by atm components): the parts of the project with the globs of
// their files. Tasks name them by ID; the dashboard shows their labels (T-323).
import type { Task, TasksFile } from './types';

export interface Component {
  id: string;
  label: string;
  paths: string[];
  review?: string;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;

/** The project's components in their order, read leniently: entries without an ID are left out */
export function projectComponents(meta: TasksFile['meta']): Component[] {
  if (!isRecord(meta) || !Array.isArray(meta.components)) return [];
  return meta.components.filter(isRecord).flatMap((c) => {
    if (typeof c.id !== 'string' || !c.id) return [];
    return [
      {
        id: c.id,
        label: typeof c.label === 'string' && c.label ? c.label : c.id,
        paths: Array.isArray(c.paths) ? c.paths.filter((p): p is string => typeof p === 'string') : [],
        ...(typeof c.review === 'string' ? { review: c.review } : {}),
      },
    ];
  });
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
