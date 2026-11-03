import { BACKLOG } from './filters';
import { NO_SPRINTS, type SprintModel } from './sprints';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, priorityOf, priorityRank, type StatusKind, type Vocabulary } from './vocab';

/** A column per status of the vocabulary, plus "other" for statuses nobody defines. */
export type ColumnId = string;
export const OTHER_COLUMN = 'other';

export type Readiness = { ready: true } | { ready: false; waitingOn: string[] };

export interface KanbanCard {
  task: Task;
  /** Only for todo-kind tasks */
  readiness?: Readiness;
  /** The task /atm:loop-tasks would pick next (atm's answer, BoardOptions.nextUp) */
  nextUp: boolean;
}

export interface KanbanColumn {
  id: ColumnId;
  label: string;
  /** undefined for the Other column */
  kind: StatusKind | undefined;
  cards: KanbanCard[];
}

export interface BoardOptions {
  showSkipped: boolean;
  /** The dashboard's status filter (empty means all statuses): only shows a skipped-kind column it names */
  statusFilter: ReadonlySet<string>;
  /** The project's vocabularies; the defaults when absent */
  vocab?: Vocabulary;
  /** The project's sprints, for the lanes */
  sprints?: SprintModel;
  /** The task the loop takes next, as atm says it: the project summary's `next` from api/projects (T-321) */
  nextUp?: string | null;
}

const byId = (a: Task, b: Task) => a.id.localeCompare(b.id, undefined, { numeric: true });
const byPhaseThenId = (a: Task, b: Task) => a.phase - b.phase || byId(a, b);
const time = (iso: string | undefined) => (iso ? Date.parse(iso) || 0 : 0);
const newestFirst = (when: (t: Task) => string | undefined) => (a: Task, b: Task) =>
  time(when(b)) - time(when(a)) || byId(a, b);

/**
 * Whether a to-do task can start, and what it waits for: as atm says it (the task's `derived` values from the API,
 * T-366), which counts an archived dependency as done. Every task comes with them, a linked worktree's copy too
 * (worked out in its worktree, T-387); one without them isn't called ready.
 */
export function readiness(task: Task): Readiness {
  const derived = task.derived;
  return derived?.ready === true ? { ready: true } : { ready: false, waitingOn: derived?.waiting_for ?? [] };
}

/** The To Do column's order after ready-first: the most urgent priority, then lowest ID. Only for reading the column;
 * which task the loop takes next is atm's to say (`nextUp`, T-321) */
const byPriorityThenId = (vocab: Vocabulary) => (a: Task, b: Task) =>
  priorityRank(vocab, priorityOf(vocab, a)) - priorityRank(vocab, priorityOf(vocab, b)) || byId(a, b);

const ORDER: Partial<Record<StatusKind, (a: Task, b: Task) => number>> = {
  // todo: ready-first is applied on top, see buildBoard
  active: newestFirst((t) => t.started_at ?? t.updated_at),
  done: newestFirst((t) => t.completed_at ?? t.updated_at),
};

/**
 * Columns for the visible (filtered) tasks, in the vocabulary's status order; readiness is atm's (each task's
 * `derived`), next-up atm's too (`nextUp`). `columns` forces a column set (used to keep swimlanes aligned).
 */
export function buildBoard(
  tasks: Task[],
  allTasks: Task[],
  { showSkipped, statusFilter, vocab = DEFAULT_VOCABULARY, nextUp = null }: BoardOptions,
  columns?: ColumnId[],
): KanbanColumn[] {
  const known = new Map(vocab.statuses.map((s) => [s.id, s]));
  const columnOf = (status: string) => (known.has(status) ? status : OTHER_COLUMN);
  const grouped = new Map<ColumnId, Task[]>();
  for (const t of tasks) grouped.set(columnOf(t.status), [...(grouped.get(columnOf(t.status)) ?? []), t]);

  // The columns stay put whatever the filters (a status filter empties columns, it doesn't remove them); a
  // skipped-kind column shows behind its toggle, or when the status filter asks for it
  const wanted = (id: ColumnId): boolean => {
    if (id === OTHER_COLUMN) return (grouped.get(OTHER_COLUMN) ?? []).length > 0;
    return known.get(id)!.kind !== 'skipped' || showSkipped || statusFilter.has(id);
  };

  const ids = columns ?? [...vocab.statuses.map((s) => s.id), OTHER_COLUMN].filter(wanted);
  return ids.map((id) => {
    const def = known.get(id);
    const kind = def?.kind;
    const cards = (grouped.get(id) ?? []).map((task) => ({
      task,
      ...(kind === 'todo' ? { readiness: readiness(task) } : {}),
      nextUp: nextUp !== null && task.id === nextUp,
    }));
    // To Do: ready first (see below), then by priority and ID
    const order = kind === 'todo' ? byPriorityThenId(vocab) : ((kind && ORDER[kind]) ?? byPhaseThenId);
    cards.sort((a, b) => {
      if (kind === 'todo') {
        const ready = Number(b.readiness!.ready) - Number(a.readiness!.ready);
        if (ready) return ready;
      }
      return order(a.task, b.task);
    });
    return { id, label: def?.label ?? 'Other', kind, cards };
  });
}

export interface KanbanLane {
  /** The sprint's ID, or BACKLOG */
  key: string;
  /** Its place in the project's sprints (for its colour); -1 for the backlog */
  index: number;
  done: number;
  total: number;
  columns: KanbanColumn[];
}

/** One swimlane per sprint with tasks, in the project's order, the backlog last (FR-3.7, SR-7.3); all with the
 * same columns as the ungrouped board so they line up. */
export function buildLanes(tasks: Task[], allTasks: Task[], options: BoardOptions): KanbanLane[] {
  const vocab = options.vocab ?? DEFAULT_VOCABULARY;
  const model = options.sprints ?? NO_SPRINTS;
  const columns = buildBoard(tasks, allTasks, options).map((c) => c.id);
  const keyOf = (t: Task) => model.sprintOf(t) ?? BACKLOG;
  const present = new Set(tasks.map(keyOf));
  const keys = [...model.sprints.map((s) => s.id), BACKLOG].filter((key) => present.has(key));
  return keys.map((key) => {
    const own = tasks.filter((t) => keyOf(t) === key);
    return {
      key,
      index: model.sprints.findIndex((s) => s.id === key),
      done: own.filter((t) => kindOf(vocab, t.status) === 'done').length,
      total: own.length,
      columns: buildBoard(own, allTasks, options, columns),
    };
  });
}
