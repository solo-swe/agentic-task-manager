// What a task's card shows of its state (ATM-683): the same for the Task List's card and the Kanban's, worked out
// once. Its sprint and that sprint's colour, its type's style, whether it is stranded in a closed sprint, its kind,
// the claim on it, who it waits on, and the code review it waits on.
import type { Claim } from './claims';
import type { SprintModel } from './sprints';
import { phaseColor, typeStyle } from './theme';
import type { Task } from './types';
import { taskKind } from './vocab';
import { waitLine } from './waiting';

/** What the card state reads of the dashboard */
export interface CardContext {
  sprints: SprintModel;
  /** Open tasks in closed sprints (reopened after it closed, T-323) */
  leftovers: readonly Task[];
  claims: ReadonlyMap<string, Claim>;
  /** When the data was loaded: what "waiting 12 m" counts from */
  loadedAt: number;
}

export interface CardState {
  /** Its sprint's ID; undefined: the backlog */
  sprint: string | undefined;
  /** Its sprint's colour, by the sprint's place in the project's sprints; the backlog muted */
  sprintColor: string;
  typeStyle: ReturnType<typeof typeStyle>;
  /** Open in a closed sprint: the badge says so */
  stranded: boolean;
  kind: ReturnType<typeof taskKind>;
  /** The claim on it, while it is active */
  claim: Claim | undefined;
  /** Who it waits on, as atm says it (ATM-717): one line, agreeing with its review's chip */
  wait: ReturnType<typeof waitLine>;
  /** Its code review, while it is in review (ATM-619): the one it waits on, as atm says it */
  codeReview: NonNullable<Task['derived']>['review'] | null;
}

export function taskCardState(task: Task, context: CardContext): CardState {
  const sprint = context.sprints.sprintOf(task);
  const sprintIndex = context.sprints.sprints.findIndex((s) => s.id === sprint);
  const kind = taskKind(task);
  return {
    sprint,
    sprintColor: sprintIndex < 0 ? 'var(--text-muted)' : phaseColor(sprintIndex),
    typeStyle: typeStyle(task.type),
    stranded: context.leftovers.includes(task),
    kind,
    claim: kind === 'active' ? context.claims.get(task.id) : undefined,
    wait: waitLine(task, context.loadedAt),
    codeReview: kind === 'review' ? (task.derived?.review ?? null) : null,
  };
}
