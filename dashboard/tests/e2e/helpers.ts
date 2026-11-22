import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test as base, expect, type Page, type Route } from '@playwright/test';
import { compmdVersion } from '../../build/compmd';
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
/** A project's reviews, or one review's page (ATM-574) */
const V1_REVIEWS = /\/api\/v1\/projects\/([^/]+)\/reviews(?:\/([^/?]+))?(?:\?.*)?$/;
/** The review routes that change something, always v2 (ATM-572): the project, and the path past `reviews` */
const V2_REVIEW_CHANGES = /\/api\/v2\/workspaces\/[^/]+\/projects\/([^/]+)\/reviews(\/[^?]*)?(?:\?.*)?$/;

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
export function asTaskViews(
  body: string,
  archive: string | null = null,
  archived = false,
  registered: readonly string[] = [],
): string {
  const key = JSON.stringify([body, archive, archived, registered, atmNow]);
  const cached = viewCache.get(key);
  if (cached !== undefined) return cached;
  const read = atmViews(body, archive, archived, registered);
  if (!read) return body;
  const views = JSON.stringify({ generated_at: '2026-01-01T00:00:00Z', bytes: body.length, ...read });
  viewCache.set(key, views);
  return views;
}

/** The time atm counts by in this test (its `ATM_NOW`, which a debug build heeds), or the real time: what atm says by
 * the day (a sprint's days to its due date, ATM-720) is then as of the page's fixed clock. Reset after each test */
let atmNow: string | undefined;

/** Run the fixture's atm at `at` (a spec that fixes the page's clock, `page.clock.setFixedTime`, sets it too) */
export function atmNowFor(at: Date | undefined) {
  atmNow = at ? at.toISOString().replace(/\.\d{3}Z$/, 'Z') : undefined;
}

/** The environment atm runs in for the fixture: its own state folder, and the test's clock */
const atmEnv = (dir: string) => ({
  ...process.env,
  ATM_STATE_DIR: path.join(dir, '.atm-state'),
  GIT_CEILING_DIRECTORIES: os.tmpdir(),
  ...(atmNow ? { ATM_NOW: atmNow } : {}),
});

/** atm's tolerant read of the files (`atm task list --derived --tolerant`: meta, tasks, problems; with `archived`, the
 * archive's tasks, T-372), or null when even that refuses them (not a task file). The `registered` projects' task
 * files are registered in atm's state for the read, so it finds their tasks a reference names (ADR-007 §7, T-549) */
function atmViews(
  body: string,
  archive: string | null,
  archived = false,
  registered: readonly string[] = [],
): Record<string, unknown> | null {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'atm-views-'));
  const env = atmEnv(dir);
  const others: string[] = [];
  try {
    mkdirSync(path.join(dir, '.agentic-task-manager'));
    writeFileSync(path.join(dir, '.agentic-task-manager/tasks.json'), body);
    if (archive !== null) writeFileSync(path.join(dir, '.agentic-task-manager/tasks-archive.json'), archive);
    for (const other of registered) {
      const folder = mkdtempSync(path.join(os.tmpdir(), 'atm-other-'));
      others.push(folder);
      mkdirSync(path.join(folder, '.agentic-task-manager'));
      writeFileSync(path.join(folder, '.agentic-task-manager/tasks.json'), other);
      const [program, ...args] = atm(['workspace', 'add-project', 'default', folder]);
      const added = spawnSync(program, args, { cwd: dir, encoding: 'utf8', env });
      if (added.status !== 0) throw new Error(`atm workspace add-project failed: ${added.stderr}`);
    }
    const [program, ...args] = atm(['task', 'list', '--derived', '--tolerant', ...(archived ? ['--archived'] : [])]);
    const r = spawnSync(program, args, {
      cwd: dir,
      encoding: 'utf8',
      maxBuffer: 256 << 20, // thousands of archived tasks are megabytes of JSON (the default, 1 MB, cuts them off)
      env,
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
    for (const folder of [dir, ...others]) rmSync(folder, { recursive: true, force: true });
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
      env: { ...atmEnv(dir), GIT_CEILING_DIRECTORIES: path.dirname(dir) },
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

interface CommentFake {
  id: string;
  author: string;
  resolved: boolean | null;
  poke?: boolean;
  editable?: boolean;
  replies?: CommentFake[];
}
type Answer = { ok: boolean; why?: string };
export interface ReviewPageFake {
  review: { status: string };
  you: string;
  chains: { comments: CommentFake[] }[];
  can?: Record<string, Answer>;
}

/** atm's answers on a review's page (ATM-633, ATM-723), as the fixture plays atm: what its reader may do (`can`:
 * nothing on an ended review; Approve not while a comment is open; sent back only while waiting; rounds the author's,
 * whom the fixture's reader never is) and which comments they may edit (`editable`: their own, not a poke, the review
 * not ended). What the page gives is kept: a test's own `can` answers, its own `editable` */
function withAtmAnswers(reference: string, page: ReviewPageFake): ReviewPageFake {
  const status = page.review.status;
  const ended = status === 'approved' || status === 'closed';
  const all = (cs: CommentFake[]): CommentFake[] => cs.flatMap((c) => [c, ...all(c.replies ?? [])]);
  const open = page.chains.flatMap((ch) => all(ch.comments)).filter((c) => c.resolved === false);
  const no = (why: string): Answer => ({ ok: false, why });
  const actions = ['approve', 'request_changes', 'comment', 'round', 'close'];
  const can: Record<string, Answer> = ended
    ? Object.fromEntries(actions.map((a) => [a, no(`${reference} is ${status}: it can’t be changed`)]))
    : {
        approve: open.length
          ? no(`${reference} can’t be approved while comments are open: ${open.map((c) => c.id).join(', ')}`)
          : { ok: true },
        request_changes:
          status === 'waiting'
            ? { ok: true }
            : no(`${reference} is ${status.replace('_', ' ')}: it can’t be sent back`),
        comment: { ok: true },
        round: no(`only ${reference}’s author adds its rounds`),
        close: { ok: true },
      };
  const mark = (c: CommentFake): CommentFake => ({
    ...c,
    editable: c.editable ?? (!ended && !c.poke && c.author === page.you),
    replies: (c.replies ?? []).map(mark),
  });
  return {
    ...page,
    can: { ...can, ...page.can },
    // In the view (ATM-725) unless the test says it is on another round: the fixture serves one page for every view
    chains: page.chains.map((ch) => ({ in_view: true, ...ch, comments: ch.comments.map(mark) })),
  };
}

/** A reviews list with atm's `ended` on each entry (ATM-714), as the fixture plays atm: approved or closed, unless the
 * entry gives its own */
function withEnded(list: unknown): unknown {
  const reviews = (list as { reviews?: { review: { status: string }; ended?: boolean }[] } | null)?.reviews;
  if (!Array.isArray(reviews)) return list;
  return {
    ...(list as object),
    reviews: reviews.map((e) => ({ ended: ['approved', 'closed'].includes(e.review.status), ...e })),
  };
}

/** A channel's ID in an app command's arguments: `__CHANNEL__:<id>` as Tauri sends it, or the Channel itself (its
 * `id`) as Playwright passes it */
function channelId(channel: unknown): number {
  return Number(typeof channel === 'string' ? channel.split(':')[1] : (channel as { id?: unknown } | null)?.id);
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
  /** Projects atm itself finds registered when it works out the default project's task views (ADR-007 §7): their
   * tasks.json. The page isn't told of them: a reference to one of their tasks is shown from what atm read there */
  private registeredBodies: string[] = [];
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

  /** Serve an image of the default project through the files route; with `at`, as that revision has it (`&at=`) */
  setImage(path: string, contentType: string, body: Buffer, at?: string) {
    this.images.set(at ? `${at}:${path}` : path, { contentType, body });
  }

  /** Answer the documents route for `path` with an error, as atm serve refuses a file */
  refuseDocument(path: string, status: number, error: string) {
    this.documents.set(path, { status, body: { error }, version: 0 });
  }

  // The default project's reviews (ATM-574): the list, each review's page by reference (for any view: the view asked
  // for is recorded), each round's sides' files by revision, and the changes the page sent
  private reviewList: unknown = { reviews: [], you: 'Ana' };
  private reviewPages = new Map<string, unknown>();
  private revisionDocuments = new Map<string, unknown>();
  /** The reviews list's queries and the reviews' pages asked for (`CR-7?round=2&against=1`), in order */
  reviewRequests: string[] = [];
  /** The changes the page sent: method, path past `reviews`, body, and its session token */
  reviewChanges: { method: string; path: string; body: unknown; session: string | undefined }[] = [];
  /** What the next changes are answered with: their status and body */
  private reviewChangeAnswer: { status: number; body: unknown } = { status: 201, body: { comment: 'c9' } };

  /** Serve the reviews list (`GET …/reviews`) */
  setReviews(list: unknown) {
    this.reviewList = withEnded(list);
    return this;
  }

  /** Another project's reviews list (ATM-575, the overview's Reviews over every project); one without is a 404 */
  private projectReviewLists = new Map<string, unknown>();
  setProjectReviews(project: string, list: unknown) {
    this.projectReviewLists.set(project, withEnded(list));
    return this;
  }

  /** Serve review `reference`'s page (`GET …/reviews/<CR>`), for any round, with atm's answers on it (`can`, each
   * comment's `editable`) unless the page gives its own */
  setReview(reference: string, page: unknown) {
    this.reviewPages.set(reference, withAtmAnswers(reference, page as ReviewPageFake));
    return this;
  }

  /** Refuse review `reference`'s page as atm serve does (a reference two reviews share is a 400 naming both) */
  refuseReview(reference: string, status: number, error: string) {
    this.reviewRefusals.set(reference, { status, error });
    return this;
  }
  private reviewRefusals = new Map<string, { status: number; error: string }>();

  /** Serve a file as revision `revision` has it (`documents?path=&at=`) */
  setRevisionDocument(revision: string, doc: { path: string } & Record<string, unknown>) {
    this.revisionDocuments.set(`${revision}:${doc.path}`, doc);
    return this;
  }

  /** Answer the page's next review changes with `status` and `body` (a refusal, say) */
  answerReviewChanges(status: number, body: unknown) {
    this.reviewChangeAnswer = { status, body };
    return this;
  }

  constructor(private page: Page) {}

  private get meta(): Record<string, unknown> {
    return (this.body && this.status === 200 ? (JSON.parse(this.body) as TasksData).meta : undefined) ?? {};
  }

  /** The default project's id: its meta.id, as atm serve uses, else FIXTURE_PROJECT */
  get projectId(): string {
    return typeof this.meta.id === 'string' && this.meta.id ? this.meta.id : FIXTURE_PROJECT;
  }

  /** A summary of the default project chosen by the test (summary() makes one), over atm's own or none */
  private ownSummary: Record<string, unknown> | null = null;
  withOwnSummary(summary: Record<string, unknown>) {
    this.ownSummary = summary;
    return this;
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
    const own = this.ownSummary ?? (this.summarized && this.body && this.status === 200 ? atmSummary(this.body) : null);
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
      return answerTasks(route, V1_TASKS, (body) =>
        body === this.body
          ? this.withReviewStatuses(asTaskViews(body, this.archiveBody, false, this.registeredBodies))
          : asTaskViews(body),
      );
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
    await this.page.route(V1_REVIEWS, (route) => {
      const url = new URL(route.request().url());
      const [, id, review] = V1_REVIEWS.exec(route.request().url())!;
      const other = this.projectReviewLists.get(decodeURIComponent(id));
      if (decodeURIComponent(id) !== this.projectId && other && !review) {
        this.reviewRequests.push(`${decodeURIComponent(id)}:${url.search}`);
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(other) });
      }
      if (decodeURIComponent(id) !== this.projectId)
        return route.fulfill({ status: 404, contentType: 'application/json', body: '{"error": "unknown project"}' });
      this.reviewRequests.push(`${review ? decodeURIComponent(review) : ''}${url.search}`);
      const refused = review ? this.reviewRefusals.get(decodeURIComponent(review)) : undefined;
      if (refused)
        return route.fulfill({
          status: refused.status,
          contentType: 'application/json',
          body: JSON.stringify({ error: refused.error }),
        });
      const found = review ? this.reviewPages.get(decodeURIComponent(review)) : this.reviewList;
      return found
        ? route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ generated_at: '2026-11-17T10:00:00Z', ...(found as object) }),
          })
        : route.fulfill({
            status: 404,
            contentType: 'application/json',
            body: JSON.stringify({ error: `no review ${decodeURIComponent(review)}` }),
          });
    });
    await this.page.route(V2_REVIEW_CHANGES, (route) => {
      const request = route.request();
      if (!['POST', 'PATCH', 'PUT', 'DELETE'].includes(request.method())) return route.fallback();
      this.reviewChanges.push({
        method: request.method(),
        path: V2_REVIEW_CHANGES.exec(request.url())![2] ?? '',
        body: request.postDataJSON(),
        session: request.headers()['x-atm-session'],
      });
      return route.fulfill({
        status: this.reviewChangeAnswer.status,
        contentType: 'application/json',
        body: JSON.stringify(this.reviewChangeAnswer.body),
      });
    });
    await this.page.route(V1_DOCUMENTS, (route) => {
      const url = new URL(route.request().url());
      const path = url.searchParams.get('path') ?? '';
      const revision = url.searchParams.get('at');
      if (revision) {
        this.documentRequests.push(`${path}@${revision}`);
        const doc = this.revisionDocuments.get(`${revision}:${path}`);
        return doc
          ? route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(doc) })
          : route.fulfill({
              status: 404,
              contentType: 'application/json',
              body: JSON.stringify({ error: `${path}: not a file in ${revision}` }),
            });
      }
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
      const query = new URL(route.request().url()).searchParams;
      const path = query.get('path') ?? '';
      const at = query.get('at');
      this.fileRequests.push(at ? `${path}@${at}` : path);
      const image = isDefault(route.request().url(), V1_FILES)
        ? this.images.get(at ? `${at}:${path}` : path)
        : undefined;
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
  /** The address of the deep link the app was started with (link), and the page's channel for later ones */
  private startLink: string | null = null;
  private linkChannel: number | null = null;
  private linkMessages = 0;

  /** The app was started by a deep link: its address, as the app makes it from `atm://…` (T-459) */
  link(address: string) {
    this.startLink = address;
    return this;
  }

  /** A deep link while the app runs: its address on the page's channel */
  async pushLink(address: string) {
    if (this.linkChannel === null) throw new Error('the page has not subscribed to links');
    const index = this.linkMessages++;
    await this.page.evaluate(
      ([id, message, at]) => {
        const callbacks = (window as unknown as { __atmCallbacks: Record<number, (m: unknown) => void> })
          .__atmCallbacks;
        callbacks[id as number]({ index: at, message });
      },
      [this.linkChannel, address, index] as const,
    );
  }

  /** The page's menu channel (T-512), and what it last reported the menu shows */
  private menuChannel: number | null = null;
  private menuMessages = 0;
  menuState: Record<string, unknown> | null = null;

  /** A menu item chosen: its action on the page's menu channel, as the app sends it */
  async chooseMenu(action: { kind: string; id?: string }) {
    if (this.menuChannel === null) throw new Error('the page has not subscribed to the menu');
    const index = this.menuMessages++;
    await this.page.evaluate(
      ([id, message, at]) => {
        const callbacks = (window as unknown as { __atmCallbacks: Record<number, (m: unknown) => void> })
          .__atmCallbacks;
        callbacks[id as number]({ index: at, message });
      },
      [this.menuChannel, action, index] as const,
    );
  }

  /** The page's live change subscriptions (T-506), by ID: what each watches and its channel */
  subscriptions = new Map<number, { workspace: string; project: string | null; channel: number }>();
  private nextSubscription = 0;
  private changeMessages = new Map<number, number>();

  /** Something changed in what the page watches: a message on each subscription's channel that matches (the
   * workspace's, or `project`'s), as the app's watcher sends it */
  async pushChange(project: string | null = null) {
    const targets = [...this.subscriptions.values()].filter((s) => s.project === project);
    if (targets.length === 0) throw new Error(`the page has not subscribed to ${project ?? 'its workspace'}`);
    for (const s of targets) {
      const index = this.changeMessages.get(s.channel) ?? 0;
      this.changeMessages.set(s.channel, index + 1);
      await this.page.evaluate(
        ([id, message, at]) => {
          const callbacks = (window as unknown as { __atmCallbacks: Record<number, (m: unknown) => void> })
            .__atmCallbacks;
          callbacks[id as number]({ index: at, message });
        },
        [s.channel, { workspace: s.workspace, project: s.project }, index] as const,
      );
    }
  }

  /** The update the app's `check_update` finds (T-483), and why `install_update` fails, if it should */
  update: { version: string; current_version: string; notes: string | null } | null = null;
  updateFails: string | null = null;
  /** What `install_update` sends on its progress channel before it returns (T-526) */
  updateProgress: unknown[] = [];
  /** `install_update` returns once this settles: a test holds it to look at the download's progress */
  updateInstallHeld: Promise<unknown> = Promise.resolve();
  /** The update commands the page called, in order */
  updateCalls: string[] = [];

  /** The app's agent commands the page called, in order (`agents_status`, `connect_agents <harness>`) */
  agentCalls: string[] = [];
  /** The titles the page gave its window (`set_title`, T-511), in order */
  windowTitles: string[] = [];
  /** What the window has of the platform's own (`shell`): macOS's by default, `{false, false}` for Windows' */
  windowShell = { system_menu_bar: true, system_window_buttons: true };
  /** The window commands the page called (T-513), in order */
  windowCalls: string[] = [];
  /** The addresses the page opened a new window at (`open_window`), in order */
  openedWindows: string[] = [];
  /** How many times the page asked the app for a project's tasks (`project_tasks`) */
  projectTaskRequests = 0;
  /** The tabs the window comes back with as the app starts (`restored_tabs`, T-518), answered once */
  restoredTabs: { addresses: string[]; active: number } | null = null;
  /** The tabs the page last kept for the next start (`save_tabs`) */
  savedTabs: { addresses: string[]; active: number } | null = null;
  /** What the page told the app about what needs you (`needs_you`, T-518, T-520), in order: the workspace it shows,
   * and whether the badge and the notifications are on (the app reads the counts itself) */
  needsYouCalls: { workspace: string; badge: boolean; notify: boolean }[] = [];
  /** The external links the page asked the app to open in the user's browser (`open_url`, T-551), in order */
  openedUrls: string[] = [];
  /** The harnesses that have atm's skills (connect_agents adds one) */
  private agentsConnected = new Set<string>();

  /** The harnesses' status, as `agents_status` reports it: Claude Code's folder, and the one Codex and Cursor (found
   * here) share with others, Cursor listing atm's skills twice once both have them (T-413) */
  private harnessStatus() {
    const harness = (id: string, label: string, folder: string, tools: [string, string, boolean][]) => ({
      harness: id,
      label,
      detected: true,
      scope: 'user',
      folder,
      version: '0.0.0',
      skills: AGENT_SKILLS.map((skill) => ({
        skill,
        path: `${folder}/${skill}`,
        state: this.agentsConnected.has(id) ? 'linked' : 'missing',
        version: null,
      })),
      tools: tools.map(([tool, label, detected]) => ({ tool, label, detected })),
      duplicates: id === 'agents' && this.agentsConnected.size === 2 ? ['Cursor'] : [],
    });
    return [
      harness('claude', 'Claude Code', '/Users/me/.claude/skills', [['claude', 'Claude Code', true]]),
      harness('agents', 'Codex, Copilot, Cursor, OpenCode and Gemini CLI', '/Users/me/.agents/skills', [
        ['codex', 'Codex', true],
        ['copilot', 'Copilot CLI', false],
        ['cursor', 'Cursor', true],
      ]),
    ];
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
      command === 'install_update' ? this.answerInstall(args) : this.answerCommand(command, args),
    );
    await this.page.addInitScript((version) => {
      const w = window as unknown as Record<string, unknown> & {
        __atmInvoke: (command: string, args: unknown) => Promise<{ ok?: unknown; error?: unknown }>;
      };
      w.isTauri = true;
      // The callbacks Tauri's IPC calls back (a channel's messages), by ID, so a test can send on a channel
      const callbacks: Record<number, (message: unknown) => void> = {};
      w.__atmCallbacks = callbacks;
      let next = 0;
      w.__TAURI_INTERNALS__ = {
        invoke: (command: string, args: unknown) =>
          w.__atmInvoke(command, args ?? {}).then((r) => ('error' in r ? Promise.reject(r.error) : r.ok)),
        transformCallback: (callback: (message: unknown) => void) => {
          next += 1;
          callbacks[next] = callback;
          return next;
        },
        unregisterCallback: () => {},
        // The app's asset scheme (`atm:`, ADR-005) is this test server, which serves CompMD's build with atm serve's
        // headers (serve.mjs); any other scheme as Tauri writes it on macOS
        convertFileSrc: (path: string, protocol = 'asset') =>
          protocol === 'atm'
            ? `${location.origin}/${encodeURIComponent(path)}`
            : `${protocol}://localhost/${encodeURIComponent(path)}`,
        metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
      };
      // What CompMD's plugin gives the page, the bundle's origin (ADR-006): here the test server's, which serves the
      // same bundle as atm serve, as the plugin serves it on compmd:
      w.__COMPMD__ = { origin: `${location.origin}/assets/compmd/${version}/compmd/` };
    }, compmdVersion());
    return this;
  }

  /** `install_update` (T-526): its progress on the page's channel (`updateProgress`, in order), then, once
   * `updateInstallHeld` lets it go, installed (true), or why it failed (`updateFails`) */
  private async answerInstall(args: Record<string, unknown>): Promise<{ ok?: unknown; error?: unknown }> {
    this.updateCalls.push('install_update');
    if (this.updateFails) return { error: { kind: 'failed', message: this.updateFails } };
    const channel = channelId(args.progress);
    for (const [index, message] of this.updateProgress.entries()) {
      await this.page.evaluate(
        ([id, message, at]) => {
          const callbacks = (window as unknown as { __atmCallbacks: Record<number, (m: unknown) => void> })
            .__atmCallbacks;
          callbacks[id as number]({ index: at, message });
        },
        [channel, message, index] as const,
      );
    }
    await this.updateInstallHeld;
    return { ok: true };
  }

  /** Where the default project's code reviews stand, by review ID (ATM-621): what atm reads from its reviews folder
   * into each task's `derived.review_statuses`, and into the status of the review it waits on (`derived.review`,
   * ATM-717). The fixture's views come from atm run outside git, with no reviews folder, so atm names the review
   * without a status and a test sets them here; atm's own reading and choice are tested by atm-lib and the CLI suite */
  reviewStatuses: Record<string, string> = {};

  /** Task views (as text) with the statuses of the reviews each task links, as atm serves them */
  private withReviewStatuses(text: string): string {
    if (!Object.keys(this.reviewStatuses).length) return text;
    const parsed = JSON.parse(text) as {
      tasks?: { task: Record<string, unknown>; derived: Record<string, unknown> }[];
    };
    for (const view of parsed.tasks ?? []) {
      const links = (view.task.code_reviews ?? []) as { id: string }[];
      const known = Object.fromEntries(
        links.filter((l) => l.id in this.reviewStatuses).map((l) => [l.id, this.reviewStatuses[l.id]]),
      );
      if (!Object.keys(known).length) continue;
      const review = view.derived.review as { id: string } | undefined;
      view.derived = {
        ...view.derived,
        review_statuses: known,
        ...(review && review.id in known ? { review: { ...review, status: known[review.id] } } : {}),
      };
    }
    return JSON.stringify(parsed);
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
        this.projectTaskRequests++;
        if (!body) return unknown;
        const archived = (args.filters as { archived?: boolean } | undefined)?.archived;
        if (archived)
          return this.archiveBody === null || project !== this.projectId
            ? { error: { kind: 'not_found', message: 'not found' } }
            : { ok: views(asTaskViews(body, this.archiveBody, true)) };
        return {
          ok: views(
            project === this.projectId
              ? this.withReviewStatuses(asTaskViews(body, this.archiveBody, false, this.registeredBodies))
              : asTaskViews(body),
          ),
        };
      }
      case 'project_reviews':
        return body ? { ok: this.reviewList ?? { reviews: [], you: 'Ana' } } : unknown;
      case 'project_review': {
        const page = this.reviewPages.get(String(args.review));
        return page ? { ok: page } : { error: { kind: 'not_found', message: `no review ${String(args.review)}` } };
      }
      case 'claims':
        return body ? { ok: { claims: this.claimsBody ? JSON.parse(this.claimsBody).claims : [] } } : unknown;
      case 'worktrees':
        return body
          ? { ok: { worktrees: this.worktreesBody ? JSON.parse(this.worktreesBody).worktrees : [] } }
          : unknown;
      case 'subscribe_links': {
        this.linkChannel = channelId(args.channel);
        const first = this.startLink;
        this.startLink = null;
        return { ok: first };
      }
      case 'subscribe_workspace':
      case 'subscribe_project': {
        const id = ++this.nextSubscription;
        this.subscriptions.set(id, {
          workspace: String(args.workspace),
          project: command === 'subscribe_project' ? project : null,
          channel: channelId(args.channel),
        });
        return { ok: id };
      }
      case 'unsubscribe':
        return { ok: this.subscriptions.delete(Number(args.id)) };
      case 'check_update':
        this.updateCalls.push(command);
        return { ok: this.update };
      case 'restart_app':
        this.updateCalls.push(command);
        return { ok: null };
      case 'agents_status':
        this.agentCalls.push(command);
        return { ok: this.harnessStatus() };
      case 'connect_agents': {
        const id = String(args.harness);
        this.agentCalls.push(`${command} ${id}`);
        const status = this.harnessStatus().find((h) => h.harness === id);
        if (!status) return { error: { kind: 'invalid', message: `no harness '${id}'` } };
        const before = this.agentsConnected.has(id);
        this.agentsConnected.add(id);
        return {
          ok: {
            written: AGENT_SKILLS.map((skill) => ({
              harness: id,
              skill,
              path: `${status.folder}/${skill}`,
              how: 'link',
              action: before ? 'unchanged' : 'installed',
            })),
            refused: [],
          },
        };
      }
      // The window (T-511): macOS's, as the app on a Mac answers
      case 'shell':
        return { ok: this.windowShell };
      case 'minimize':
      case 'toggle_maximize':
      case 'close_window':
      case 'start_dragging':
        this.windowCalls.push(command);
        return { ok: null };
      // A short definition of the app's menus (its own tests check the whole one)
      case 'menu':
        return {
          ok: [
            {
              label: 'View',
              windows: false,
              entries: [
                {
                  type: 'item',
                  action: { kind: 'show_list' },
                  label: 'List',
                  accelerator: 'CmdOrCtrl+Digit3',
                  checked: false,
                },
                { type: 'separator' },
                { type: 'item', action: { kind: 'reload' }, label: 'Reload', accelerator: null, checked: null },
              ],
            },
          ],
        };
      case 'set_title':
        this.windowTitles.push(String(args.title));
        return { ok: null };
      // The windows' tabs kept for the next start, and the badge (T-518)
      case 'restored_tabs': {
        const restored = this.restoredTabs;
        this.restoredTabs = null;
        return { ok: restored };
      }
      case 'save_tabs':
        this.savedTabs = args.tabs as { addresses: string[]; active: number };
        return { ok: null };
      // A Reload (ATM-584): as the app, the window's tabs come back and its page loads again from the app's page (here
      // the test server's root, which serves the same page), never from the route it was at
      case 'reload_window':
        this.windowCalls.push(command);
        this.restoredTabs = this.savedTabs;
        setTimeout(() => void this.page.evaluate(() => location.assign('/')).catch(() => {}), 0);
        return { ok: null };
      case 'needs_you':
        this.needsYouCalls.push(args as { workspace: string; badge: boolean; notify: boolean });
        return { ok: null };
      case 'open_url':
        this.openedUrls.push(String(args.url));
        return { ok: null };
      // A new window at an address (T-514, ATM-699's menus): recorded, as the app would open it
      case 'open_window':
        this.openedWindows.push(String(args.address));
        return { ok: null };
      // The menu (T-512)
      case 'subscribe_menu':
        this.menuChannel = channelId(args.channel);
        return { ok: null };
      case 'menu_state':
        this.menuState = args.shown as Record<string, unknown>;
        return { ok: null };
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

  /** Register a project (a fixture by name or inline tasks) for atm only, not the page: the default project's task
   * views show its tasks that a reference names, as atm finds them there (ADR-007 §7, T-549) */
  registerForAtm(data: string | TasksData) {
    this.registeredBodies.push(toBody(data));
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
    atmNowFor(undefined);
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
