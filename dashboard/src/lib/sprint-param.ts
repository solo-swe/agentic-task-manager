// The Sprint filter in the address (T-323): "?sprint=active", "?sprint=Foundations&sprint=backlog", "?sprint=all", so
// a link opens on a sprint's board. The scope bar and the filter popup write it; a link with it wins over the saved
// view. A selection is one `sprint=` per value, as HTML forms send a list (T-494): a sprint's name may hold a comma.
import { ACTIVE, BACKLOG } from './filters';
import { addressParams, formatAddress } from './address';
import type { SprintModel } from './sprints';

const ALL = 'all';

/** The sprint selection an address names, as Sprint filter values (ACTIVE, BACKLOG, sprint IDs; [] for all tasks);
 * null when it names none. Names are matched to the project's sprints case-insensitively; unknown ones are dropped. */
export function sprintParamOf(address: string, model: SprintModel): string[] | null {
  const given = addressParams(address).getAll('sprint');
  if (!given.length) return null;
  const values = new Set<string>();
  for (const part of given.map((p) => p.trim())) {
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

/** The selection as the parameter's values: "active", "backlog", names; ["all"] for none */
export function sprintParamValues(values: Iterable<string>): string[] {
  const parts = [...values].map((v) => (v === ACTIVE ? 'active' : v === BACKLOG ? 'backlog' : v));
  return parts.length ? parts : [ALL];
}

/** `address` with its sprint parameters set to the selection (null removes them), keeping the others */
export function withSprintParam(address: string, values: Iterable<string> | null): string {
  const params = addressParams(address);
  params.delete('sprint');
  if (values !== null) for (const value of sprintParamValues(values)) params.append('sprint', value);
  return formatAddress(params);
}
