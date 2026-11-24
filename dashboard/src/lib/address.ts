// The dashboard's address (T-429): the page in the path, what is open over it in the query.
//
//   /                          the landing page (the overview with two or more projects, the project with one)
//   /projects                  the overview's project cards; /board, /list, /progress, /agents, /reviews its other tabs
//   /projects/<id>[/<tab>]     a project, on a tab (kanban, list, graph, progress, agents, project, reviews, inbox,
//                              documents); without one, its Overview (the `project` tab, docs/sidebar-overhaul-design.md)
//   /new                       the app's new tab: a chooser of scope and item (ATM-755)
//   ?sprint=…&task=T-1&doc=docs/x.md:L40&about    the Sprint filter, a task's details, a document, a dialog
//   /w/<w>/…                   any of these in workspace <w> (ADR-003 §5); without it, `default` (every link so far)
//
// Each page belongs to one of the sidebar's items (docs/sidebar-overhaul-design.md §4, ATM-751): `itemOf`.
//
// The page's state is read and written as parameters with the names the hash used before (`project`, `overview`,
// `view`, then the query's own), so the helpers that change one (tabs.ts, dialogs.ts, …) don't need to know which
// part of the address it lives in. An old link (`#project=x&view=list`) has the same parameters in its hash: it is
// read as such, and the page replaces it with its new address. The route files under `src/routes/` are these paths.

/** The All projects page's tabs (T-259): the project cards, the board of every project's tasks, every project's
 * progress (T-329) or agents (T-333) */
export const OVERVIEW_TABS = ['projects', 'board', 'progress', 'agents', 'reviews'] as const;
export type OverviewTab = (typeof OVERVIEW_TABS)[number];

export function isOverviewTab(value: unknown): value is OverviewTab {
  return OVERVIEW_TABS.includes(value as OverviewTab);
}

/** The All projects page's paths: its tabs, and All's List (ATM-751: the Tasks item's List; its page is ATM-752's) */
export const OVERVIEW_PATHS = [...OVERVIEW_TABS, 'list'] as const;
type OverviewPath = (typeof OVERVIEW_PATHS)[number];
const isOverviewPath = (value: unknown): value is OverviewPath => OVERVIEW_PATHS.includes(value as OverviewPath);

/** The app's new tab (ATM-755): `/new`, the chooser of scope and item */
const NEW_TAB = 'new';

/** Parameters in the path, not the query */
const PATH_KEYS = ['workspace', 'project', 'overview', 'view', 'review', 'section', NEW_TAB];
/** The project tab a review's page is under: /projects/<id>/reviews/<CR> (ATM-574) */
const REVIEWS = 'reviews';
/** A review page's sub-tabs (ATM-697): the Overview at the review's own path, the others after it (…/<CR>/diffs) */
export const REVIEW_SECTIONS = ['overview', 'diffs', 'comments'] as const;
export type ReviewSection = (typeof REVIEW_SECTIONS)[number];
const isSection = (value: unknown): value is ReviewSection => REVIEW_SECTIONS.includes(value as ReviewSection);
/** Parameters an old `#…` link may hold (the hash routes before T-429) */
const LEGACY_KEYS = ['project', 'overview', 'view', 'sprint', 'doc', 'about', 'help'];
const PROJECTS = 'projects';
/** The workspace an address without `/w/<w>` is in */
const DEFAULT_WORKSPACE = 'default';

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
  const decoded = path.split('/').filter(Boolean).map(decode);
  // A segment that doesn't decode, or is `.` or `..` (`%2E%2E` decoded), names no page: as an ID it would change the
  // API path a read or a write goes to (ATM-676, audit D7)
  const usable = decoded.every((s): s is string => s !== null && s !== '' && s !== '.' && s !== '..');
  let segments: string[] = usable ? (decoded as string[]) : [];
  // /w/<w>/…: the page in workspace <w>
  if (usable && segments[0] === 'w' && segments[1]) {
    if (segments[1] !== DEFAULT_WORKSPACE) params.set('workspace', segments[1]);
    segments = segments.slice(2);
  }
  if (usable) {
    const [first, id, tab, ...rest] = segments;
    if (first === PROJECTS && id === undefined) {
      params.set('overview', '');
      params.set('view', PROJECTS);
    } else if (first === PROJECTS && rest.length === 0) {
      params.set('project', id);
      if (tab !== undefined) params.set('view', tab);
    } else if (
      first === PROJECTS &&
      tab === REVIEWS &&
      (rest.length === 1 || (rest.length === 2 && isSection(rest[1]) && rest[1] !== 'overview'))
    ) {
      params.set('project', id);
      params.set('view', REVIEWS);
      params.set('review', rest[0]);
      if (rest[1]) params.set('section', rest[1]);
    } else if (segments.length === 1 && isOverviewPath(first)) {
      params.set('overview', '');
      params.set('view', first);
    } else if (segments.length === 1 && first === NEW_TAB) {
      params.set(NEW_TAB, '');
    }
  }
  for (const [key, value] of new URLSearchParams(query)) {
    // The landing page may name a tab (an old link's `#view=list`), any other page in its path
    // The workspace is the path's alone
    if (key === 'view' ? !params.has('project') && !params.has('overview') : !PATH_KEYS.includes(key))
      params.append(key, value);
  }
  return params;
}

/** The address for `params`: the page as a path, the rest as the query, a flag without "=" (`?about`). Slashes,
 * colons and commas stay readable (`?doc=docs/x.md:L40`, `?sprint=Library,%20gallery`). */
export function formatAddress(params: URLSearchParams): string {
  const view = params.get('view');
  const project = params.get('project');
  const workspace = params.get('workspace');
  const prefix = workspace && workspace !== DEFAULT_WORKSPACE ? `/w/${encodeURIComponent(workspace)}` : '';
  let path = prefix || '/';
  let pathView = false;
  const review = view === REVIEWS ? params.get('review') : null;
  const section = review ? params.get('section') : null;
  if (project) {
    path =
      `${prefix}/${PROJECTS}/${encodeURIComponent(project)}` +
      (view ? `/${encodeURIComponent(view)}` : '') +
      (review ? `/${encodeURIComponent(review)}` : '') +
      (isSection(section) && section !== 'overview' ? `/${section}` : '');
    pathView = true;
  } else if (params.has('overview')) {
    path = `${prefix}/` + (view && isOverviewPath(view) ? view : PROJECTS);
    pathView = true;
  } else if (params.has(NEW_TAB)) {
    path = `${prefix}/${NEW_TAB}`;
  }
  const parts = [...params]
    .filter(
      ([key]) =>
        !(
          key === 'workspace' ||
          key === 'project' ||
          key === 'overview' ||
          key === NEW_TAB ||
          (key === 'view' && pathView) ||
          // A review is a path's alone, on the reviews tab, and so is its sub-tab
          key === 'review' ||
          key === 'section'
        ),
    )
    .map(([key, value]) =>
      value === ''
        ? encodeURIComponent(key)
        : `${encodeURIComponent(key)}=${encodeURIComponent(value).replace(/%2C/g, ',').replace(/%2F/g, '/').replace(/%3A/g, ':')}`,
    );
  return parts.length ? `${path}?${parts.join('&')}` : path;
}

/** The dashboard's page paths, as `src/routes/` has them and `atm serve` answers them (atm-api's pages.rs):
 * `/`, `/projects`, `/projects/<id>`, `/projects/<id>/<tab>`, `/projects/<id>/reviews/<CR>` (and its sub-tabs,
 * `…/<CR>/diffs` and `…/<CR>/comments`, ATM-697), `/board`, `/list`, `/progress`, `/agents`, `/reviews` (ATM-575),
 * `/new` (ATM-751), each also under `/w/<w>` (and `/w/<w>` itself) */
export const PAGE_PATH =
  /^(?:\/w\/[^/]+)?(?:\/(?:projects(?:\/[^/]+(?:\/[a-z]+|\/reviews\/[A-Za-z0-9.-]{1,64}(?:\/(?:diffs|comments))?)?)?|board|list|progress|agents|reviews|new)?\/?)?$/;

/** The sidebar's items (docs/sidebar-overhaul-design.md §1, ATM-751): each page belongs to one */
export const ITEMS = ['overview', 'inbox', 'tasks', 'reviews', 'documents', 'analysis'] as const;
export type Item = (typeof ITEMS)[number];
/** All projects' items: no Inbox or Documents (Hakan, CR-135) */
export const ALL_ITEMS: readonly Item[] = ['overview', 'tasks', 'reviews', 'analysis'];

/** A view's item: a project's tabs and All's, which share their names but the cards (`projects`) and the board */
const ITEM_OF: Record<string, Item> = {
  project: 'overview',
  projects: 'overview',
  inbox: 'inbox',
  kanban: 'tasks',
  board: 'tasks',
  list: 'tasks',
  graph: 'tasks',
  reviews: 'reviews',
  documents: 'documents',
  progress: 'analysis',
  agents: 'analysis',
};

/** The item the page `address` shows belongs to: a project's or All's page, by its view (a project without one, or one
 * it doesn't know, is on its Overview); none for the landing page (which shows All's or the one project's Overview, as
 * the projects listed decide: routes.ts `routeFor`) and the new tab */
export function itemOf(address: string): Item | null {
  const params = addressParams(address);
  const view = params.get('view') ?? '';
  if (params.has('project')) return ITEM_OF[view] ?? 'overview';
  if (params.has('overview')) return ITEM_OF[view] ?? 'overview';
  return null;
}

/** Whether `address` is the app's new tab (`/new`) */
export function namesNewTab(address: string): boolean {
  return addressParams(address).has(NEW_TAB);
}

/** The review a review's page shows (/projects/<id>/reviews/<CR>), its sub-tab (the path's end, ATM-697), and the view
 * of it its query names: the round, the round against, the file, the comment in view (`?comment=c12`, never a `#`
 * anchor: the app's links refuse fragments) */
export interface ReviewAddress {
  review: string | null;
  section: ReviewSection;
  round: number | null;
  against: number | null;
  file: string | null;
  comment: string | null;
}

const roundParam = (value: string | null) => {
  const n = Number(value);
  return value && Number.isInteger(n) && n > 0 ? n : null;
};

export function reviewOf(address: string): ReviewAddress {
  const params = addressParams(address);
  const review = params.get('view') === REVIEWS ? params.get('review') : null;
  const section = params.get('section');
  return {
    review,
    section: review && isSection(section) ? section : 'overview',
    round: review ? roundParam(params.get('round')) : null,
    against: review ? roundParam(params.get('against')) : null,
    file: review ? params.get('file') : null,
    comment: review ? params.get('comment') || null : null,
  };
}

/** `address` showing `at` on its project's reviews tab: a review (and its sub-tab, its round, the round against, the
 * file, the comment in view), or the list (`review` null, the list's filter kept) */
export function withReview(address: string, at: Partial<ReviewAddress> & { review: string | null }): string {
  const params = addressParams(address);
  params.set('view', REVIEWS);
  for (const key of ['review', 'section', 'round', 'against', 'file', 'comment'] as const) params.delete(key);
  if (at.review) {
    for (const key of LIST_KEYS) params.delete(key);
    params.set('review', at.review);
    if (at.section && at.section !== 'overview') params.set('section', at.section);
    if (at.round) params.set('round', String(at.round));
    if (at.against) params.set('against', String(at.against));
    if (at.file) params.set('file', at.file);
    if (at.comment) params.set('comment', at.comment);
  }
  return formatAddress(params);
}

/** What the Reviews list lists by, in its address (ATM-697): `?q=…&status=waiting,live&author=…&reviewer=…&tasks=…
 * &branch=…` (`tasks`, as `task` is a task's details over the page) */
export interface ReviewListAddress {
  q: string;
  status: string[];
  author: string;
  reviewer: string;
  task: string;
  branch: string;
}

/** Each of the list's fields, by its parameter */
const LIST_PARAMS = {
  q: 'q',
  status: 'status',
  author: 'author',
  reviewer: 'reviewer',
  task: 'tasks',
  branch: 'branch',
};
const LIST_KEYS = Object.values(LIST_PARAMS);

/** The Reviews list's filter and search `address` names; none when it names none (the list opens with the one kept) */
export function reviewListOf(address: string): ReviewListAddress | null {
  const params = addressParams(address);
  if (params.get('view') !== REVIEWS || params.has('review') || !LIST_KEYS.some((k) => params.has(k))) return null;
  const text = (key: string) => params.get(key) ?? '';
  return {
    q: text(LIST_PARAMS.q),
    status: text(LIST_PARAMS.status).split(',').filter(Boolean),
    author: text(LIST_PARAMS.author),
    reviewer: text(LIST_PARAMS.reviewer),
    task: text(LIST_PARAMS.task),
    branch: text(LIST_PARAMS.branch),
  };
}

/** `address` (the Reviews list's) listing by `query`: each field given in the query, the empty ones left out */
export function withReviewList(address: string, query: ReviewListAddress): string {
  const params = addressParams(address);
  for (const key of LIST_KEYS) params.delete(key);
  for (const [field, key] of Object.entries(LIST_PARAMS) as [keyof ReviewListAddress, string][]) {
    const value = field === 'status' ? query.status.join(',') : query[field];
    if (value) params.set(key, value);
  }
  return formatAddress(params);
}

/** The page of review `review` of `project`, in the workspace `address` is in, with nothing else open over it (a task's
 * details or a document: ATM-585, a task linking to its review) */
export function reviewPageOf(address: string, project: string, review: string): string {
  const params = new URLSearchParams();
  const workspace = addressParams(address).get('workspace');
  if (workspace) params.set('workspace', workspace);
  params.set('project', project);
  params.set('view', REVIEWS);
  params.set('review', review);
  return formatAddress(params);
}

/** The workspace an address is in: its `/w/<w>`, else `default` */
export function workspaceOf(address: string): string {
  return addressParams(address).get('workspace') || DEFAULT_WORKSPACE;
}

/** `address` in workspace `id`: the same page and query (the project and task cleared, as they are another
 * workspace's) */
export function withWorkspace(address: string, id: string): string {
  const params = addressParams(address);
  if (workspaceOf(address) !== id) {
    params.delete('project');
    params.delete('task');
    params.delete('doc');
    params.delete('sprint');
    if (!params.has('overview')) params.delete('view');
  }
  params.delete('workspace');
  if (id !== DEFAULT_WORKSPACE) params.set('workspace', id);
  return formatAddress(params);
}

export function isPagePath(pathname: string): boolean {
  return PAGE_PATH.test(pathname);
}

/** The parameters of an old `#…` link (`#project=x&view=list&doc=…`), or null for a hash that isn't one (a heading's
 * anchor, nothing) */
export function legacyParams(hash: string): URLSearchParams | null {
  const params = new URLSearchParams(hash.replace(/^#/, ''));
  // A project named `.` or `..` is none (ATM-676, as in a path)
  const project = params.get('project');
  if (project === '.' || project === '..') params.delete('project');
  return [...params.keys()].some((key) => LEGACY_KEYS.includes(key)) ? params : null;
}

/** The address a location shows, as the dashboard writes it: an old `#…` link's, or the path and query's own */
export function addressOf(location: { pathname: string; search: string; hash: string }): string {
  const legacy = legacyParams(location.hash);
  return formatAddress(legacy ?? addressParams(location.pathname + location.search));
}
