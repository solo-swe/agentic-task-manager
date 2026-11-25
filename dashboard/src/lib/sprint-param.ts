// The Sprint filter in the address (T-323): "?sprint=active", "?sprint=Foundations&sprint=backlog", "?sprint=all", so
// a link opens on a sprint's board. The scope bar and the filter popup write it; a link with it wins over the saved
// view. A selection is one `sprint=` per value, as HTML forms send a list (T-494): a sprint's name may hold a comma.
import { ACTIVE, BACKLOG } from './filters';
import { addressParams, formatAddress } from './address';
import type { SprintModel } from './sprints';
import { projectPage } from './tabs';

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

/** The Board of the project `address` shows, on one sprint (its ID, ACTIVE or BACKLOG): a sprint's link on the
 * project's Overview (ATM-756, docs/sidebar-overhaul-design.md §2.4) */
export function sprintBoardAddress(address: string, sprint: string): string {
  return withSprintParam(projectPage(address, 'kanban'), [sprint]);
}

/** The List of the project `address` shows, over every sprint: where a status count on its Overview opens (ATM-756),
 * the status chosen in the Filters */
export function statusListAddress(address: string): string {
  return withSprintParam(projectPage(address, 'list'), []);
}
