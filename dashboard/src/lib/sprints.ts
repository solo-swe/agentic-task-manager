// Sprints (docs/sprints-and-releases-design.md): meta.sprints, the task's sprint and its history. A project not yet
// migrated to sprints (tasks with phases, no meta.sprints) is read as read-only sprints named after its phases, the
// names atm migrate gives them, so a dashboard serving several projects shows them all the same way (SR-7.8).
import type { Task, TasksFile } from './types';
import { kindOf, type Vocabulary } from './vocab';

export type SprintState = 'planned' | 'active' | 'closed';

export interface Sprint {
  /** Its name, which is its ID */
  id: string;
  state: SprintState;
  goal?: string;
  release?: string;
  /** ISO 8601 UTC, set when it became active */
  start?: string;
  /** ISO 8601 UTC, set when it closed */
  end?: string;
  /** The planned end, YYYY-MM-DD */
  due?: string;
  report?: { done?: string[]; carried?: string[]; added?: string[]; to?: string };
}

export interface SprintModel {
  sprints: Sprint[];
  /** Read from phases: the project hasn't been migrated, and its sprints can't be changed */
  derived: boolean;
  /** The sprint a task is in (its ID), or undefined: the backlog */
  sprintOf(task: Task): string | undefined;
}

const STATES: readonly SprintState[] = ['planned', 'active', 'closed'];
const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const text = (v: unknown): string | undefined => (typeof v === 'string' && v !== '' ? v : undefined);

/** meta.sprints, read leniently: entries without a name are left out, an unknown state counts as planned. */
function readSprints(value: unknown[]): Sprint[] {
  const seen = new Set<string>();
  const sprints: Sprint[] = [];
  for (const item of value) {
    if (!isRecord(item) || !text(item.id) || seen.has((item.id as string).toLowerCase())) continue;
    seen.add((item.id as string).toLowerCase());
    const sprint: Sprint = {
      id: item.id as string,
      state: STATES.includes(item.state as SprintState) ? (item.state as SprintState) : 'planned',
    };
    for (const key of ['goal', 'release', 'start', 'end', 'due'] as const)
      if (text(item[key])) sprint[key] = text(item[key]);
    if (isRecord(item.report)) sprint.report = item.report as Sprint['report'];
    sprints.push(sprint);
  }
  return sprints;
}

/** The sprints atm migrate makes from phases: one per phase with tasks or a name, named after it ("Phase n" when
 * unnamed, " (phase n)" added to a name already taken); closed when all its tasks are finished, else planned. */
function sprintsFromPhases(tasks: Task[], vocab: Vocabulary) {
  const phases = new Set<number>(vocab.phaseNames.keys());
  for (const t of tasks) if (typeof t.phase === 'number') phases.add(t.phase);
  const names = new Map<number, string>();
  const taken = new Set<string>();
  for (const phase of [...phases].sort((a, b) => a - b)) {
    let name = vocab.phaseNames.get(phase) ?? `Phase ${phase}`;
    if (taken.has(name.toLowerCase())) name = `${name} (phase ${phase})`;
    taken.add(name.toLowerCase());
    names.set(phase, name);
  }
  const sprints: Sprint[] = [...names].map(([phase, id]) => {
    const own = tasks.filter((t) => t.phase === phase);
    const finished = own.length > 0 && own.every((t) => ['done', 'skipped'].includes(kindOf(vocab, t.status) ?? ''));
    return { id, state: finished ? 'closed' : 'planned' };
  });
  return { sprints, sprintOf: (t: Task) => (typeof t.phase === 'number' ? names.get(t.phase) : undefined) };
}

export function sprintModel(meta: TasksFile['meta'], tasks: Task[], vocab: Vocabulary): SprintModel {
  if (isRecord(meta) && Array.isArray(meta.sprints)) {
    const sprints = readSprints(meta.sprints);
    const byName = new Map(sprints.map((s) => [s.id.toLowerCase(), s.id]));
    return {
      sprints,
      derived: false,
      sprintOf: (t) => (typeof t.sprint === 'string' ? byName.get(t.sprint.toLowerCase()) : undefined),
    };
  }
  if (tasks.some((t) => typeof t.phase === 'number') || vocab.phaseNames.size) {
    return { ...sprintsFromPhases(tasks, vocab), derived: true };
  }
  return { sprints: [], derived: false, sprintOf: () => undefined };
}

/** A project without sprints */
export const NO_SPRINTS: SprintModel = { sprints: [], derived: false, sprintOf: () => undefined };

export function activeSprint(model: SprintModel): Sprint | undefined {
  return model.sprints.find((s) => s.state === 'active');
}

export function sprintTasks(model: SprintModel, tasks: Task[], sprint: string): Task[] {
  return tasks.filter((t) => model.sprintOf(t) === sprint);
}

/** The tasks with no sprint */
export function backlog(model: SprintModel, tasks: Task[]): Task[] {
  return tasks.filter((t) => model.sprintOf(t) === undefined);
}

/** A task's release: its own, else its sprint's (SR-2.6) */
export function effectiveRelease(model: SprintModel, task: Task): string | undefined {
  if (task.release) return task.release;
  const sprint = model.sprintOf(task);
  return sprint ? model.sprints.find((s) => s.id === sprint)?.release : undefined;
}

/** The task's history entry for `sprint` (the last one, if it was there more than once) */
function entryFor(task: Task, sprint: string) {
  const history = Array.isArray(task.sprint_history) ? task.sprint_history : [];
  return [...history].reverse().find((e) => e?.sprint?.toLowerCase() === sprint.toLowerCase());
}

/** Pulled into the sprint after it started (its history entry for the sprint begins after the sprint's start) */
export function addedMidSprint(task: Task, sprint: Sprint): boolean {
  const entry = entryFor(task, sprint.id);
  return !!(entry && sprint.start && Date.parse(entry.from) > Date.parse(sprint.start));
}

const DAY = 86_400_000;

/** Days from today (local) to a YYYY-MM-DD date: 0 today, negative once it has passed; undefined for a bad date */
export function daysUntil(date: string, now: number): number | undefined {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!m) return undefined;
  const today = new Date(now);
  const due = Date.UTC(+m[1], +m[2] - 1, +m[3]);
  return Math.round((due - Date.UTC(today.getFullYear(), today.getMonth(), today.getDate())) / DAY);
}

/** "due in 3 days", "due tomorrow", "due today", "1 day overdue", "4 days overdue" */
export function dueText(days: number): string {
  if (days > 1) return `due in ${days} days`;
  if (days === 1) return 'due tomorrow';
  if (days === 0) return 'due today';
  return days === -1 ? '1 day overdue' : `${-days} days overdue`;
}

export interface SprintSummary {
  sprint: Sprint;
  /** Done-kind tasks */
  done: number;
  /** Its tasks but the skipped ones */
  total: number;
  /** Pulled in after the sprint started */
  added: number;
  /** Carried in from an earlier sprint */
  carried: number;
  /** Days to its due date (negative: overdue); undefined without one, or once it is closed */
  dueIn?: number;
  /** Done batch tasks waiting for the end-of-sprint review (SR-4.4) */
  reviewPending: number;
}

/** The figures the scope bar shows for one sprint (SR-7.2, T-323), over `tasks` (the history, archived tasks too once loaded) */
export function sprintSummary(
  model: SprintModel,
  tasks: Task[],
  sprint: Sprint,
  vocab: Vocabulary,
  now: number,
): SprintSummary {
  const own = sprintTasks(model, tasks, sprint.id);
  const kinds = own.map((t) => kindOf(vocab, t.status));
  return {
    sprint,
    done: kinds.filter((k) => k === 'done').length,
    total: kinds.filter((k) => k !== 'skipped').length,
    added: own.filter((t) => addedMidSprint(t, sprint)).length,
    carried: own.filter((t) => carriedCount(t, sprint.id) > 0).length,
    dueIn: sprint.due && sprint.state !== 'closed' ? daysUntil(sprint.due, now) : undefined,
    reviewPending: own.filter((t) => t.sprint_review === 'pending').length,
  };
}

/** How many sprints the task was carried through before this one: the earlier entries for other sprints (SR-4.5) */
export function carriedCount(task: Task, sprint: string): number {
  const history = Array.isArray(task.sprint_history) ? task.sprint_history : [];
  const entry = entryFor(task, sprint);
  if (!entry) return 0;
  return new Set(
    history
      .filter((e) => e?.from < entry.from && e.sprint?.toLowerCase() !== sprint.toLowerCase())
      .map((e) => e.sprint),
  ).size;
}
