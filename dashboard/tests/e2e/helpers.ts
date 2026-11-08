import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test as base, expect, type Page, type Route } from '@playwright/test';
import type { FilterGroup } from '../../src/lib/filters';
import { PREFS_KEY } from '../../src/lib/prefs';
import type { ViewId } from '../../src/lib/tabs';

export { expect };

export type TasksData = { meta?: Record<string, unknown>; tasks: Record<string, unknown>[] };

// atm serve's API (docs/multi-project-api.md): the project list, and each project's files under p/<id>/
const PROJECTS_JSON = /\/api\/projects(\?.*)?$/;
const TASKS_JSON = /\/p\/([^/]+)\/tasks\.json(\?.*)?$/;
const CLAIMS_JSON = /\/p\/([^/]+)\/claims\.json(\?.*)?$/;
const ARCHIVE_JSON = /\/p\/([^/]+)\/tasks-archive\.json(\?.*)?$/;
const WORKTREES_JSON = /\/p\/([^/]+)\/worktrees\.json(\?.*)?$/;
const LOGO = /\/p\/([^/]+)\/logo(\?.*)?$/;
const SEARCH = /\/api\/search(\?.*)?$/;
const WORKSPACES = /\/api\/v2\/workspaces(\?.*)?$/;
/** Two of atm's skills, for the app's agent commands (asApp) */
const AGENT_SKILLS = ['atm-do-task', 'atm-serve'];
const WORKSPACE_PROJECTS = /\/api\/v2\/workspaces\/[^/]+\/projects(\?.*)?$/;
const V1_TASKS = /\/api\/v1\/projects\/([^/]+)\/tasks(\?.*)?$/;
const V1_DOCUMENTS = /\/api\/v1\/projects\/([^/]+)\/documents\?/;
const V1_FILES = /\/api\/v1\/projects\/([^/]+)\/files\?/;

/** A file as the documents route answers it (docs/multi-project-api.md §2.6): Markdown by default, as it is */
export function projectDocument(path: string, text: string, fields: Record<string, unknown> = {}) {
  return {
    path,
    kind: 'markdown',
    language: null,
    text,
    document: text,
    line_offset: 0,
    lines: text.split('\n').length - (text.endsWith('\n') ? 1 : 0),
    bytes: text.length,
    modified_at: '2026-01-01T00:00:00Z',
    tracked: true,
    commit: 'c0ffee'.padEnd(40, '0'),
    ...fields,
  };
}

/** Task views already worked out, by the files they are of: atm runs once per version of the data */
const viewCache = new Map<string, string>();

/**
 * GET api/v1/projects/<id>/tasks for a project holding `body` as its tasks.json and `archive` as its archive
 * (docs/multi-project-api.md §2.5): its meta and size, and each task with what atm-cli itself derives for it
 * (`atm list --derived --tolerant`, read as atm serve reads it), so the dashboard is tested against atm's rules, not
 * a copy of them (T-366): a malformed file is read as far as it can be, with its problems (T-377). A body that isn't a
 * task file at all is served as it is, as atm serve refuses it, so the dashboard's error handling sees it.
 */
export function asTaskViews(body: string, archive: string | null = null, archived = false): string {
  const key = JSON.stringify([body, archive, archived]);
  const cached = viewCache.get(key);
  if (cached !== undefined) return cached;
  const read = atmViews(body, archive, archived);
  if (!read) return body;
  const views = JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', bytes: body.length, ...read });
  viewCache.set(key, views);
  return views;
}

/** atm's tolerant read of the files (`atm task list --derived --tolerant`: meta, tasks, problems; with `archived`, the
 * archive's tasks, T-372), or null when even that refuses them (not a task file) */
function atmViews(body: string, archive: string | null, archived = false): Record<string, unknown> | null {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'atm-views-'));
  try {
    mkdirSync(path.join(dir, '.agentic-task-manager'));
    writeFileSync(path.join(dir, '.agentic-task-manager/tasks.json'), body);
    if (archive !== null) writeFileSync(path.join(dir, '.agentic-task-manager/tasks-archive.json'), archive);
    const [program, ...args] = atm(['task', 'list', '--derived', '--tolerant', ...(archived ? ['--archived'] : [])]);
    const r = spawnSync(program, args, {
      cwd: dir,
      encoding: 'utf8',
      maxBuffer: 256 << 20, // thousands of archived tasks are megabytes of JSON (the default, 1 MB, cuts them off)
      env: { ...process.env, ATM_STATE_DIR: path.join(dir, '.atm-state'), GIT_CEILING_DIRECTORIES: path.dirname(dir) },
    });
    if (r.status === 0) return JSON.parse(r.stdout) as Record<string, unknown>;
    // Only what isn't a task file at all may be refused; anything else is a failure to see, not to serve around
    let data: unknown = null;
    try {
      data = JSON.parse(body);
    } catch {
      return null;
    }
    if (!data || typeof data !== 'object' || !Array.isArray((data as TasksData).tasks)) return null;
    throw new Error(`atm task list --derived failed: ${r.error?.message ?? r.stderr}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
/** The id of the project the fixture serves by default */
export const FIXTURE_PROJECT = 'fixture';

/**
 * This checkout's atm (`cargo build` first), as tests/scripts runs it (T-478), or `ATM_E2E_BIN`: the Docker screenshot
 * scripts run a Linux build of it there (`pnpm run build:atm-linux`, into `target/linux/`; T-485)
 */
const ATM = process.env.ATM_E2E_BIN
  ? path.resolve(process.env.ATM_E2E_BIN)
  : path.resolve(import.meta.dirname, `../../../target/debug/${process.platform === 'win32' ? 'atm.exe' : 'atm'}`);

/** The command line that runs atm with `args` */
function atm(args: string[]): string[] {
  return [ATM, ...args];
}

/** What atm serve would list as the summary of a project holding `body` as its tasks.json: atm-cli's own
 * (`atm project summary`), so the dashboard's Next up is tested against atm's rule, not a copy of it (T-321) */
function atmSummary(body: string): Record<string, unknown> {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'atm-summary-'));
  try {
    mkdirSync(path.join(dir, '.agentic-task-manager'));
    writeFileSync(path.join(dir, '.agentic-task-manager/tasks.json'), body);
    const [program, ...args] = atm(['project', 'summary']);
    const r = spawnSync(program, args, {
      cwd: dir,
      encoding: 'utf8',
      env: { ...process.env, ATM_STATE_DIR: path.join(dir, '.atm-state'), GIT_CEILING_DIRECTORIES: path.dirname(dir) },
    });
    if (r.status !== 0) throw new Error(`atm project summary failed: ${r.stderr}`);
    return JSON.parse(r.stdout) as Record<string, unknown>;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** A project summary as api/projects gives it (docs/multi-project-api.md 3.2), with chosen fields */
export function summary(fields: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    total: 10,
    done: 4,
    skipped: 0,
    percent: 40,
    active_sprint: null,
    recent_sprints: null,
    agents_working: 0,
    working: [],
    next: [],
    last_done: null,
    git: { branch: null, pr: null },
    needs_you: { review: 0, blocked_on_user: 0 },
    last_activity: '2026-01-01T00:00:00Z',
    completions_14d: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 1],
    ...fields,
  };
}

/** A sprint's numbers as a project's summary gives them (recent_sprints, active_sprint) */
export function sprintNumbers(
  id: string,
  done: number,
  total: number,
  state: 'planned' | 'active' | 'closed' = 'planned',
): Record<string, unknown> {
  return { id, state, due: null, release: null, done, total, percent: Math.round((100 * done) / total) };
}

/** A board card as a project's summary gives it (docs/multi-project-api.md 3.3) */
export function boardCard(id: string, name: string, fields: Record<string, unknown> = {}): Record<string, unknown> {
  return { id, name, type: 'feature', sprint: null, priority: 'P2', labels: [], ...fields };
}

type BoardColumnId = 'todo' | 'active' | 'review' | 'blocked' | 'done';

/** A project summary's board: the given columns' cards, the rest empty; `counts` says how many a column has in all
 * (more than its cards: "+ N more"), else as many as its cards */
export function board(
  columns: Partial<Record<BoardColumnId, unknown[]>>,
  counts: Partial<Record<BoardColumnId, number>> = {},
): Record<string, unknown> {
  return Object.fromEntries(
    (['todo', 'active', 'review', 'blocked', 'done'] as const).map((id) => {
      const cards = columns[id] ?? [];
      return [id, { count: counts[id] ?? cards.length, cards }];
    }),
  );
}

export function loadFixture(name: string): TasksData {
  return JSON.parse(readFileSync(new URL(`../fixtures/${name}.json`, import.meta.url), 'utf8'));
}

const toBody = (data: string | TasksData) => JSON.stringify(typeof data === 'string' ? loadFixture(data) : data);

/**
 * Plays atm serve for the dashboard: api/projects lists one project by default (its id is the served data's meta.id,
 * else FIXTURE_PROJECT), more with addProject(), and each project's tasks.json, claims.json, tasks-archive.json and
 * worktrees.json (the last three a 404 by default) are served under p/<id>/.
 */
export class TasksServer {
  private body: string | null = null;
  private status = 200;
  private claimsBody: string | null = null;
  private archiveBody: string | null = null;
  private worktreesBody: string | null = null;
  /** Projects besides the default one: their tasks.json and summary, by id */
  private others = new Map<string, { name: string; body: string; summary: Record<string, unknown> | null }>();
  /** Registered projects whose folder is gone: their names, by id */
  private gone = new Map<string, string>();
  /** Projects with a logo: the SVG served at p/<id>/logo, by id; null lists one that fails to load */
  private logos = new Map<string, string | null>();
  private listed = true;
  /** Whether api/projects gives the default project atm's own summary of what it serves (withAtmSummary) */
  private summarized = false;
  /** How many times tasks-archive.json was fetched (it is loaded only when a view needs it) */
  archiveRequests = 0;
  // Like atm serve: each version of the file has an ETag, and a request that already has it gets a 304 (D-5.1)
  private version = 0;
  requests = 0;
  /** How many requests were answered 304 Not Modified */
  notModified = 0;
  /** The queries api/search was asked, in order */
  searches: string[] = [];
  /** The default project's files for the document viewer, by path: the documents route's answer, or a refusal */
  private documents = new Map<string, { status: number; body: unknown; version: number }>();
  /** The paths the documents route was asked for, in order, and how many were answered 304 */
  documentRequests: string[] = [];
  documentsNotModified = 0;

  /** Serve a file of the default project through the documents route (a new version when it is set again) */
  setDocument(doc: { path: string } & Record<string, unknown>) {
    const version = (this.documents.get(doc.path)?.version ?? 0) + 1;
    this.documents.set(doc.path, { status: 200, body: doc, version });
  }

  /** The default project's image files the files route serves, by path; the paths it was asked for, in order */
  private images = new Map<string, { contentType: string; body: Buffer }>();
  fileRequests: string[] = [];

  /** Serve an image of the default project through the files route */
  setImage(path: string, contentType: string, body: Buffer) {
    this.images.set(path, { contentType, body });
  }

  /** Answer the documents route for `path` with an error, as atm serve refuses a file */
  refuseDocument(path: string, status: number, error: string) {
    this.documents.set(path, { status, body: { error }, version: 0 });
  }

  constructor(private page: Page) {}

  private get meta(): Record<string, unknown> {
    return (this.body && this.status === 200 ? (JSON.parse(this.body) as TasksData).meta : undefined) ?? {};
  }

  /** The default project's id: its meta.id, as atm serve uses, else FIXTURE_PROJECT */
  get projectId(): string {
    return typeof this.meta.id === 'string' && this.meta.id ? this.meta.id : FIXTURE_PROJECT;
  }

  /** The project list api/projects answers with (summaries are null until a test needs them) */
  projectList() {
    if (!this.listed) return [];
    const project = (id: string, name: string, summary: Record<string, unknown> | null = null) => ({
      id,
      name,
      path: `/projects/${id}`,
      missing: false,
      summary,
      logo: this.logos.has(id) ? `p/${id}/logo` : null,
    });
    const own = this.summarized && this.body && this.status === 200 ? atmSummary(this.body) : null;
    return [
      project(this.projectId, typeof this.meta.project === 'string' ? this.meta.project : this.projectId, own),
      ...[...this.others].map(([id, p]) => project(id, p.name, p.summary)),
      ...[...this.gone].map(([id, name]) => ({ ...project(id, name), missing: true })),
    ];
  }

  /** Workspaces besides `default` (api/v2/workspaces), and the one opened last; their projects aren't served */
  private workspaceList: string[] = [];
  private recentWorkspace = 'default';

  /** List more workspaces than `default` (the switcher shows), and which one was opened last (`/` opens it) */
  workspaces(ids: string[], recent = 'default') {
    this.workspaceList = ids;
    this.recentWorkspace = recent;
    return this;
  }

  async install() {
    await this.page.route(WORKSPACES, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          generated_at: '2026-01-01T00:00:00Z',
          workspaces: [
            { id: 'default', projects: this.projectList().length },
            ...this.workspaceList.map((id) => ({ id, projects: 0 })),
          ],
          recent: this.recentWorkspace,
        }),
      }),
    );
    // Another workspace's list: none of the fixture's projects are in it
    await this.page.route(WORKSPACE_PROJECTS, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', projects: [] }),
      }),
    );
    await this.page.route(PROJECTS_JSON, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', projects: this.projectList() }),
      }),
    );
    // A project's tasks: tasks.json as stored, and the v1 route the dashboard loads (docs/multi-project-api.md §2.5),
    // the same body as views, with the same ETag, status and counters
    const answerTasks = (route: Route, pattern: RegExp, wrap: (body: string) => string) => {
      const id = decodeURIComponent(pattern.exec(route.request().url())![1]);
      if (id !== this.projectId) {
        const other = this.others.get(id);
        return other
          ? route.fulfill({ status: 200, contentType: 'application/json', body: wrap(other.body) })
          : route.fulfill({ status: 404, contentType: 'application/json', body: '{"error": "unknown project"}' });
      }
      this.requests++;
      const etag = `"v${this.version}"`;
      if (this.status === 200 && route.request().headers()['if-none-match'] === etag) {
        this.notModified++;
        return route.fulfill({ status: 304, headers: { ETag: etag } });
      }
      const headers = this.status === 200 ? { ETag: etag } : undefined;
      const body = this.status === 200 ? wrap(this.body ?? '') : (this.body ?? '');
      return route.fulfill({ status: this.status, contentType: 'application/json', body, headers });
    };
    await this.page.route(TASKS_JSON, (route) => answerTasks(route, TASKS_JSON, (body) => body));
    await this.page.route(V1_TASKS, (route) => {
      // ?archived=true: the archive's tasks (T-372), as tasks-archive.json was served: counted, a 404 without one
      if (new URL(route.request().url()).searchParams.get('archived') === 'true') {
        const mine = decodeURIComponent(V1_TASKS.exec(route.request().url())![1]) === this.projectId;
        if (mine) this.archiveRequests++;
        return this.archiveBody === null || !mine || this.body === null
          ? route.fulfill({ status: 404, body: 'not found' })
          : route.fulfill({
              status: 200,
              contentType: 'application/json',
              body: asTaskViews(this.body, this.archiveBody, true),
            });
      }
      return answerTasks(route, V1_TASKS, (body) => asTaskViews(body, body === this.body ? this.archiveBody : null));
    });
    const isDefault = (url: string, pattern: RegExp) => decodeURIComponent(pattern.exec(url)![1]) === this.projectId;
    await this.page.route(ARCHIVE_JSON, (route) => {
      const mine = isDefault(route.request().url(), ARCHIVE_JSON);
      if (mine) this.archiveRequests++;
      return this.archiveBody === null || !mine
        ? route.fulfill({ status: 404, body: 'not found' })
        : route.fulfill({ status: 200, contentType: 'application/json', body: this.archiveBody });
    });
    await this.page.route(CLAIMS_JSON, (route) =>
      this.claimsBody === null || !isDefault(route.request().url(), CLAIMS_JSON)
        ? route.fulfill({ status: 404, body: 'not found' })
        : route.fulfill({ status: 200, contentType: 'application/json', body: this.claimsBody }),
    );
    await this.page.route(WORKTREES_JSON, (route) =>
      this.worktreesBody === null || !isDefault(route.request().url(), WORKTREES_JSON)
        ? route.fulfill({ status: 404, body: 'not found' })
        : route.fulfill({ status: 200, contentType: 'application/json', body: this.worktreesBody }),
    );
    await this.page.route(SEARCH, (route) => {
      const q = new URL(route.request().url()).searchParams.get('q') ?? '';
      this.searches.push(q);
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(this.search(q)) });
    });
    await this.page.route(V1_DOCUMENTS, (route) => {
      const url = new URL(route.request().url());
      const path = url.searchParams.get('path') ?? '';
      this.documentRequests.push(path);
      const found = isDefault(route.request().url(), V1_DOCUMENTS) ? this.documents.get(path) : undefined;
      if (!found) {
        return route.fulfill({ status: 404, contentType: 'application/json', body: '{"error": "not in the project"}' });
      }
      const etag = `"d${found.version}"`;
      if (found.status === 200 && route.request().headers()['if-none-match'] === etag) {
        this.documentsNotModified++;
        return route.fulfill({ status: 304, headers: { ETag: etag } });
      }
      return route.fulfill({
        status: found.status,
        contentType: 'application/json',
        body: JSON.stringify(found.body),
        headers: found.status === 200 ? { ETag: etag } : undefined,
      });
    });
    await this.page.route(V1_FILES, (route) => {
      const path = new URL(route.request().url()).searchParams.get('path') ?? '';
      this.fileRequests.push(path);
      const image = isDefault(route.request().url(), V1_FILES) ? this.images.get(path) : undefined;
      return image
        ? route.fulfill({ status: 200, contentType: image.contentType, body: image.body })
        : route.fulfill({ status: 404, contentType: 'application/json', body: '{"error": "not in the project"}' });
    });
    await this.page.route(LOGO, (route) => {
      const svg = this.logos.get(decodeURIComponent(LOGO.exec(route.request().url())![1]));
      return svg
        ? route.fulfill({ status: 200, contentType: 'image/svg+xml', body: svg })
        : route.fulfill({ status: 404, contentType: 'application/json', body: '{"error": "no logo"}' });
    });
  }

  /** api/search's answer (docs/multi-project-api.md 2.4): per project, the IDs of the tasks in which every word of
   * `q` appears in the ID, name, description, type, labels or notes, or the project's name */
  private search(q: string): Record<string, string[]> {
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return {};
    const projects = [
      { id: this.projectId, name: String(this.meta.project ?? this.projectId), body: this.body ?? '{"tasks":[]}' },
      ...[...this.others].map(([id, p]) => ({ id, name: p.name, body: p.body })),
    ];
    const found: Record<string, string[]> = {};
    for (const p of projects) {
      const ids = (JSON.parse(p.body) as TasksData).tasks
        .filter((t) => {
          const notes = Array.isArray(t.notes) ? t.notes.map((n) => (n as { text?: string }).text) : [];
          const text = [p.name, t.id, t.name, t.description, t.type, ...((t.labels as string[]) ?? []), ...notes]
            .join(' ')
            .toLowerCase();
          return words.every((w) => text.includes(w));
        })
        .map((t) => String(t.id));
      if (ids.length) found[p.id] = ids;
    }
    return found;
  }

  /** Give a project a logo, an SVG served at p/<id>/logo; null lists a logo that fails to load (a 404) */
  logo(id: string, svg: string | null) {
    this.logos.set(id, svg);
    return this;
  }

  /** Serve worktrees.json (the project's git worktrees, main first, linked ones with their differing tasks); null
   * makes it a 404 (no linked worktrees) again. */
  worktrees(worktrees: Record<string, unknown>[] | null) {
    this.worktreesBody =
      worktrees === null ? null : JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', worktrees });
    return this;
  }

  /** The paths of requests the page made to the server's routes (api/, p/), when it plays the app (asApp) */
  httpRequests: string[] = [];
  /** The app's agent commands the page called, in order (`agents_status`, `connect_agents <harness>`) */
  agentCalls: string[] = [];
  /** Whether the app's Claude Code has atm's skills (connect_agents sets it) */
  private agentsConnected = false;

  /** Claude Code's status, as `agents_status` reports it */
  private harnessStatus() {
    return {
      harness: 'claude',
      label: 'Claude Code',
      detected: true,
      scope: 'user',
      folder: '/Users/me/.claude/skills',
      version: '0.0.0',
      skills: AGENT_SKILLS.map((skill) => ({
        skill,
        path: `/Users/me/.claude/skills/${skill}`,
        state: this.agentsConnected ? 'linked' : 'missing',
        version: null,
      })),
    };
  }

  /**
   * Play the atm app instead of atm serve (T-457): the page runs as in Tauri's window (`isTauri`, Tauri's IPC
   * internals mocked, as `@tauri-apps/api/mocks` does), and the app's commands answer from the same data as the routes
   * (the default workspace's projects, their tasks as atm's views, claims, worktrees, search, logos). Requests the page
   * still makes to the server's routes are recorded in `httpRequests`: the app has none to answer them.
   */
  async asApp() {
    this.page.on('request', (r) => {
      const { pathname } = new URL(r.url());
      if (/^\/(api|p)\//.test(pathname)) this.httpRequests.push(pathname);
    });
    await this.page.exposeFunction('__atmInvoke', (command: string, args: Record<string, unknown>) =>
      this.answerCommand(command, args),
    );
    await this.page.addInitScript(() => {
      const w = window as unknown as Record<string, unknown> & {
        __atmInvoke: (command: string, args: unknown) => Promise<{ ok?: unknown; error?: unknown }>;
      };
      w.isTauri = true;
      w.__TAURI_INTERNALS__ = {
        invoke: (command: string, args: unknown) =>
          w.__atmInvoke(command, args ?? {}).then((r) => ('error' in r ? Promise.reject(r.error) : r.ok)),
        transformCallback: () => 0,
        unregisterCallback: () => {},
        convertFileSrc: (path: string) => path,
        metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
      };
    });
    return this;
  }

  /** What the app's command answers: `{ok}`, or `{error}` as its `AppError` */
  private answerCommand(command: string, args: Record<string, unknown>): { ok?: unknown; error?: unknown } {
    const unknown = { error: { kind: 'not_found', message: 'unknown project' } };
    const views = (text: string) => {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      delete parsed.generated_at;
      return parsed;
    };
    const project = String(args.project ?? '');
    const body = project === this.projectId ? this.body : this.others.get(project)?.body;
    switch (command) {
      case 'list_workspaces':
        return { ok: { workspaces: [{ id: 'default', projects: this.projectList().length }], recent: 'default' } };
      case 'list_projects':
        return { ok: { projects: this.projectList() } };
      case 'search':
        return { ok: this.search(String(args.q ?? '')) };
      case 'project_tasks': {
        if (!body) return unknown;
        const archived = (args.filters as { archived?: boolean } | undefined)?.archived;
        if (archived)
          return this.archiveBody === null || project !== this.projectId
            ? { error: { kind: 'not_found', message: 'not found' } }
            : { ok: views(asTaskViews(body, this.archiveBody, true)) };
        return { ok: views(asTaskViews(body, project === this.projectId ? this.archiveBody : null)) };
      }
      case 'claims':
        return body ? { ok: { claims: this.claimsBody ? JSON.parse(this.claimsBody).claims : [] } } : unknown;
      case 'worktrees':
        return body
          ? { ok: { worktrees: this.worktreesBody ? JSON.parse(this.worktreesBody).worktrees : [] } }
          : unknown;
      case 'agents_status':
        this.agentCalls.push(command);
        return { ok: [this.harnessStatus()] };
      case 'connect_agents': {
        this.agentCalls.push(`${command} ${String(args.harness)}`);
        if (args.harness !== 'claude')
          return { error: { kind: 'invalid', message: `no harness '${String(args.harness)}'` } };
        const before = this.agentsConnected;
        this.agentsConnected = true;
        return {
          ok: {
            written: AGENT_SKILLS.map((skill) => ({
              harness: 'claude',
              skill,
              path: `/Users/me/.claude/skills/${skill}`,
              how: 'link',
              action: before ? 'unchanged' : 'installed',
            })),
            refused: [],
          },
        };
      }
      case 'logo': {
        const svg = this.logos.get(project);
        return svg ? { ok: { content_type: 'image/svg+xml', data: [...Buffer.from(svg)] } } : { ok: null };
      }
      default:
        return { error: { kind: 'not_found', message: `no command ${command}` } };
    }
  }

  /** Register another project, served under p/<id>/ (a fixture by name or inline tasks), with an optional summary
   * (summary() makes one) for api/projects. */
  addProject(id: string, data: string | TasksData, name = id, summary: Record<string, unknown> | null = null) {
    this.others.set(id, { name, body: toBody(data), summary });
    asTaskViews(toBody(data)); // worked out now, not during a request
    return this;
  }

  /** Give the default project the summary atm serve would: atm-cli's own, of what is served at each request (its
   * `next` is the Kanban's Next up). Off by default, so the overview shows the default project without numbers. */
  withAtmSummary() {
    this.summarized = true;
    return this;
  }

  /** Answer __atm/whoami with this dashboard build, as a server restarted on a new plugin version would. */
  async serverBuild(build: string) {
    await this.page.route(/\/__atm\/whoami$/, (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ atm: true, build }) }),
    );
    return this;
  }

  /** List a registered project whose folder is gone (missing, no summary, no files). */
  goneProject(id: string, name = id) {
    this.gone.set(id, name);
    return this;
  }

  /** api/projects lists nothing, as for a user who hasn't registered a project yet. */
  noProjects() {
    this.listed = false;
    return this;
  }

  /** Serve claims.json (the `atm` claims snapshot); null makes it a 404 again. */
  claims(claims: Record<string, unknown>[] | null) {
    this.claimsBody = claims === null ? null : JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', claims });
    return this;
  }

  /** Serve tasks-archive.json (a fixture by name or inline tasks); null makes it a 404 (no archive) again. */
  archive(data: string | TasksData | null) {
    this.archiveBody = data === null ? null : toBody(data);
    this.warm();
    return this;
  }

  /** Serve a fixture by name (tests/fixtures/<name>.json) or an inline object, as the default project's tasks.json. */
  use(data: string | TasksData) {
    this.body = toBody(data);
    this.status = 200;
    this.version++;
    this.warm();
    return this;
  }

  /** Work out the task views now, when the data is set, so no request waits for atm (the timing tests measure them) */
  private warm() {
    if (this.body !== null) asTaskViews(this.body, this.archiveBody);
    if (this.body !== null && this.archiveBody !== null) asTaskViews(this.body, this.archiveBody, true);
  }

  fail(status = 404) {
    this.status = status;
    this.body = 'not found';
    this.version++;
    return this;
  }
}

type Fixtures = { tasksServer: TasksServer; externalRequestGuard: void; cspGuard: void; sandboxRefusals: string[] };

export const test = base.extend<Fixtures>({
  tasksServer: async ({ page }, use) => {
    const server = new TasksServer(page);
    await server.install();
    await use(server);
  },

  // The dashboard promises zero external dependencies: any non-localhost request fails the test.
  externalRequestGuard: [
    async ({ page }, use) => {
      const external: string[] = [];
      page.on('request', (req) => {
        const { protocol, hostname } = new URL(req.url());
        if ((protocol === 'http:' || protocol === 'https:') && hostname !== 'localhost') external.push(req.url());
      });
      await use();
      expect(external, 'external requests').toEqual([]);
    },
    { auto: true },
  ],

  // The page runs under the Content-Security-Policy atm serve sends (serve.mjs sends the same, T-226): anything it
  // blocks (an inline script, a style, an image, a request) fails the test, so the policy and the dashboard agree.
  // CompMD's sandbox frames (T-285) have a policy of their own, which refuses what untrusted text tries there: those
  // refusals are the sandbox working, kept apart in `sandboxRefusals` for a test to look at.
  cspGuard: [
    async ({ page, sandboxRefusals }, use) => {
      const violations: string[] = [];
      page.on('console', (msg) => {
        if (!/Content Security Policy/i.test(msg.text())) return;
        if (msg.location().url.includes('/assets/compmd/')) sandboxRefusals.push(msg.text());
        else violations.push(msg.text());
      });
      await use();
      expect(violations, 'Content-Security-Policy violations').toEqual([]);
    },
    { auto: true },
  ],
  sandboxRefusals: async ({}, use) => {
    await use([]);
  },
});

/**
 * Serve `data` as tasks.json and open the dashboard.
 * Auto-refresh is switched off (unless `autoRefresh: true`) so background reloads can't interfere with what a
 * test checks. It is only set when no preference is stored yet, so a test that changes it and reloads keeps its choice.
 */
export async function openDashboard(
  page: Page,
  tasksServer: TasksServer,
  data: string | TasksData,
  {
    autoRefresh = false,
    tab,
    graphAutoFit = true,
  }: {
    autoRefresh?: boolean;
    tab?: ViewId;
    /** false: the project's view starts with the graph's auto-fit off (T-341), as before it existed */
    graphAutoFit?: boolean;
  } = {},
) {
  if (!autoRefresh) {
    await page.addInitScript((key) => {
      try {
        const prefs = JSON.parse(localStorage.getItem(key) || '{}');
        if (!('autoRefresh' in prefs)) localStorage.setItem(key, JSON.stringify({ ...prefs, autoRefresh: false }));
      } catch {
        // storage unavailable: the dashboard falls back to its defaults
      }
    }, PREFS_KEY);
  }
  tasksServer.use(data);
  if (!graphAutoFit) {
    // The project's saved view (view-store.ts projectKey: the server's project id)
    await page.addInitScript((key) => {
      try {
        const view = JSON.parse(localStorage.getItem(key) || '{}');
        if (!('graphAutoFit' in view)) localStorage.setItem(key, JSON.stringify({ ...view, graphAutoFit: false }));
      } catch {
        // storage unavailable: the dashboard falls back to its defaults
      }
    }, `atm-dashboard:view:id:${tasksServer.projectId}`);
  }
  await page.goto(tab ? `/projects/${encodeURIComponent(tasksServer.projectId)}/${tab}` : '/');
}

/** Follow a link to `address` in the page (`/projects/fixture/list?doc=docs/x.md`), as a link in task text or another
 * page does: the dashboard moves there without loading again (T-429) */
export async function follow(page: Page, address: string) {
  await page.evaluate((href) => {
    const link = document.createElement('a');
    link.href = href;
    document.body.append(link);
    link.click();
    link.remove();
  }, address);
}

/** The page's address as the dashboard writes it: its path and query (`/projects/fixture/list?about`) */
export function addressOf(page: Page): string {
  const url = new URL(page.url());
  return url.pathname + url.search;
}

/** Open the filter panel (Phase, Type and Status buttons) under the search line, if it isn't open yet. */
export async function openFilters(page: Page) {
  const toggle = page.locator('#filters-toggle');
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
  await expect(page.locator('#filter-panel')).toBeVisible();
}

/** Close the filter popup (Escape), as a click outside it would, so what's under it can be clicked. */
export async function closeFilters(page: Page) {
  // A click outside it, on the title; not Escape, which in the search box also clears the search
  if ((await page.locator('#filters-toggle').getAttribute('aria-expanded')) === 'true')
    await page.locator('#dashboard-title').click();
  await expect(page.locator('#filter-panel')).toBeHidden();
}

/** "N of M tasks": how many tasks the search and filters leave, at the top of the filter popup (T-323). */
export function matchCount(page: Page) {
  return page.locator('#match-count');
}

/** Open a menu in the top bar (refresh, appearance) unless it is open already. */
export async function openMenu(page: Page, button: string) {
  const b = page.locator(button);
  if ((await b.getAttribute('aria-expanded')) !== 'true') await b.click();
}

export function filterButton(page: Page, group: FilterGroup, value: string) {
  return page.locator(`.filter-btn[data-filter="${group}"][data-value="${value}"]`);
}

/** IDs of the task cards currently shown, in display order. */
export function visibleCardIds(page: Page) {
  return page.locator('.task-card').evaluateAll((cards) => cards.map((c) => (c as HTMLElement).dataset.id));
}
