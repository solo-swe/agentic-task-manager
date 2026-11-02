// Loading from atm serve's API (docs/multi-project-api.md): the registered projects from api/projects, each project's
// tasks from api/v1/projects/<id>/tasks, and its other files from p/<id>/. The requests go through the client layer,
// api/http.ts; this module reads what they return.
import { browserFetch, fetchFile, projectFileUrl, v1TasksUrl, type Fetch, type Validators } from './api/http';
import type { Task, TasksFile } from './types';
import type { NeedsYouItem } from './waiting';

export { fetchProjectFile, projectFileUrl, type ProjectFile, type Validators } from './api/http';

export const DEFAULT_PROJECT_NAME = 'Task Dashboard';
export const PROJECTS_URL = 'api/projects';

/** A sprint as the server's summary gives it */
export interface SprintNumbers {
  id: string;
  state: 'planned' | 'active' | 'closed';
  due: string | null;
  release: string | null;
  done: number;
  total: number;
  percent: number;
}

/** A project's numbers for the overview and the project selector, computed by the server */
export interface ProjectSummary {
  total: number;
  done: number;
  skipped: number;
  percent: number;
  /** The active sprint with its progress (SR-7.7); null when none is active; absent from servers before 0.24 */
  active_sprint?: (SprintNumbers & { goal: string | null }) | null;
  /** Up to 4 sprints with tasks, in the project's order: the open ones, then the latest closed; null without sprints */
  recent_sprints?: SprintNumbers[] | null;
  agents_working: number;
  /** Live claims, oldest first: who works on what (absent from servers before 0.18; names capped at 80 characters) */
  working?: { task: string; name: string; agent: string; branch: string | null }[];
  /** Up to 2 tasks the loop takes next, in its order (absent before 0.18) */
  next?: { id: string; name: string }[];
  /** The task completed last (absent before 0.18) */
  last_done?: { id: string; name: string; completed_at: string } | null;
  /** meta.description, capped at 200 characters; null without one (absent before 0.18) */
  description?: string | null;
  /** The main worktree's branch and the latest PR recorded on the tasks (absent before 0.18) */
  git?: { branch: string | null; pr: { number: number; url: string | null; state: 'open' | 'merged' } | null };
  /** `together`: to do with the user (absent from servers before 0.24) */
  needs_you: { review: number; blocked_on_user: number; together?: number };
  /** The tasks behind needs_you (absent from servers before 0.17) */
  needs_you_tasks?: NeedsYouItem[];
  last_activity: string | null;
  completions_14d: number[];
  /** The project's cards for the All projects board (absent before 0.20; lib/allboard.ts) */
  board?: ProjectBoard;
  /** The same, of the active sprint's tasks only; null without an active sprint (absent before 0.26; T-340) */
  board_active_sprint?: ProjectBoard | null;
  /** The overview card's line, archived tasks included (absent before 0.20; lib/projects.ts statsLine) */
  stats?: { started: string | null; tasks: number; agent_minutes: number };
}

/** A card on the All projects board: what a project's Kanban card shows, plus its column's fields */
export interface BoardCard {
  id: string;
  name: string;
  type: string;
  /** The task's sprint; null in the backlog */
  sprint: string | null;
  priority: string | null;
  labels: string[];
  /** In progress: who works on it */
  by?: string | null;
  /** In progress, review, blocked: since when */
  since?: string | null;
  /** Blocked: who decides, and how many questions */
  waiting_on?: string | null;
  questions?: number;
  /** Done: when */
  completed_at?: string | null;
  /** To do (T-349): done with the user, or the dependencies it waits for */
  with_user?: boolean;
  waiting_for?: string[];
}

export type BoardColumnId = 'todo' | 'active' | 'review' | 'blocked' | 'done';
/** A project's column: its first cards, and how many there are in all */
export type ProjectBoard = Record<BoardColumnId, { count: number; cards: BoardCard[] }>;

/** A registered project, as api/projects lists it */
export interface ProjectEntry {
  id: string;
  name: string;
  path: string;
  /** Its folder or tasks.json is gone */
  missing: boolean;
  summary: ProjectSummary | null;
  /** Its git worktrees, main first (empty outside git, or for a missing project) */
  worktrees?: { path: string; branch: string | null; main: boolean }[];
  /** Where its logo is served (p/<id>/logo), or null without one; older servers don't send it */
  logo?: string | null;
}

export interface LoadedTasks {
  tasks: Task[];
  taskMap: Map<string, Task>;
  projectName: string;
  meta: TasksFile['meta'];
  /** The file's size, in characters (≈ bytes: task files are mostly ASCII), for the archive notice (D-5.4) */
  bytes: number;
}

export function parseTasksFile(data: TasksFile, bytes = 0): LoadedTasks {
  const tasks = data.tasks ?? [];
  return {
    tasks,
    taskMap: new Map(tasks.map((t) => [t.id, t])),
    projectName: data.meta?.project || DEFAULT_PROJECT_NAME,
    meta: data.meta,
    bytes,
  };
}

/** A project's logo, as api/projects gives it: p/<id>/logo, relative to the dashboard */
const LOGO_URL = /^p\/[^/?#:]+\/logo$/;

const isProject = (p: unknown): p is ProjectEntry =>
  !!p && typeof p === 'object' && typeof (p as ProjectEntry).id === 'string' && !!(p as ProjectEntry).id;

export type ProjectsResult =
  { unchanged: true } | { unchanged: false; projects: ProjectEntry[]; validators: Validators };

/**
 * Fetches api/projects, conditionally when `since` is given. Entries without an id are left out; a missing name falls
 * back to the id. Rejects with "HTTP <status>" for error responses and with the JSON error for invalid content.
 */
export async function loadProjects(fetchFn: Fetch = browserFetch, since?: Validators): Promise<ProjectsResult> {
  const file = await fetchFile(PROJECTS_URL, fetchFn, since);
  if (file.unchanged) return file;
  const list = (JSON.parse(file.text) as { projects?: unknown }).projects;
  const projects = (Array.isArray(list) ? list : []).filter(isProject).map((p) => ({
    id: p.id,
    name: typeof p.name === 'string' && p.name ? p.name : p.id,
    path: typeof p.path === 'string' ? p.path : '',
    missing: p.missing === true,
    summary: p.summary && typeof p.summary === 'object' ? p.summary : null,
    worktrees: Array.isArray(p.worktrees)
      ? p.worktrees
          .filter((w) => !!w && typeof w === 'object' && typeof w.path === 'string')
          .map((w) => ({ path: w.path, branch: typeof w.branch === 'string' ? w.branch : null, main: w.main === true }))
      : [],
    // Only the server's own route: never an address elsewhere (the dashboard makes no outside requests)
    logo: typeof p.logo === 'string' && LOGO_URL.test(p.logo) ? p.logo : null,
  }));
  return { unchanged: false, projects, validators: file.validators };
}

export const SEARCH_URL = 'api/search';

/**
 * The tasks matching `query` in every project (api/search, T-258): their IDs per project id. Anything unexpected in
 * the answer is left out; a failed request throws.
 */
export async function searchProjects(query: string, fetchFn: Fetch = browserFetch): Promise<Map<string, Set<string>>> {
  const res = await fetchFn(`${SEARCH_URL}?q=${encodeURIComponent(query)}`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`${SEARCH_URL}: HTTP ${res.status}`);
  const data: unknown = await res.json();
  const found = new Map<string, Set<string>>();
  if (data && typeof data === 'object' && !Array.isArray(data))
    for (const [project, ids] of Object.entries(data))
      if (Array.isArray(ids)) found.set(project, new Set(ids.filter((id): id is string => typeof id === 'string')));
  return found;
}

export const WHOAMI_URL = '__atm/whoami';

/**
 * The build of the dashboard the server holds now (its /__atm/whoami), or null when it doesn't say (a server or dev
 * server without it, or any failure): a new plugin version restarts the server with a new build.
 */
export async function serverBuild(fetchFn: Fetch = browserFetch): Promise<string | null> {
  try {
    const resp = await fetchFn(WHOAMI_URL, { cache: 'no-store' });
    if (!resp.ok) return null;
    const info = (await resp.json()) as { build?: unknown };
    return typeof info.build === 'string' && info.build ? info.build : null;
  } catch {
    return null;
  }
}

/**
 * A reload brings a new dashboard when the server holds another build than this page; unknown builds never do. The
 * build the user said "Not now" to (`dismissed`) isn't offered again; another one is.
 */
export function newBuildReady(pageBuild: string | undefined, server: string | null, dismissed?: string): boolean {
  return !!pageBuild && !!server && pageBuild !== server && server !== dismissed;
}

export type LoadResult = { unchanged: true } | { unchanged: false; loaded: LoadedTasks; validators: Validators };

/** GET api/v1/projects/<id>/tasks (docs/multi-project-api.md §2.5): the tasks, each with what atm derives for it,
 * and the project's meta and size, one snapshot */
interface TaskViewList {
  meta?: TasksFile['meta'];
  bytes?: number;
  tasks?: { task: Task }[];
}

/**
 * Fetches a project's tasks (from the v1 route, with its meta), conditionally when `since` is given: a 304 means
 * unchanged, and nothing is parsed. Rejects with "HTTP <status>" for error responses and with the JSON error for
 * invalid content.
 */
export async function loadTasks(
  project: string,
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<LoadResult> {
  const file = await fetchFile(v1TasksUrl(project), fetchFn, since);
  if (file.unchanged) return file;
  const list = JSON.parse(file.text) as TaskViewList;
  const tasks = Array.isArray(list.tasks) ? list.tasks.map((view) => view.task) : [];
  return {
    unchanged: false,
    loaded: parseTasksFile({ meta: list.meta, tasks }, typeof list.bytes === 'number' ? list.bytes : file.text.length),
    validators: file.validators,
  };
}

export type ArchiveResult = { unchanged: true } | { unchanged: false; tasks: Task[]; validators: Validators };

/**
 * Fetches a project's tasks-archive.json (DM-5), conditionally when `since` is given. A missing archive (404) is an
 * empty one; an unreadable one is an error, like tasks.json.
 */
export async function loadArchive(
  project: string,
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<ArchiveResult> {
  const file = await fetchFile(projectFileUrl(project, 'tasks-archive.json'), fetchFn, since, '{"tasks": []}');
  if (file.unchanged) return file;
  const data = JSON.parse(file.text) as TasksFile;
  return { unchanged: false, tasks: Array.isArray(data.tasks) ? data.tasks : [], validators: file.validators };
}
