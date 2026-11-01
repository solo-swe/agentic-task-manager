// The Sprint filter in the URL (T-323): "#sprint=active", "#sprint=Foundations,backlog", "#sprint=all", so a link
// opens on a sprint's board. The scope bar and the filter popup write it; a link with it wins over the saved view.
import { ACTIVE, BACKLOG } from './filters';
import { formatHash, hashParams } from './hash';
import type { SprintModel } from './sprints';

const ALL = 'all';

/** The sprint selection a hash names, as Sprint filter values (ACTIVE, BACKLOG, sprint IDs; [] for all tasks); null
 * when it names none. Names are matched to the project's sprints case-insensitively; unknown ones are dropped. */
export function parseSprintHash(hash: string, model: SprintModel): string[] | null {
  const raw = hashParams(hash).get('sprint');
  if (raw === null) return null;
  const values = new Set<string>();
  for (const part of raw.split(',').map((p) => p.trim())) {
    const lower = part.toLowerCase();
    if (lower === ALL) return [];
    if (lower === 'active') values.add(ACTIVE);
    else if (lower === 'backlog') values.add(BACKLOG);
    else {
      const sprint = model.sprints.find((s) => s.id.toLowerCase() === lower);
      if (sprint) values.add(sprint.id);
    }
  }
  return [...values];
}

/** The selection as the parameter's value: "active", "backlog", names, comma-separated; "all" for none */
export function sprintHashValue(values: Iterable<string>): string {
  const parts = [...values].map((v) => (v === ACTIVE ? 'active' : v === BACKLOG ? 'backlog' : v));
  return parts.length ? parts.join(',') : ALL;
}

/** `hash` with its sprint parameter set to the selection (null removes it), keeping the others */
export function withSprintHash(hash: string, values: Iterable<string> | null): string {
  const params = hashParams(hash);
  if (values === null) params.delete('sprint');
  else params.set('sprint', sprintHashValue(values));
  return formatHash(params);
}
