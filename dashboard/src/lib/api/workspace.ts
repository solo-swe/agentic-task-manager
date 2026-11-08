// The workspace the dashboard reads from (ADR-003 §5): its address names it (`/w/<w>/…`, lib/address.ts; none is
// `default`), and every request goes to that workspace's routes. `default`'s are the routes atm serve has always had
// (`api/projects`, `api/v1/…`, `p/<id>/…`), which the server keeps as `default`'s; another workspace's are the v2 routes
// (`api/v2/workspaces/<w>/…`, docs/multi-project-api.md §2.8).

/** The workspace every project atm meets joins, always there */
export const DEFAULT_WORKSPACE = 'default';

let current = DEFAULT_WORKSPACE;

/** The workspace requests go to */
export function apiWorkspace(): string {
  return current;
}

/** Send requests to workspace `id` from now on */
export function setApiWorkspace(id: string) {
  current = id || DEFAULT_WORKSPACE;
}

/** The v2 routes' prefix for the current workspace, or null for `default` (its routes are the v1 ones) */
export function workspacePrefix(): string | null {
  return current === DEFAULT_WORKSPACE ? null : `/api/v2/workspaces/${encodeURIComponent(current)}`;
}
