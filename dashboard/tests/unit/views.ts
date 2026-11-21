// A task as atm's tasks route serves it, for unit tests that stub the API: its fields, and what atm derives for it,
// as the test says atm answers (atm-lib's rules have their own tests; ATM-717). Only the values a test gives are its
// answers: the rest are empty, so a component that reads one the test didn't give shows nothing for it.
import type { TaskDerived } from '$generated/types';
import type { Task } from '../../src/lib/types';
import progress from '../fixtures/progress.json';

export function viewed(task: Task, derived: Partial<TaskDerived>): Task {
  return {
    ...task,
    derived: { kind: '', ready: false, waiting_for: [], priority: '', archived: false, ...task.derived, ...derived },
  };
}

/** The tasks of `fixtures/progress.json` (its numbers: `progress.md`) as atm serves them: agent work is left on all of
 * them but the finished ones and T-010, blocked */
const PROGRESS_NO_AGENT_WORK = new Set([
  'T-001',
  'T-002',
  'T-003',
  'T-004',
  'T-005',
  'T-006',
  'T-007',
  'T-008',
  'T-010',
  'T-016',
]);
export const progressTasks: Task[] = (progress.tasks as Task[]).map((x) =>
  viewed(x, { agent_work: !PROGRESS_NO_AGENT_WORK.has(x.id) }),
);

/** A task, with atm's answer of whether agent work is left on it */
export const taskWith = (id: string, status: string, extra: Partial<Task> = {}, agentWork = true): Task =>
  viewed({ id, name: id, phase: 0, type: 'feature', description: '', status, ...extra }, { agent_work: agentWork });
