// Which page the dashboard shows: a project ("#project=<id>"), the overview of all projects ("#overview"), or, with
// neither in the URL, the landing page's choice (docs/multi-project-dashboard-proposal.md 3.5). Other hash
// parameters (the view, a dialog) are kept.
import { formatHash, hashParams } from './hash';
import type { ProjectEntry } from './load';

export type Route =
  | { page: 'project'; id: string }
  | { page: 'overview' }
  /** No project is registered (or none of them is there): how to add one */
  | { page: 'none' }
  /** The URL names a project the server doesn't list, or one whose folder is gone */
  | { page: 'unknown'; id: string };

/** The project named in a location hash, if any. */
export function parseProjectHash(hash: string): string | null {
  return hashParams(hash).get('project') || null;
}

/** Whether the hash asks for the overview. */
export function hasOverviewHash(hash: string): boolean {
  return hashParams(hash).has('overview');
}

/** The All projects page's tabs (T-259): the project cards, the board of every project's tasks, every project's
 * progress (T-329) or agents (T-333) */
export const OVERVIEW_TABS = ['projects', 'board', 'progress', 'agents'] as const;
export type OverviewTab = (typeof OVERVIEW_TABS)[number];

export function isOverviewTab(value: unknown): value is OverviewTab {
  return OVERVIEW_TABS.includes(value as OverviewTab);
}

/** The overview tab a hash names ("#overview&view=board"), or null. A project's hash names a project's tab, even one
 * of the same name ("agents" is both, T-333). */
export function parseOverviewTab(hash: string): OverviewTab | null {
  const params = hashParams(hash);
  const view = params.get('view');
  return !params.has('project') && isOverviewTab(view) ? view : null;
}

/** `hash` showing project `id`, or the overview for null. Other parameters are kept, and the view too when it is the
 * same kind of page: a project's tab stays out of the overview, the overview's tab out of a project. */
export function withProjectHash(hash: string, id: string | null): string {
  const params = hashParams(hash);
  // Whose tab the view is: the page's the hash names; on the landing page (neither named), by its value
  const view = params.get('view');
  const projectView = params.has('project') || (!params.has('overview') && !isOverviewTab(view));
  params.delete('project');
  params.delete('overview');
  if (view !== null && projectView !== (id !== null)) params.delete('view');
  if (id === null) params.set('overview', '');
  else params.set('project', id);
  return formatHash(params);
}

/**
 * The page for a hash. A named project is shown if the server lists it and its files are there; #overview is the
 * overview. Without either, the landing page: the overview with two or more projects, the project itself with one,
 * and how to add a project with none.
 */
export function routeFor(projects: readonly ProjectEntry[], hash: string): Route {
  const present = projects.filter((p) => !p.missing);
  const named = parseProjectHash(hash);
  if (named !== null) {
    return present.some((p) => p.id === named) ? { page: 'project', id: named } : { page: 'unknown', id: named };
  }
  if (hasOverviewHash(hash)) return { page: 'overview' };
  if (present.length === 0) return { page: 'none' };
  if (present.length === 1) return { page: 'project', id: present[0].id };
  return { page: 'overview' };
}
