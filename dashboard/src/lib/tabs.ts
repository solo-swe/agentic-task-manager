// The selected dashboard view lives in the address (/projects/<id>/kanban, lib/address.ts) so it survives reloads and
// can be shared. The address's other parameters are kept.
//
// The views are grouped by the sidebar's items (docs/sidebar-overhaul-design.md §1, ATM-752): the page shows the items
// as its tabs, and an item with more than one view (Tasks, Analysis) its views as sub-tabs under them.
import { addressParams, formatAddress, type Item, type OverviewTab } from './address';

/** A project's views, in their items' order */
export const VIEWS = [
  'project',
  'inbox',
  'kanban',
  'list',
  'graph',
  'reviews',
  'documents',
  'progress',
  'agents',
] as const;
export type ViewId = (typeof VIEWS)[number];
/** A project without a tab in its address opens on its Overview (the `project` tab; docs/sidebar-overhaul-design.md §7,
 * ATM-751), which links into its Board */
export const DEFAULT_VIEW: ViewId = 'project';

/** The views' names, as their sub-tabs (and a back button, places.ts) name them: the Kanban is the Board and the graph
 * Dependencies, as the sidebar's Tasks item names them (ATM-751); their addresses keep `kanban` and `graph`, so old
 * links still work. A view alone in its item is named by its item (ITEM_NAMES) */
export const VIEW_LABELS: Record<ViewId, string> = {
  project: 'Overview',
  inbox: 'Inbox',
  kanban: 'Board',
  list: 'List',
  graph: 'Dependencies',
  reviews: 'Code Reviews',
  documents: 'Documents',
  progress: 'Progress',
  agents: 'Agents',
};

export const VIEW_ICONS = {
  project: 'layout-dashboard',
  inbox: 'inbox',
  kanban: 'square-kanban',
  list: 'list-todo',
  graph: 'network',
  reviews: 'file-diff',
  documents: 'file-text',
  progress: 'trending-up',
  agents: 'bot',
} as const satisfies Record<ViewId, string>;

/** The items' icons (docs/sidebar-overhaul-design.md §1) */
export const ITEM_ICONS = {
  overview: 'layout-dashboard',
  inbox: 'inbox',
  tasks: 'square-kanban',
  reviews: 'file-diff',
  documents: 'file-text',
  analysis: 'trending-up',
} as const satisfies Record<Item, string>;

/** Each item's views on a project's page: the first is where the item opens */
export const PROJECT_ITEM_VIEWS: Record<Item, readonly ViewId[]> = {
  overview: ['project'],
  inbox: ['inbox'],
  tasks: ['kanban', 'list', 'graph'],
  reviews: ['reviews'],
  documents: ['documents'],
  analysis: ['progress', 'agents'],
};

/** Each of All projects' items' views (no Inbox or Documents, Hakan CR-135): the first is where the item opens */
export const ALL_ITEM_VIEWS: Partial<Record<Item, readonly OverviewTab[]>> = {
  overview: ['projects'],
  tasks: ['board', 'list'],
  reviews: ['reviews'],
  analysis: ['progress', 'agents'],
};

/** The item a project's view belongs to */
export function itemOfView(view: ViewId): Item {
  return (Object.keys(PROJECT_ITEM_VIEWS) as Item[]).find((item) => PROJECT_ITEM_VIEWS[item].includes(view))!;
}

/** The item one of All projects' views belongs to */
export function itemOfOverviewTab(tab: OverviewTab): Item {
  return (Object.keys(ALL_ITEM_VIEWS) as Item[]).find((item) => ALL_ITEM_VIEWS[item]!.includes(tab))!;
}

function isView(value: string | null): value is ViewId {
  return (VIEWS as readonly string[]).includes(value ?? '');
}

/** The view an address names; anything missing or unknown means the default view. */
export function viewOf(address: string): ViewId {
  const view = addressParams(address).get('view');
  return isView(view) ? view : DEFAULT_VIEW;
}

/** Whether the address names a view at all (a link that should win over the saved tab). */
export function namesView(address: string): boolean {
  return addressParams(address).has('view');
}

/** `address` on view `view`, keeping any other parameters. */
export function withView(address: string, view: ViewId): string {
  const params = addressParams(address);
  params.set('view', view);
  return formatAddress(params);
}

/** The neighbour of `value` in `values` for arrow-key navigation, wrapping around at the ends */
export function adjacent<T>(values: readonly T[], value: T, step: 1 | -1): T {
  const i = Math.max(0, values.indexOf(value));
  return values[(i + step + values.length) % values.length];
}
