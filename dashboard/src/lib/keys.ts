// Task keys (ADR-007, T-546): a project in the keys format stores each task by its UUID (`id`) and shows it by its key,
// the project's short name and the task's number (`ATM-527`, which the API's task views carry as `key`). A project
// from before task keys has only IDs (`T-001`), shown and matched as they always were. Everything the dashboard shows,
// orders or looks up a task by goes through here.

import type { Task } from './types';

/** What a provisional key's mark says when pointed at (ADR-007 §3) */
export const PROVISIONAL_TIP = 'Provisional until merged into main: the number may change if another branch took it';

/** The fields a task is shown and ordered by */
export type Keyed = Pick<Task, 'id' | 'key' | 'number' | 'key_final'>;

/** The project fields a key is resolved against: its short name and the ones it had */
export interface KeyMeta {
  key?: string;
  key_history?: string[];
}

/** How a task reads to people: its key, else (before task keys) its ID */
export function taskLabel(task: Pick<Task, 'id' | 'key'>): string {
  return typeof task.key === 'string' && task.key ? task.key : task.id;
}

/** A task's key as a field to spread into what is worked out from it (`{ id, ...keyOf(t) }`): none before task keys,
 * so such an entry stays as it was */
export function keyOf(task: Pick<Task, 'key'>): { key?: string } {
  return typeof task.key === 'string' && task.key ? { key: task.key } : {};
}

/** Whether a task's key may still change (ADR-007 §3): it has a key, not yet final. An ID never is */
export function isProvisional(task: Pick<Task, 'key' | 'key_final'>): boolean {
  return typeof task.key === 'string' && task.key !== '' && task.key_final !== true;
}

/** The order of tasks by their place in the project: by number when both have one (ATM-2 before ATM-10), else by ID
 * as before task keys (T-2 before T-10) */
export function compareTasks(a: Keyed, b: Keyed): number {
  if (typeof a.number === 'number' && typeof b.number === 'number') return a.number - b.number;
  return a.id.localeCompare(b.id, undefined, { numeric: true });
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
/** Another project's task: `<project id>:<UUID>` (ADR-007 §7) */
const FOREIGN = /^([0-9a-f]{8}):([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

export function isUuid(text: string): boolean {
  return UUID.test(text);
}

/** A reference's parts: another project's (`3f2a9c1e:<UUID>`) names that project, this project's has none */
export function parseRef(ref: string): { project: string | null; id: string } {
  const m = FOREIGN.exec(ref);
  return m ? { project: m[1].toLowerCase(), id: m[2].toLowerCase() } : { project: null, id: ref };
}

/** An ID short enough to read where its task isn't known: a UUID's first 8 characters, an ID as it is */
export function shortId(id: string): string {
  return isUuid(id) ? id.slice(0, 8) : id;
}

/** The task a reference people type or link names (ADR-007 §5), among `tasks` (the working ones first, then the
 * archive's): its ID (a UUID in any case, or an ID from before task keys exactly); its current key, any case; a key
 * with one of the project's earlier prefixes (`meta.key_history`) and its number; or one of its own aliases. */
export function findTask(
  ref: string,
  tasks: readonly ReadonlyMap<string, Task>[],
  meta: KeyMeta | undefined,
): Task | undefined {
  const text = ref.trim();
  if (!text) return undefined;
  const exact = isUuid(text) ? text.toLowerCase() : text;
  for (const map of tasks) {
    const task = map.get(exact);
    if (task) return task;
  }
  if (typeof meta?.key !== 'string' || !meta.key) return undefined;
  const all = tasks.flatMap((map) => [...map.values()]);
  const m = /^(.+)-(\d+)$/.exec(text);
  if (m) {
    const prefix = m[1].toLowerCase();
    const number = Number(m[2]);
    const prefixes = [meta.key, ...(Array.isArray(meta.key_history) ? meta.key_history : [])].map((p) =>
      String(p).toLowerCase(),
    );
    if (prefixes.includes(prefix)) {
      const found = all.find((t) => t.number === number);
      if (found) return found;
    }
  }
  const lower = text.toLowerCase();
  return all.find((t) => Array.isArray(t.aliases) && t.aliases.some((a) => String(a).toLowerCase() === lower));
}

/** Another project the dashboard lists, and its tasks when they are loaded (the overview's) */
export interface OtherProject {
  id: string;
  name?: string;
  tasks?: readonly Task[];
}

/** How a dependency or related task shows: its label, whether it is provisional, the task when it is known, and for
 * another project's task, that project and a tooltip */
export interface RefView {
  label: string;
  provisional: boolean;
  /** The task, when it is loaded (this project's, or another's from the overview) */
  task?: Task;
  /** Another project's task: the project's id */
  project?: string;
  /** What pointing at it says, for another project's task */
  tip?: string;
}

/** How reference `ref` (a dependency, a related task) shows: this project's task by its key (`own` finds it by ID);
 * another project's (`<project id>:<UUID>`) by its key when that project is listed and its tasks are loaded,
 * otherwise in short (its project's name and the UUID's first 8 characters), saying it isn't loaded here. */
export function refView(
  ref: string,
  own: (id: string) => Task | undefined,
  others: readonly OtherProject[] = [],
): RefView {
  const { project, id } = parseRef(ref);
  if (project === null) {
    const task = own(ref);
    return task
      ? { label: taskLabel(task), provisional: isProvisional(task), task }
      : { label: shortId(ref), provisional: false };
  }
  const listed = others.find((p) => p.id.toLowerCase() === project);
  const name = listed?.name || project;
  const task = listed?.tasks?.find((t) => t.id.toLowerCase() === id);
  if (task)
    return {
      label: taskLabel(task),
      provisional: isProvisional(task),
      task,
      project,
      tip: `${task.name} (a task of ${name})`,
    };
  return {
    label: `${name}:${id.slice(0, 8)}`,
    provisional: false,
    project,
    tip: `A task of another project (${name}) that isn't loaded here`,
  };
}
