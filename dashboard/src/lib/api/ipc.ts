// The dashboard's IPC transport (T-457): in the atm app, the same requests the browser makes of atm serve are the app's
// commands (app/src-tauri/src/commands.rs, ADR-003 §5), which answer what the REST routes do. `ipcFetch` takes the
// dashboard's own URLs (api/http.ts builds them: `default`'s v1 routes, another workspace's v2 ones) and answers each
// with a `Response`, so everything above the client layer works unchanged. No ETags: the app reads its own files, and
// its live changes come through the subscriptions' channels.
import { invoke } from '@tauri-apps/api/core';
import { DEFAULT_WORKSPACE } from './workspace';

/** A command's error, as the app sends it (`AppError`) */
interface AppError {
  kind: 'not_found' | 'invalid' | 'refused' | 'too_large' | 'failed';
  message: string;
}

/** An image the app sends (`FileData`) */
interface FileData {
  content_type: string;
  data: number[];
}

const STATUS: Record<AppError['kind'], number> = {
  not_found: 404,
  invalid: 400,
  refused: 403,
  too_large: 413,
  failed: 500,
};

/** A request the app can answer: the command, its arguments, and whether it answers with an image */
interface Call {
  command: string;
  args: Record<string, unknown>;
  image?: boolean;
}

/** The filters of a tasks route's query, as the `project_tasks` command takes them (`TaskFilters`) */
function taskFilters(query: URLSearchParams): Record<string, unknown> {
  const flag = (name: string) => ['true', '1'].includes((query.get(name) ?? '').toLowerCase());
  const first = (name: string) => query.get(name) || undefined;
  return {
    sprint: query.getAll('sprint'),
    backlog: flag('backlog'),
    release: first('release'),
    component: first('component'),
    status: query.getAll('status'),
    type: first('type'),
    priority: first('priority'),
    label: query.getAll('label'),
    agent: first('agent'),
    ready: flag('ready'),
    blocked: flag('blocked'),
    in_pr: flag('in_pr'),
    with_user: flag('with_user'),
    archived: flag('archived'),
  };
}

/** A project's route (`tasks`, `documents`, …) as its command */
function projectCall(workspace: string, project: string, route: string, query: URLSearchParams): Call | null {
  const at = { workspace, project };
  switch (route) {
    case 'tasks':
      return { command: 'project_tasks', args: { ...at, filters: taskFilters(query) } };
    case 'documents':
      return { command: 'document', args: { ...at, path: query.get('path') ?? '' } };
    case 'files':
      return { command: 'file', args: { ...at, path: query.get('path') ?? '' }, image: true };
    case 'claims':
    case 'claims.json':
      return { command: 'claims', args: at };
    case 'worktrees':
    case 'worktrees.json':
      return { command: 'worktrees', args: at };
    case 'logo':
      return { command: 'logo', args: at, image: true };
    default:
      return null;
  }
}

/** The command a dashboard URL is, or null for one the app doesn't answer (whoami: there is no server to restart) */
export function callFor(url: string): Call | null {
  const parsed = new URL(url, 'http://app.invalid');
  const parts = parsed.pathname.split('/').filter(Boolean).map(decodeURIComponent);
  const query = parsed.searchParams;
  const [a, b, c, d, e, f, g] = parts;
  // The v2 routes: api/v2/workspaces[/<w>/projects | /<w>/search | /<w>/projects/<p>/<route>]
  if (a === 'api' && b === 'v2' && c === 'workspaces') {
    if (d === undefined) return { command: 'list_workspaces', args: {} };
    if (e === 'projects' && f === undefined) return { command: 'list_projects', args: { workspace: d } };
    if (e === 'search' && f === undefined)
      return { command: 'search', args: { workspace: d, q: query.get('q') ?? '' } };
    if (e === 'projects' && f && g && parts.length === 7) return projectCall(d, f, g, query);
    return null;
  }
  // The v1 routes: the default workspace's
  if (a === 'api' && b === 'projects' && c === undefined)
    return { command: 'list_projects', args: { workspace: DEFAULT_WORKSPACE } };
  if (a === 'api' && b === 'search' && c === undefined)
    return { command: 'search', args: { workspace: DEFAULT_WORKSPACE, q: query.get('q') ?? '' } };
  if (a === 'api' && b === 'v1' && c === 'projects' && d && e && parts.length === 5)
    return projectCall(DEFAULT_WORKSPACE, d, e, query);
  if (a === 'p' && b && c && parts.length === 3) return projectCall(DEFAULT_WORKSPACE, b, c, query);
  return null;
}

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

/** `fetch`'s shape, answered by the app's commands (a request's options, such as its validators, don't apply) */
export async function ipcFetch(input: RequestInfo | URL): Promise<Response> {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
  const call = callFor(url);
  if (!call) return json({ error: 'not found' }, 404);
  try {
    const answer = await invoke<unknown>(call.command, call.args);
    if (!call.image) return json(answer);
    if (answer === null) return json({ error: 'no logo' }, 404);
    const file = answer as FileData;
    return new Response(new Uint8Array(file.data), { status: 200, headers: { 'Content-Type': file.content_type } });
  } catch (e) {
    const error = e as Partial<AppError>;
    const status = (error.kind && STATUS[error.kind]) || 500;
    return json({ error: error.message ?? String(e) }, status);
  }
}
