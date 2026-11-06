// The dashboard's address (T-429): the page in the path, what is open over it in the query.
//
//   /                          the landing page (the overview with two or more projects, the project with one)
//   /projects                  the overview's project cards; /board, /progress, /agents its other tabs
//   /projects/<id>[/<tab>]     a project, on a tab (kanban, list, graph, progress, agents, project)
//   ?sprint=…&task=T-1&doc=docs/x.md:L40&about    the Sprint filter, a task's details, a document, a dialog
//
// The page's state is read and written as parameters with the names the hash used before (`project`, `overview`,
// `view`, then the query's own), so the helpers that change one (tabs.ts, dialogs.ts, …) don't need to know which
// part of the address it lives in. An old link (`#project=x&view=list`) has the same parameters in its hash: it is
// read as such, and the page replaces it with its new address. The route files under `src/routes/` are these paths.

/** The All projects page's tabs (T-259): the project cards, the board of every project's tasks, every project's
 * progress (T-329) or agents (T-333) */
export const OVERVIEW_TABS = ['projects', 'board', 'progress', 'agents'] as const;
export type OverviewTab = (typeof OVERVIEW_TABS)[number];

export function isOverviewTab(value: unknown): value is OverviewTab {
  return OVERVIEW_TABS.includes(value as OverviewTab);
}

/** Parameters in the path, not the query */
const PATH_KEYS = ['project', 'overview', 'view'];
/** Parameters an old `#…` link may hold (the hash routes before T-429) */
const LEGACY_KEYS = [...PATH_KEYS, 'sprint', 'doc', 'about', 'help'];
const PROJECTS = 'projects';

function decode(segment: string): string | null {
  try {
    return decodeURIComponent(segment);
  } catch {
    return null;
  }
}

/** An address's parameters: `/projects/demo/list?sprint=a` → project=demo, view=list, sprint=a. A path the dashboard
 * has no page for names none (the landing page). */
export function addressParams(address: string): URLSearchParams {
  const q = address.indexOf('?');
  const path = (q >= 0 ? address.slice(0, q) : address).split('#')[0];
  const query = q >= 0 ? address.slice(q + 1).split('#')[0] : '';
  const params = new URLSearchParams();
  const segments = path.split('/').filter(Boolean).map(decode);
  if (segments.every((s): s is string => s !== null && s !== '')) {
    const [first, id, tab, ...rest] = segments;
    if (first === PROJECTS && id === undefined) {
      params.set('overview', '');
      params.set('view', PROJECTS);
    } else if (first === PROJECTS && rest.length === 0) {
      params.set('project', id);
      if (tab !== undefined) params.set('view', tab);
    } else if (segments.length === 1 && isOverviewTab(first)) {
      params.set('overview', '');
      params.set('view', first);
    }
  }
  for (const [key, value] of new URLSearchParams(query)) {
    // The landing page may name a tab (an old link's `#view=list`), any other page in its path
    if (key === 'view' ? !params.has('project') && !params.has('overview') : !PATH_KEYS.includes(key))
      params.append(key, value);
  }
  return params;
}

/** The address for `params`: the page as a path, the rest as the query, a flag without "=" (`?about`). Slashes,
 * colons and commas stay readable (`?doc=docs/x.md:L40`, `?sprint=a,b`). */
export function formatAddress(params: URLSearchParams): string {
  const view = params.get('view');
  const project = params.get('project');
  let path = '/';
  let pathView = false;
  if (project) {
    path = `/${PROJECTS}/${encodeURIComponent(project)}` + (view ? `/${encodeURIComponent(view)}` : '');
    pathView = true;
  } else if (params.has('overview')) {
    path = '/' + (view && isOverviewTab(view) ? view : PROJECTS);
    pathView = true;
  }
  const parts = [...params]
    .filter(([key]) => !(key === 'project' || key === 'overview' || (key === 'view' && pathView)))
    .map(([key, value]) =>
      value === ''
        ? encodeURIComponent(key)
        : `${encodeURIComponent(key)}=${encodeURIComponent(value).replace(/%2C/g, ',').replace(/%2F/g, '/').replace(/%3A/g, ':')}`,
    );
  return parts.length ? `${path}?${parts.join('&')}` : path;
}

/** The dashboard's page paths, as `src/routes/` has them and `atm serve` answers them (api/server.py PAGE_PATH):
 * `/`, `/projects`, `/projects/<id>`, `/projects/<id>/<tab>`, `/board`, `/progress`, `/agents` */
export const PAGE_PATH = /^\/(?:projects(?:\/[^/]+(?:\/[a-z]+)?)?|board|progress|agents)?\/?$/;

export function isPagePath(pathname: string): boolean {
  return PAGE_PATH.test(pathname);
}

/** The parameters of an old `#…` link (`#project=x&view=list&doc=…`), or null for a hash that isn't one (a heading's
 * anchor, nothing) */
export function legacyParams(hash: string): URLSearchParams | null {
  const params = new URLSearchParams(hash.replace(/^#/, ''));
  return [...params.keys()].some((key) => LEGACY_KEYS.includes(key)) ? params : null;
}

/** The address a location shows, as the dashboard writes it: an old `#…` link's, or the path and query's own */
export function addressOf(location: { pathname: string; search: string; hash: string }): string {
  const legacy = legacyParams(location.hash);
  return formatAddress(legacy ?? addressParams(location.pathname + location.search));
}
