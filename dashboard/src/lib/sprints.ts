// Sprints (docs/sprints-and-releases-design.md), as atm serves them with the tasks (ATM-720): each sprint in the
// project's order with its figures (done, total and percent by one rule, added, carried, the sprint review pending,
// the days to its due date by the project's day), the backlog as `atm sprint list` counts it, and each task's sprint
// (`derived.sprint`). An unmigrated project's phases come as the sprints `atm db migrate` would make (`fromPhases`,
// read-only). The dashboard shows them; it counts none of it.
import type { ServedSprint, ServedSprints } from '$generated/types';
import type { Task } from './types';

export type SprintState = ServedSprint['state'];

/** A sprint as atm serves it: its stored fields and its figures */
export type Sprint = ServedSprint;

export interface SprintModel {
  sprints: Sprint[];
  /** Read from phases: the project hasn't been migrated, and its sprints can't be changed */
  derived: boolean;
  /** Unfinished tasks in no sprint, as `atm sprint list` counts them */
  backlog: number;
  /** The sprint a task is in (its ID), or undefined: the backlog */
  sprintOf(task: Task): string | undefined;
}

/** A project without sprints */
export const NO_SPRINTS: SprintModel = { sprints: [], derived: false, backlog: 0, sprintOf: () => undefined };

/** The sprints atm serves with the tasks (the response's `sprints`), as the dashboard uses them; none before atm has
 * served any */
export function fromServedSprints(served: ServedSprints | undefined): SprintModel {
  if (!served) return NO_SPRINTS;
  return {
    sprints: served.sprints,
    derived: served.from_phases,
    backlog: served.backlog,
    sprintOf: (t) => t.derived?.sprint,
  };
}

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
  // As atm says it (T-371), for a task that comes with atm's values; judged here only for the others (archived ones,
  // a linked worktree's copy)
  if (typeof task.derived?.ready === 'boolean') return task.derived.effective_release;
  if (task.release) return task.release;
  const sprint = model.sprintOf(task);
  return sprint ? model.sprints.find((s) => s.id === sprint)?.release : undefined;
}

/** atm's days to a due date (`days_left`) in words: "due in 3 days", "due tomorrow", "due today", "1 day overdue",
 * "4 days overdue" */
export function dueText(days: number): string {
  if (days > 1) return `due in ${days} days`;
  if (days === 1) return 'due tomorrow';
  if (days === 0) return 'due today';
  return days === -1 ? '1 day overdue' : `${-days} days overdue`;
}
