// The selected dashboard view lives in the address (/projects/<id>/kanban, lib/address.ts) so it survives reloads and
// can be shared. The address's other parameters are kept.
import { addressParams, formatAddress } from './address';

// The task views first, then the views about the project (docs/project-stats-proposal.md §1.1, T-251); a divider
// sits between the groups, after GROUP_END
export const VIEWS = ['kanban', 'list', 'graph', 'progress', 'agents', 'project'] as const;
export type ViewId = (typeof VIEWS)[number];
export const DEFAULT_VIEW: ViewId = 'kanban';
export const GROUP_END: ViewId = 'graph';

export const VIEW_LABELS: Record<ViewId, string> = {
  graph: 'Dependency Graph',
  kanban: 'Kanban Board',
  list: 'Task List',
  agents: 'Agents',
  progress: 'Progress',
  project: 'Project',
};

export const VIEW_ICONS = {
  graph: 'network',
  kanban: 'square-kanban',
  list: 'list-todo',
  agents: 'bot',
  progress: 'trending-up',
  project: 'folder',
} as const satisfies Record<ViewId, string>;

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

/** The neighbouring view for arrow-key navigation, wrapping around at the ends. */
export function adjacentView(view: ViewId, step: 1 | -1): ViewId {
  const i = VIEWS.indexOf(view);
  return VIEWS[(i + step + VIEWS.length) % VIEWS.length];
}
