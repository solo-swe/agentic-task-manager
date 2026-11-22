// A task as atm's tasks route serves it, for unit tests that stub the API: its fields, and what atm derives for it,
// as the test says atm answers (atm-lib's rules have their own tests; ATM-717). Only the values a test gives are its
// answers: the rest are empty, so a component that reads one the test didn't give shows nothing for it.
import type { ServedSprint, TaskDerived } from '$generated/types';
import type { Sprint, SprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import progress from '../fixtures/progress.json';

const FIGURES = { done: 0, total: 0, percent: 100, added: 0, carried: 0, review_pending: 0 };

/** A sprint as atm serves it, with the figures the test gives (the rest zero) */
export const sprint = (fields: Partial<ServedSprint> & Pick<ServedSprint, 'id'>): Sprint => ({
  state: 'planned',
  ...FIGURES,
  ...fields,
});

/**
 * A fake of the sprints atm serves with the tasks (ATM-720), for unit tests on fixtures: `meta.sprints` with their
 * stored fields, or a fixture's phases as sprints ("Phase n", or the phase's name in `vocab`; closed once all its
 * tasks are finished), each task in its sprint. Their figures are zero but `done`. atm's own rules (the phases'
 * names, every figure) are atm-lib's tests; these only give the views something to group by.
 */
export function sprintModel(
  meta: TasksFile['meta'],
  tasks: readonly Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
): SprintModel {
  const kind = (t: Task) => vocab.statuses.find((s) => s.id === t.status)?.kind;
  const listed = (meta as { sprints?: Partial<ServedSprint>[] } | undefined)?.sprints;
  if (Array.isArray(listed)) {
    const sprints = listed.filter((s) => typeof s?.id === 'string' && s.id).map((s) => sprint({ ...s, id: s.id! }));
    const byName = new Map(sprints.map((s) => [s.id.toLowerCase(), s.id]));
    const sprintOf = (t: Task) => (typeof t.sprint === 'string' ? byName.get(t.sprint.toLowerCase()) : undefined);
    for (const s of sprints) s.done = tasks.filter((t) => sprintOf(t) === s.id && kind(t) === 'done').length;
    const backlog = tasks.filter((t) => !sprintOf(t) && !['done', 'skipped'].includes(kind(t) ?? '')).length;
    return { sprints, derived: false, backlog, sprintOf };
  }
  const phases = [
    ...new Set([...vocab.phaseNames.keys(), ...tasks.flatMap((t) => (typeof t.phase === 'number' ? [t.phase] : []))]),
  ];
  if (!phases.length) return { sprints: [], derived: false, backlog: 0, sprintOf: () => undefined };
  const names = new Map(phases.sort((a, b) => a - b).map((p) => [p, vocab.phaseNames.get(p) ?? `Phase ${p}`]));
  const sprintOf = (t: Task) => (typeof t.phase === 'number' ? names.get(t.phase) : undefined);
  const sprints = [...names.values()].map((id) => {
    const own = tasks.filter((t) => sprintOf(t) === id);
    const finished = own.length > 0 && own.every((t) => ['done', 'skipped'].includes(kind(t) ?? ''));
    return sprint({ id, state: finished ? 'closed' : 'planned', done: own.filter((t) => kind(t) === 'done').length });
  });
  return { sprints, derived: true, backlog: 0, sprintOf };
}

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
    // The kind, and the work, stage and resolution when the test gives none; a test that gives atm's other answers
    // gives them itself
    const work = t.derived?.work ?? workOf(t, kind);
    const stage = t.derived?.stage ?? stageOf(t, kind);
    const resolution = t.derived?.resolution ?? resolutionOf(t, kind);
    const more = { ...(work ? { work } : {}), ...(stage ? { stage } : {}), ...(resolution ? { resolution } : {}) };
    return kind ? { ...t, derived: { ...t.derived, kind, ...more } as Task['derived'] } : t;
  }) as T[];
}

/** A fake of atm's stage (DM-4.3, `derived.stage`) for unit tests on fixtures: a done task's, merged, in a PR or done */
function stageOf(t: Task, kind: string | undefined): string | undefined {
  if (kind !== 'done') return undefined;
  if (t.merged_at) return 'merged';
  return t.pr && typeof t.pr === 'object' && typeof t.pr.number === 'number' ? 'in_pr' : 'done';
}

/** A fake of atm's resolution (SR-2.8, `derived.resolution`): a finished task's own, else the default */
function resolutionOf(t: Task, kind: string | undefined): string | undefined {
  if (kind === 'done') return t.resolution || 'done';
  if (kind === 'skipped') return t.resolution || 'wont_do';
  return undefined;
}

/**
 * A fake of atm's links and places (ATM-719) for unit tests on fixtures, over the project's `tasks` and its `meta`:
 * the links pointing at each task (`required_by`, `related_by`, `duplicated_by`), its release (its own, else its
 * sprint's: `effective_release`) and whether it is left open in a closed sprint (`in_closed_sprint`). atm's rules are
 * atm-lib's (`tasks::Backlinks`, `query`), tested there; this gives the panels what atm would.
 */
export function withLinks<T extends Task>(tasks: readonly T[], meta: TasksFile['meta'] = {}): T[] {
  const sprints = ((meta as { sprints?: { id: string; state?: string; release?: string }[] }).sprints ?? []).filter(
    (s) => s && typeof s.id === 'string',
  );
  const sprintOf = (t: Task) =>
    typeof t.sprint === 'string' ? sprints.find((s) => s.id.toLowerCase() === t.sprint!.toLowerCase()) : undefined;
  const pointing = (t: Task, at: (other: Task) => boolean) =>
    tasks.filter((other) => other.id !== t.id && at(other)).map((other) => other.id);
  return tasks.map((t) => {
    const own = (t.related ?? []).filter((id) => typeof id === 'string');
    const kind = t.derived?.kind;
    const sprint = sprintOf(t);
    const release = t.release ?? sprint?.release;
    const links = {
      required_by: pointing(t, (o) => (o.dependencies ?? []).includes(t.id)),
      related_by: pointing(t, (o) => (o.related ?? []).includes(t.id) && !own.includes(o.id)),
      duplicated_by: pointing(t, (o) => o.duplicate_of === t.id),
    };
    const more = {
      ...Object.fromEntries(Object.entries(links).filter(([, ids]) => ids.length)),
      ...(release ? { effective_release: release } : {}),
      ...(sprint?.state === 'closed' && kind !== 'done' && kind !== 'skipped' ? { in_closed_sprint: true } : {}),
    };
    return { ...t, derived: { ...more, ...t.derived } as Task['derived'] };
  }) as T[];
}

/**
 * A fake of atm's work (ATM-721, `derived.work`) for unit tests on fixtures: one stretch from the task's start to its
 * review request or its finish (to its start while active: the dashboard runs a live one to now). atm's rule (every
 * attempt and round, less reviews and blocks) is atm-lib's `work` table; this only gives the charts something to draw.
 */
function workOf(t: Task, kind: string | undefined): TaskDerived['work'] {
  const time = (iso: string | undefined) => (iso && !Number.isNaN(Date.parse(iso)) ? iso : undefined);
  const from = time(t.started_at);
  if (!from || !['active', 'review', 'done'].includes(kind ?? '')) return undefined;
  const to =
    kind === 'active'
      ? from
      : (time(t.review_requested_at) ?? time(kind === 'done' ? t.completed_at : t.updated_at) ?? from);
  const [start, end] = [Date.parse(from), Date.parse(to)];
  // As atm: a finish before the start is a time that can't be, no work
  if (end < start) return undefined;
  const by = t.started_by ?? t.agent;
  return {
    minutes: (end - start) / 60_000,
    live: kind === 'active',
    spans: [{ from, to: new Date(end).toISOString(), ...(by ? { by } : {}) }],
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
export const progressTasks: Task[] = withKinds(
  (progress.tasks as Task[]).map((x) => viewed(x, { agent_work: !PROGRESS_NO_AGENT_WORK.has(x.id) })),
);

/** A task, with atm's answer of whether agent work is left on it */
export const taskWith = (id: string, status: string, extra: Partial<Task> = {}, agentWork = true): Task =>
  withKinds([
    viewed({ id, name: id, phase: 0, type: 'feature', description: '', status, ...extra }, { agent_work: agentWork }),
  ])[0];
