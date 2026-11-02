// What a Task List card shows beyond its chips (T-338): who created and worked on the task, when, how long it took,
// and its latest outcome or handoff, from what the task already holds (no new data loads).
import { approvedBy } from './actors';
import { formatAgo, shortAgent } from './claims';
import { formatMinutes } from './format';
import { taskNotes } from './notes';
import type { Note, Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, type Vocabulary } from './vocab';
import { ms, startedBy, workSpan } from './work';

export interface CardPerson {
  role: 'created' | 'worked' | 'finished';
  who: string;
}

export interface CardFacts {
  people: CardPerson[];
  /** The moment that matters for its status: "done 2 h ago", "started 12 m ago", "created 3 d ago" */
  when: string | null;
  /** The work time: "took 35 m", or "35 m so far" while an agent works on it */
  took: string | null;
}

export function cardFacts(task: Task, now: number, vocab: Vocabulary = DEFAULT_VOCABULARY): CardFacts {
  const kind = kindOf(vocab, task.status);
  const people: CardPerson[] = [];
  if (task.created_by) people.push({ role: 'created', who: shortAgent(task.created_by) });
  const worker = startedBy(task);
  if (worker) people.push({ role: 'worked', who: shortAgent(worker) });
  const finisher = approvedBy(task);
  if (finisher) people.push({ role: 'finished', who: shortAgent(finisher) });

  const ago = (label: string, iso: string | undefined) =>
    ms(iso) === null ? null : `${label} ${formatAgo(iso!, now)}`;
  let when: string | null;
  if (kind === 'done') when = ago('done', task.completed_at);
  else if (kind === 'skipped') when = ago('skipped', task.skipped_at);
  else if (kind === 'active' || kind === 'review') when = ago('started', task.started_at);
  else when = ago('created', task.created_at);

  const span = workSpan(task, now, vocab);
  const minutes = span ? (span.end - span.start) / 60_000 : 0;
  const took =
    !span || (!span.live && minutes < 1)
      ? null
      : span.live
        ? `${formatMinutes(minutes)} so far`
        : `took ${formatMinutes(minutes)}`;
  return { people, when, took };
}

/** The latest outcome or handoff: what was done, or what the last agent left unfinished */
export function cardNote(task: Task): Note | undefined {
  return taskNotes(task).find((n) => n.kind === 'outcome' || n.kind === 'handoff');
}
