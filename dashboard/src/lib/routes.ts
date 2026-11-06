// Which page the dashboard shows: a project (/projects/<id>), the overview of all projects (/projects, /board, …), or,
// with neither in the address, the landing page's choice (docs/multi-project-dashboard-proposal.md 3.5). The address's
// other parameters (the tab, a dialog, a document) are kept (lib/address.ts).
import { addressParams, formatAddress, isOverviewTab, OVERVIEW_TABS, type OverviewTab } from './address';
import type { ProjectEntry } from './load';

export { isOverviewTab, OVERVIEW_TABS, type OverviewTab };

export type Route =
  | { page: 'project'; id: string }
  | { page: 'overview' }
  /** No project is registered (or none of them is there): how to add one */
  | { page: 'none' }
  /** The address names a project the server doesn't list, or one whose folder is gone */
  | { page: 'unknown'; id: string };

/** The project an address names, if any. */
export function projectOf(address: string): string | null {
  return addressParams(address).get('project') || null;
}

/** Whether the address is the overview's. */
export function namesOverview(address: string): boolean {
  return addressParams(address).has('overview');
}

/** The overview tab an address names (/board), or null. A project's address names a project's tab, even one of the
 * same name ("agents" is both, T-333). */
export function overviewTabOf(address: string): OverviewTab | null {
  const params = addressParams(address);
  const view = params.get('view');
  return !params.has('project') && isOverviewTab(view) ? view : null;
}

/** `address` showing project `id`, or the overview for null, on `overviewTab` when given. Other parameters are kept,
 * and the tab too when it is the same kind of page: a project's tab stays out of the overview, the overview's out of a
 * project. */
export function withProject(address: string, id: string | null, overviewTab?: OverviewTab): string {
  const params = addressParams(address);
  // Whose tab the view is: the page's the address names; on the landing page (neither named), by its value
  const view = params.get('view');
  const projectView = params.has('project') || (!params.has('overview') && !isOverviewTab(view));
  params.delete('project');
  params.delete('overview');
  if (view !== null && projectView !== (id !== null)) params.delete('view');
  // A task's details are its project's
  if (id !== projectOf(address)) params.delete('task');
  if (id === null) {
    params.set('overview', '');
    if (overviewTab) params.set('view', overviewTab);
  } else params.set('project', id);
  return formatAddress(params);
}

/** The task whose details an address opens (?task=T-1), if any */
export function taskOf(address: string): string | null {
  return addressParams(address).get('task') || null;
}

/** `address` with task `id`'s details open (or none, for null), keeping the other parameters */
export function withTask(address: string, id: string | null): string {
  const params = addressParams(address);
  params.delete('task');
  if (id !== null) params.set('task', id);
  return formatAddress(params);
}

/**
 * The page for an address. A named project is shown if the server lists it and its files are there; /projects (or
 * another overview tab) is the overview. Without either, the landing page: the overview with two or more projects,
 * the project itself with one, and how to add a project with none.
 */
export function routeFor(projects: readonly ProjectEntry[], address: string): Route {
  const present = projects.filter((p) => !p.missing);
  const named = projectOf(address);
  if (named !== null) {
    return present.some((p) => p.id === named) ? { page: 'project', id: named } : { page: 'unknown', id: named };
  }
  if (namesOverview(address)) return { page: 'overview' };
  if (present.length === 0) return { page: 'none' };
  if (present.length === 1) return { page: 'project', id: present[0].id };
  return { page: 'overview' };
}
