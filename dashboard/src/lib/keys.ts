// Task keys (ADR-007, T-546): a project in the keys format stores each task by its UUID (`id`) and shows it by its key,
// the project's short name and the task's number (`ATM-527`, which the API's task views carry as `key`). A project
// from before task keys has only IDs (`T-001`), shown and matched as they always were. Everything the dashboard shows,
// orders or looks up a task by goes through here.

import type { ForeignTask } from '$generated/types';
import { formatAddress } from './address';
import type { Task } from './types';
import { KINDS, type StatusKind } from './vocab';

/** The fields a task is shown and ordered by */
export type Keyed = Pick<Task, 'id' | 'key' | 'number'>;

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

/** The keys a task had in this project before its number moved (ADR-009: two branches gave the same number, a merge
 * moved it): its aliases under the project's current key, as the details show them ("was ATM-538"). Aliases of other
 * prefixes (an ID from before task keys, an earlier project key) aren't moves */
export function movedFrom(task: Pick<Task, 'aliases'>, projectKey: string | undefined): string[] {
  if (!projectKey || !Array.isArray(task.aliases)) return [];
  const own = new RegExp(`^${projectKey.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}-\\d+$`, 'i');
  return task.aliases.map(String).filter((a) => own.test(a));
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
  // `~ATM-12`, as an older atm printed a key before ADR-009, is `ATM-12`
  const text = ref.trim().replace(/^~(?=.)/, '');
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

/** How a dependency or related task shows: its label, the task when it is known, and for another project's task, that
 * project and a tooltip */
export interface RefView {
  label: string;
  /** The task, when it is loaded (this project's, or another's from the overview) */
  task?: Task;
  /** Another project's task: the project's id */
  project?: string;
  /** What pointing at it says, for another project's task */
  tip?: string;
  /** Another project's task as the server found it there (derived.foreign): its name, status and status's kind */
  found?: { name: string; status: string | null; kind?: StatusKind };
  /** Where another project's task opens: its project's page with it open */
  address?: string;
  /** Why another project's task can't be shown: its project isn't registered, or has no such task */
  missing?: string;
}

/** The dashboard's address of another project's task: its project in `workspace` when that workspace lists it, else
 * in the first one that does, with the task open by `task` (its key, else its UUID); null when no workspace lists it */
export function foreignAddress(found: ForeignTask, task: string, workspace: string): string | null {
  const places = found.places ?? [];
  const place = places.find((p) => p.workspace === workspace) ?? places[0];
  if (!place) return null;
  const params = new URLSearchParams({ project: place.project, task });
  if (place.workspace !== 'default') params.set('workspace', place.workspace);
  return formatAddress(params);
}

/** A status kind the server named, if it is one */
function asKind(kind: string | undefined): StatusKind | undefined {
  return (KINDS as readonly string[]).includes(kind ?? '') ? (kind as StatusKind) : undefined;
}

/** How another project's task (`id`, its UUID, in project `project`) shows from what the server found of it there */
function foundView(found: ForeignTask, project: string, id: string, workspace: string): RefView {
  const name = found.project_name || project;
  if (!found.found) {
    const missing =
      found.reason === 'missing' ? `no such task in ${name}` : "its project isn't in your workspaces (or is gone)";
    return {
      label: `${name}:${id.slice(0, 8)}`,
      project,
      missing,
      tip: `A task of ${name}: ${missing}`,
    };
  }
  // A key another registered project uses too would be ambiguous: the UUID's first 8 characters instead (ADR-007 §8)
  const key = typeof found.key === 'string' && found.key !== '' && found.ambiguous !== true ? found.key : null;
  const label = key ?? (found.ambiguous ? id.slice(0, 8) : `${name}:${id.slice(0, 8)}`);
  const address = foreignAddress(found, key ?? id, workspace);
  return {
    label,
    project,
    tip: `${found.name ?? ''} (a task of ${name})`,
    found: { name: found.name ?? '', status: found.status ?? null, kind: asKind(found.kind) },
    ...(address ? { address } : {}),
  };
}

/** How reference `ref` (a dependency, a related task) shows: this project's task by its key (`own` finds it by ID);
 * another project's (`<project id>:<UUID>`) as the server found it there (`foreign`, from the task views: its key,
 * status and where it opens, or why it can't be found); without that, by its key when that project is listed and its
 * tasks are loaded, otherwise in short (its project's name and the UUID's first 8 characters), saying it isn't loaded
 * here. */
export function refView(
  ref: string,
  own: (id: string) => Task | undefined,
  others: readonly OtherProject[] = [],
  foreign?: (ref: string) => ForeignTask | undefined,
  workspace = 'default',
): RefView {
  const { project, id } = parseRef(ref);
  if (project === null) {
    const task = own(ref);
    return task ? { label: taskLabel(task), task } : { label: shortId(ref) };
  }
  const found = foreign?.(ref);
  if (found) return foundView(found, project, id, workspace);
  const listed = others.find((p) => p.id.toLowerCase() === project);
  const name = listed?.name || project;
  const task = listed?.tasks?.find((t) => t.id.toLowerCase() === id);
  if (task)
    return {
      label: taskLabel(task),
      task,
      project,
      tip: `${task.name} (a task of ${name})`,
    };
  return {
    label: `${name}:${id.slice(0, 8)}`,
    project,
    tip: `A task of another project (${name}) that isn't loaded here`,
  };
}
