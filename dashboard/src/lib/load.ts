// Loading from atm serve's API (docs/multi-project-api.md): the registered projects from api/projects, each project's
// tasks from api/v1/projects/<id>/tasks, and its other files from p/<id>/. The requests go through the client layer,
// api/http.ts; this module reads what they return.
import { browserFetch, fetchFile, v1TasksUrl, type Fetch, type Validators } from './api/http';
import type { ArchiveHint, ProjectBoard, ProjectSummary } from '$generated/types';
import type { Task, TasksFile } from './types';

export { fetchProjectFile, projectFileUrl, type ProjectFile, type Validators } from './api/http';

export const DEFAULT_PROJECT_NAME = 'Task Dashboard';
export const PROJECTS_URL = '/api/projects';

// A project's summary, as the server computes it (api.schema.json, generated: T-382)
export type { BoardCard, ProjectBoard, ProjectSummary, SprintNumbers } from '$generated/types';

export type BoardColumnId = keyof ProjectBoard;

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
  /** What atm finds wrong with the task file as a whole, when it reads a malformed one as far as it can (T-377) */
  problems: string[];
  /** atm's suggestion to archive, when the task file is over meta.archive's thresholds (T-372); null otherwise */
  archiveHint: ArchiveHint | null;
}

export function parseTasksFile(
  data: TasksFile,
  bytes = 0,
  problems: string[] = [],
  archiveHint: ArchiveHint | null = null,
): LoadedTasks {
  const tasks = data.tasks ?? [];
  return {
    tasks,
    taskMap: new Map(tasks.map((t) => [t.id, t])),
    projectName: data.meta?.project || DEFAULT_PROJECT_NAME,
    meta: data.meta,
    bytes,
    problems,
    archiveHint,
  };
}

const isHint = (v: unknown): v is ArchiveHint =>
  !!v &&
  typeof v === 'object' &&
  typeof (v as ArchiveHint).bytes === 'number' &&
  typeof (v as ArchiveHint).archivable === 'number';

/** How many problems atm finds in the project's task file: the file's own and its tasks' (T-377) */
export function problemCount(problems: readonly string[], tasks: readonly Task[]): number {
  return problems.length + tasks.reduce((n, t) => n + (t.derived?.problems?.length ?? 0), 0);
}

/** A project's logo, as api/projects gives it: p/<id>/logo, relative to the server's root (where the dashboard was
 * before its pages had paths, T-429); the dashboard uses it from the root, /p/<id>/logo */
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
    logo: typeof p.logo === 'string' && LOGO_URL.test(p.logo) ? `/${p.logo}` : null,
  }));
  return { unchanged: false, projects, validators: file.validators };
}

export const SEARCH_URL = '/api/search';

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

export const WHOAMI_URL = '/__atm/whoami';

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
  tasks?: { task: Task; derived?: Task['derived'] }[];
  problems?: unknown;
  archive_hint?: unknown;
}

/** The tasks of a v1 tasks response, each as stored with what atm derives for it beside its fields (T-366) */
function viewedTasks(list: TaskViewList): Task[] {
  return Array.isArray(list.tasks)
    ? list.tasks.map((view) => (view.derived ? { ...view.task, derived: view.derived } : view.task))
    : [];
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
  const problems = Array.isArray(list.problems) ? list.problems.filter((p): p is string => typeof p === 'string') : [];
  return {
    unchanged: false,
    loaded: parseTasksFile(
      { meta: list.meta, tasks: viewedTasks(list) },
      typeof list.bytes === 'number' ? list.bytes : file.text.length,
      problems,
      isHint(list.archive_hint) ? list.archive_hint : null,
    ),
    validators: file.validators,
  };
}

export type ArchiveResult = { unchanged: true } | { unchanged: false; tasks: Task[]; validators: Validators };

/**
 * Fetches a project's archived tasks (DM-5) from the v1 tasks route (`?archived=true`, T-372), conditionally when
 * `since` is given. A missing archive (404) is an empty one; an unreadable one is an error, like the tasks.
 */
export async function loadArchive(
  project: string,
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<ArchiveResult> {
  const file = await fetchFile(`${v1TasksUrl(project)}?archived=true`, fetchFn, since, '{"tasks": []}');
  if (file.unchanged) return file;
  return { unchanged: false, tasks: viewedTasks(JSON.parse(file.text) as TaskViewList), validators: file.validators };
}
