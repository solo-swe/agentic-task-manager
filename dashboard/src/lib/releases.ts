// Releases (docs/sprints-and-releases-design.md SR-2.2, SR-7.5): meta.releases, read leniently, with what the release
// view shows for each: its sprints, its tasks by stage, and the notes draft `atm releases notes` prints.
import { effectiveRelease, type SprintModel } from './sprints';
import { stageOf } from './stages';
import type { Task, TasksFile } from './types';
import { kindOf, type Vocabulary } from './vocab';

export interface Release {
  id: string;
  name?: string;
  state: 'planned' | 'released' | 'archived';
  target?: string;
  pr?: { number: number; url?: string };
  released_at?: string;
}

export interface ReleaseSummary {
  release: Release;
  /** The sprints that deliver to it, in the project's order */
  sprints: string[];
  /** Its tasks (own release, or their sprint's) but the skipped ones */
  total: number;
  done: number;
  inPr: number;
  merged: number;
  /** The draft of its changes, in Markdown (the same as `atm releases notes`) */
  notes: string;
}

const STATES = ['planned', 'released', 'archived'] as const;
const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const text = (v: unknown) => (typeof v === 'string' && v !== '' ? v : undefined);

/** meta.releases, read leniently: entries without an ID are left out, an unknown state counts as planned. */
export function readReleases(meta: TasksFile['meta']): Release[] {
  const list = isRecord(meta) && Array.isArray(meta.releases) ? meta.releases : [];
  const releases: Release[] = [];
  for (const item of list) {
    if (!isRecord(item) || !text(item.id) || releases.some((r) => r.id === item.id)) continue;
    const pr = isRecord(item.pr) && typeof item.pr.number === 'number' ? item.pr : undefined;
    releases.push({
      id: item.id as string,
      state: STATES.includes(item.state as Release['state']) ? (item.state as Release['state']) : 'planned',
      ...(text(item.name) ? { name: item.name as string } : {}),
      ...(text(item.target) ? { target: item.target as string } : {}),
      ...(pr ? { pr: { number: pr.number as number, ...(text(pr.url) ? { url: pr.url as string } : {}) } } : {}),
      ...(text(item.released_at) ? { released_at: item.released_at as string } : {}),
    });
  }
  return releases;
}

/** The last outcome note of a task, or undefined */
const outcomeOf = (task: Task) => [...(task.notes ?? [])].reverse().find((n) => n?.kind === 'outcome')?.text;

/** The notes draft (§3.3): done tasks by type in the project's order, with their outcome notes; then what's open */
export function releaseNotes(release: Release, tasks: Task[], vocab: Vocabulary): string {
  const done = tasks.filter((t) => kindOf(vocab, t.status) === 'done');
  const open = tasks.filter((t) => !['done', 'skipped'].includes(kindOf(vocab, t.status) ?? ''));
  const types = [...vocab.types.map((t) => [t.id, t.label]), ...done.map((t) => [t.type, t.type])];
  const seen = new Set<string>();
  const lines = [`# ${release.id}${release.name ? ` “${release.name}”` : ''}`];
  for (const [id, label] of types) {
    const own = done.filter((t) => t.type === id);
    if (seen.has(id) || !own.length) continue;
    seen.add(id);
    lines.push('', `## ${label}`, '');
    for (const t of own) lines.push(`- **${t.id}** ${t.name}${outcomeOf(t) ? `: ${outcomeOf(t)}` : ''}`);
  }
  if (!done.length) lines.push('', 'Nothing done in it yet.');
  if (open.length) lines.push('', '## Not done yet', '', ...open.map((t) => `- ${t.id} ${t.name} (${t.status})`));
  return lines.join('\n');
}

/**
 * The release view's two parts (T-323): the releases still open (planned), in the project's order, and the released
 * ones, newest first (by released_at; those without one after, the last listed first).
 */
export function splitReleases(summaries: ReleaseSummary[]): { open: ReleaseSummary[]; released: ReleaseSummary[] } {
  const at = (s: ReleaseSummary) => Date.parse(s.release.released_at ?? '') || -Infinity;
  const released = summaries
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => s.release.state === 'released')
    .sort((a, b) => at(b.s) - at(a.s) || b.i - a.i)
    .map(({ s }) => s);
  return { open: summaries.filter((s) => s.release.state !== 'released'), released };
}

/** Each release not archived (all with `withArchived`), with its sprints, its tasks by stage and its notes draft */
export function releaseSummaries(
  meta: TasksFile['meta'],
  tasks: Task[],
  model: SprintModel,
  vocab: Vocabulary,
  withArchived = false,
): ReleaseSummary[] {
  return readReleases(meta)
    .filter((r) => withArchived || r.state !== 'archived')
    .map((release) => {
      const own = tasks.filter((t) => effectiveRelease(model, t) === release.id);
      const counted = own.filter((t) => kindOf(vocab, t.status) !== 'skipped');
      const stages = counted.map((t) => stageOf(t, vocab));
      return {
        release,
        sprints: model.sprints.filter((s) => s.release === release.id).map((s) => s.id),
        total: counted.length,
        done: stages.filter(Boolean).length,
        inPr: stages.filter((s) => s === 'in_pr').length,
        merged: stages.filter((s) => s === 'merged').length,
        notes: releaseNotes(release, own, vocab),
      };
    });
}
