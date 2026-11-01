// The scope bar (T-323): which tasks the Kanban, List and Graph show, read from the Sprint filter. One sprint (the
// active one by default), the backlog, several of them, or all tasks; a project without sprints has no scope.
import { ACTIVE, BACKLOG } from './filters';
import { activeSprint, type Sprint, type SprintModel } from './sprints';
import type { Task } from './types';
import { kindOf, type Vocabulary } from './vocab';

/** A set of tasks by kind of status; `total` leaves the skipped ones out, as every progress figure does (T-162) */
export interface ScopeCounts {
  total: number;
  done: number;
  todo: number;
  active: number;
  review: number;
  blocked: number;
  skipped: number;
}

export function countKinds(tasks: Task[], vocab: Vocabulary): ScopeCounts {
  const counts: ScopeCounts = { total: 0, done: 0, todo: 0, active: 0, review: 0, blocked: 0, skipped: 0 };
  for (const t of tasks) {
    const kind = kindOf(vocab, t.status) ?? 'todo';
    counts[kind]++;
    if (kind !== 'skipped') counts.total++;
  }
  return counts;
}

export type Scope =
  /** No sprints: a plain Kanban, no scope bar */
  | { kind: 'none' }
  /** Nothing selected: every task; the active sprint, or the last closed and the next planned when none is */
  | { kind: 'all'; active?: Sprint; last?: Sprint; next?: Sprint }
  /** One sprint; `follows` when it was chosen as "the active sprint" rather than by name */
  | { kind: 'sprint'; sprint: Sprint; follows: boolean }
  | { kind: 'backlog' }
  /** Several sprints, the backlog among them maybe: their names */
  | { kind: 'several'; names: string[] };

/** The sprint closed last: the latest end, else the last closed one listed */
export function lastClosed(model: SprintModel): Sprint | undefined {
  const closed = model.sprints.filter((s) => s.state === 'closed');
  const ended = closed.filter((s) => s.end).sort((a, b) => Date.parse(a.end!) - Date.parse(b.end!));
  return ended.at(-1) ?? closed.at(-1);
}

/** The scope `selected` (the Sprint filter as it applies, lib/filters.ts sprintSelection) gives */
export function scopeOf(model: SprintModel, selected: ReadonlySet<string>): Scope {
  if (!model.sprints.length) return { kind: 'none' };
  const active = activeSprint(model);
  const find = (value: string) =>
    value === ACTIVE ? active : model.sprints.find((s) => s.id.toLowerCase() === value.toLowerCase());
  if (selected.size === 0) {
    return active
      ? { kind: 'all', active }
      : { kind: 'all', last: lastClosed(model), next: model.sprints.find((s) => s.state === 'planned') };
  }
  if (selected.size === 1) {
    const [value] = selected;
    if (value === BACKLOG) return { kind: 'backlog' };
    const sprint = find(value);
    if (sprint) return { kind: 'sprint', sprint, follows: value === ACTIVE };
  }
  // In the sprints' order, the backlog last, whatever order they were picked in
  const picked = new Set([...selected].map((v) => find(v)?.id ?? v));
  return {
    kind: 'several',
    names: [
      ...model.sprints.filter((s) => picked.has(s.id)).map((s) => s.id),
      ...(selected.has(BACKLOG) ? ['Backlog'] : []),
    ],
  };
}

/**
 * Open tasks in closed sprints (T-323): closing carries every unfinished task on, but a task reopened afterwards (atm
 * reopen), or a merge of a branch that closed the sprint with one that reopened a task in it, leaves one behind. The
 * loop never takes them (it works on the active sprint), so they are shown until they are moved.
 */
export function leftovers(tasks: Task[], model: SprintModel, vocab: Vocabulary): Task[] {
  const closed = new Set(model.sprints.filter((s) => s.state === 'closed').map((s) => s.id));
  return tasks.filter((t) => {
    const sprint = model.sprintOf(t);
    const kind = kindOf(vocab, t.status);
    return sprint !== undefined && closed.has(sprint) && kind !== 'done' && kind !== 'skipped';
  });
}

/** The scope's own tasks, before the other filters and the search */
export function scopeTasks(tasks: Task[], model: SprintModel, selected: ReadonlySet<string>): Task[] {
  if (selected.size === 0) return tasks;
  const active = activeSprint(model)?.id;
  const ids = new Set([...selected].map((v) => (v === ACTIVE ? active : v)).map((v) => v?.toLowerCase()));
  return tasks.filter((t) => ids.has(model.sprintOf(t)?.toLowerCase() ?? BACKLOG));
}
