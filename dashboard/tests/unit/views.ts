// A task as atm's tasks route serves it, for unit tests that stub the API: its fields, and what atm derives for it,
// as the test says atm answers (atm-lib's rules have their own tests; ATM-717). Only the values a test gives are its
// answers: the rest are empty, so a component that reads one the test didn't give shows nothing for it.
import type { TaskDerived } from '$generated/types';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';

export function viewed(task: Task, derived: Partial<TaskDerived>): Task {
  return {
    ...task,
    derived: { kind: '', ready: false, waiting_for: [], priority: '', archived: false, ...task.derived, ...derived },
  };
}

/** A project's vocabulary as atm serves it (ATM-718): the defaults, with the lists the test gives */
export const vocabulary = (lists: Partial<Vocabulary>): Vocabulary => ({ ...DEFAULT_VOCABULARY, ...lists });

/** `tasks` with the kind atm gives each (`derived.kind`): its status's in `vocab` (the defaults unless given); none
 * for a status the vocabulary doesn't define */
export function withKinds<T extends Task>(tasks: readonly T[], vocab: Vocabulary = DEFAULT_VOCABULARY): T[] {
  return tasks.map((t) => {
    const kind = vocab.statuses.find((s) => s.id === t.status)?.kind;
    // Only the kind: a test that gives atm's other answers gives them itself
    return kind ? { ...t, derived: { ...t.derived, kind } as Task['derived'] } : t;
  }) as T[];
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
export const progressTasks: Task[] = withKinds(
  (progress.tasks as Task[]).map((x) => viewed(x, { agent_work: !PROGRESS_NO_AGENT_WORK.has(x.id) })),
);

/** A task, with atm's answer of whether agent work is left on it */
export const taskWith = (id: string, status: string, extra: Partial<Task> = {}, agentWork = true): Task =>
  withKinds([
    viewed({ id, name: id, phase: 0, type: 'feature', description: '', status, ...extra }, { agent_work: agentWork }),
  ])[0];
