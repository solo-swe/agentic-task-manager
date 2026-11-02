// `atm serve`: one detached dashboard server per user (ATM_HOME), serving every registered project. Each test has its
// own ATM_HOME, so it has its own server; ports come from a test range so a developer's own server is never touched,
// and every test stops what it started.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';
import { after, describe, test } from 'node:test';
import {
  PLUGIN_BUILD,
  PLUGIN_VERSION,
  REPO,
  baseEnv,
  git,
  makeProject,
  removeDir,
  runAtm,
  tempDir,
} from './helpers.mjs';
import { pageCsp } from '../../dashboard/tests/e2e/csp.mjs';

const TASKS = { meta: { id: 'abcd1234', project: 'Serve' }, tasks: [] };
// A different range per run: ports of the previous run can still be in TIME_WAIT, and the server deliberately doesn't
// use SO_REUSEADDR (on macOS it would let it share a port another server is listening on).
// Below 32768, where Linux starts handing out ports to outgoing connections (32768–60999; macOS from 49152): the
// suite's own connections, from the files running in parallel, could hold a test's port for a moment, so atm served
// on the next one and the test's request to the port it asked for was refused (T-276; CI run 40470063094).
const EPHEMERAL_PORTS = 32768;
let nextPort = 20000 + (process.pid % 240) * 50;
/** A block of ports for one test, so tests never collide even if a stop fails. */
const ports = () => {
  nextPort += 10;
  assert.ok(nextPort <= EPHEMERAL_PORTS, `test ports reached ${nextPort}: keep them below ${EPHEMERAL_PORTS} (T-276)`);
  return nextPort - 10;
};

const record = (home) => {
  try {
    return JSON.parse(fs.readFileSync(path.join(home, 'server.json'), 'utf8'));
  } catch {
    return null;
  }
};
/** Running, and not a zombie (an exited process nobody reaped yet, as happens in containers without an init). */
const alive = (pid) => {
  try {
    process.kill(pid, 0);
  } catch {
    return false;
  }
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
    return stat.slice(stat.lastIndexOf(')') + 1).trim()[0] !== 'Z';
  } catch {
    return true; // no /proc (macOS): orphans are reaped right away
  }
};
const portOf = (stdout) => Number(/localhost:(\d+)\//.exec(stdout)?.[1]);

const cleanups = [];
after(() => cleanups.forEach((fn) => fn()));

/** A user folder (ATM_HOME); its server is killed and the folder removed when the tests end. */
function home() {
  const dir = tempDir('atm-home-');
  cleanups.push(() => {
    const r = record(dir);
    if (r && alive(r.pid)) process.kill(r.pid, 'SIGTERM');
    removeDir(dir);
  });
  return dir;
}

/** A project with tasks.json in the new folder, removed when the tests end. */
function project(tasks = TASKS, files = {}) {
  const root = makeProject({ git: true, files: { '.agentic-task-manager/tasks.json': tasks, ...files } });
  cleanups.push(() => removeDir(root));
  return root;
}

const atmIn = (atmHome) => (args, cwd) => runAtm(args, cwd, { ATM_HOME: atmHome });

/** A project's task data as the v1 route serves it, in the task file's shape: {meta, tasks} (the raw task files
 * aren't served since T-374). */
async function v1Tasks(port, id) {
  const body = await (await fetch(`http://localhost:${port}/api/v1/projects/${id}/tasks`)).json();
  return { meta: body.meta, tasks: body.tasks.map((view) => view.task) };
}

/** A GET with a chosen Host header (fetch can't set it). */
function rawGet(port, urlPath, host) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path: urlPath, headers: { Host: host } }, (res) => {
      let body = '';
      res.on('data', (d) => (body += d));
      res.on('end', () => resolve({ status: res.statusCode, body }));
    });
    req.on('error', reject);
    req.end();
  });
}

describe('serve', () => {
  test('start, reuse, status, stop', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    const started = atm(['serve', '--port', String(port)], root);
    assert.equal(started.code, 0, started.stderr);
    assert.match(started.stdout, new RegExp(`Dashboard server started: http://localhost:${port}/#project=abcd1234\n`));
    // The launching process has exited; the server keeps running, recorded in ATM_HOME
    const r = record(atmHome);
    assert.equal(r.port, port);
    assert.equal(r.version, PLUGIN_VERSION);
    assert.equal(r.build, PLUGIN_BUILD);
    const dashboard = await fetch(`http://localhost:${port}/`);
    assert.equal(dashboard.status, 200);
    assert.match(dashboard.headers.get('content-type'), /^text\/html/);
    assert.equal(dashboard.headers.get('cache-control'), 'no-store');
    assert.match(await dashboard.text(), /atm-dashboard-version/);
    assert.deepEqual(await v1Tasks(port, 'abcd1234'), TASKS);
    assert.deepEqual(await (await fetch(`http://localhost:${port}/__atm/whoami`)).json(), {
      atm: true,
      pid: r.pid,
      port,
      version: PLUGIN_VERSION,
      build: PLUGIN_BUILD,
    });

    const again = atm(['serve', 'start', '--port', String(port + 5)], root);
    assert.match(again.stdout, new RegExp(`already running: http://localhost:${port}/#project=abcd1234`));
    assert.equal(record(atmHome).pid, r.pid);

    const running = atm(['serve', 'status'], root).stdout;
    assert.match(running, new RegExp(`running: http://localhost:${port}/ .*pid ${r.pid}`));
    const status = JSON.parse(atm(['serve', 'status', '--json'], root).stdout);
    assert.equal(status.running, true);
    assert.equal(status.url, `http://localhost:${port}/`);

    assert.equal(atm(['serve', 'stop'], root).stdout, `Dashboard server stopped (pid ${r.pid}).\n`);
    assert.equal(alive(r.pid), false);
    assert.equal(record(atmHome), null);
    assert.equal(atm(['serve', 'status'], root).stdout, 'Dashboard server is not running.\n');
    assert.equal(atm(['serve', 'stop'], root).stdout, 'Dashboard server is not running.\n');
  });

  test('one server for two projects; /api/projects lists them', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const a = project();
    const b = project({ meta: { id: 'bbbb2222', project: 'Second' }, tasks: [] });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], a).code, 0);
    const second = atm(['serve', '--port', String(port)], b);
    assert.match(second.stdout, new RegExp(`already running: http://localhost:${port}/#project=bbbb2222`));
    const api = await (await fetch(`http://localhost:${port}/api/projects`)).json();
    assert.match(api.generated_at, /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
    assert.deepEqual(
      api.projects.map((p) => [p.id, p.name, p.path, p.missing, p.summary.total]),
      [
        ['abcd1234', 'Serve', a, false, 0],
        ['bbbb2222', 'Second', b, false, 0],
      ],
    );
    assert.equal((await v1Tasks(port, 'bbbb2222')).meta.project, 'Second');
    atm(['serve', 'stop'], a);
  });

  test('/api/projects summarizes each project by status kind; skipped tasks count as neither done nor open', async () => {
    const atmHome = home();
    const env = { ATM_HOME: atmHome, ATM_NOW: '2026-03-15T12:00:00Z' };
    const atm = (args, cwd) => runAtm(args, cwd, env);
    const task = (id, status, extra = {}) => ({ id, name: id, type: 'feature', status, ...extra });
    const root = project({
      meta: { id: 'abcd1234', project: 'Summary' },
      tasks: [
        task('T-1', 'done', { completed_at: '2026-03-15T10:00:00Z' }),
        task('T-2', 'done', { completed_at: '2026-03-13T10:00:00Z' }),
        task('T-3', 'skipped', { updated_at: '2026-03-14T09:00:00Z' }),
        task('T-4', 'done', { completed_at: '2026-02-01T10:00:00Z' }),
        task('T-5', 'in_review', { review_requested_at: '2026-03-15T09:00:00Z' }),
        task('T-6', 'blocked', { waiting_on: 'user', questions: ['?'], blocked_at: '2026-03-15T11:00:00Z' }),
        task('T-7', 'todo'),
        // Blocked on an agent: open, but it doesn't need the user
        task('T-8', 'blocked', { waiting_on: 'agent', questions: ['?'] }),
      ],
    });
    const card = (id) => ({ id, name: id, type: 'feature', sprint: null, priority: 'P2', labels: [] });
    const skippedOnly = project({ meta: { id: 'eeee5555', project: 'Skipped' }, tasks: [task('T-1', 'skipped')] });
    const gone = project({ meta: { id: 'ffff6666', project: 'Gone' }, tasks: [] });
    const port = ports();
    for (const p of [root, skippedOnly, gone]) assert.equal(atm(['projects', 'add'], p).code, 0);
    removeDir(gone);
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const url = `http://localhost:${port}/api/projects`;
    const first = await fetch(url);
    const byId = Object.fromEntries((await first.json()).projects.map((p) => [p.id, p]));
    assert.deepEqual(byId.abcd1234.summary, {
      total: 8,
      done: 3,
      skipped: 1,
      percent: 43,
      active_sprint: null,
      recent_sprints: null,
      agents_working: 0,
      working: [],
      next: [{ id: 'T-7', name: 'T-7' }],
      last_done: { id: 'T-1', name: 'T-1', completed_at: '2026-03-15T10:00:00Z' },
      needs_you: { review: 1, blocked_on_user: 1, together: 0 },
      needs_you_tasks: [
        { id: 'T-5', name: 'T-5', reason: 'review' },
        { id: 'T-6', name: 'T-6', reason: 'blocked' },
      ],
      git: { branch: 'main', pr: null },
      description: null,
      last_activity: '2026-03-15T11:00:00Z',
      completions_14d: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
      // The All projects board (T-257): T-7 ready; T-5 waits for a review; T-6 blocked on the user before T-8 on an
      // agent; T-1 the only one finished in the last 24 hours
      board: {
        todo: { count: 1, cards: [card('T-7')] },
        active: { count: 0, cards: [] },
        review: { count: 1, cards: [{ ...card('T-5'), since: '2026-03-15T09:00:00Z' }] },
        blocked: {
          count: 2,
          cards: [
            { ...card('T-6'), since: '2026-03-15T11:00:00Z', waiting_on: 'user', questions: 1 },
            { ...card('T-8'), since: null, waiting_on: 'agent', questions: 1 },
          ],
        },
        done: { count: 1, cards: [{ ...card('T-1'), completed_at: '2026-03-15T10:00:00Z' }] },
      },
      // No sprints, so no active sprint's board (T-340)
      board_active_sprint: null,
      // The card's line (T-255): no creation times, no work recorded
      stats: { started: null, tasks: 8, agent_minutes: 0 },
    });
    assert.deepEqual(
      [byId.eeee5555.summary.percent, byId.eeee5555.summary.recent_sprints, byId.eeee5555.summary.last_activity],
      [100, null, null],
    );
    assert.deepEqual([byId.ffff6666.missing, byId.ffff6666.summary], [true, null]);

    // A claim (an agent at work) changes the summary and the ETag
    const etag = first.headers.get('etag');
    assert.equal((await fetch(url, { headers: { 'If-None-Match': etag } })).status, 304);
    assert.equal(atm(['start', 'T-7', '--agent', 'claude:test'], root).code, 0);
    const changed = await fetch(url, { headers: { 'If-None-Match': etag } });
    assert.equal(changed.status, 200);
    const summary = (await changed.json()).projects.find((p) => p.id === 'abcd1234').summary;
    assert.deepEqual([summary.agents_working, summary.last_activity], [1, '2026-03-15T12:00:00Z']);
    // What the agent works on, by name; nothing is left to take next
    // (branch null: the test repository has no commit yet, so git names no branch)
    assert.deepEqual(summary.working, [{ task: 'T-7', name: 'T-7', agent: 'claude:test', branch: null }]);
    assert.deepEqual(summary.next, []);
    atm(['serve', 'stop'], root);
  });

  test('working, next and last_done: the next two by the loop rule; names capped at 80 characters', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const long = 'A very long task name '.repeat(6);
    const task = (id, status, extra = {}) => ({ id, name: id, type: 'feature', status, ...extra });
    const root = project({
      meta: { id: 'cccc3333', project: 'Next' },
      tasks: [
        task('T-1', 'todo', { priority: 'P3' }),
        task('T-2', 'todo'),
        task('T-3', 'todo', { priority: 'P0', name: long }),
        task('T-4', 'todo', { dependencies: ['T-5'] }),
        task('T-5', 'todo', { priority: 'P0', with: 'user' }),
        task('T-6', 'done', { completed_at: '2026-03-02T10:00:00Z' }),
        task('T-7', 'done', { completed_at: '2026-03-01T10:00:00Z' }),
      ],
    });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const api = await (await fetch(`http://localhost:${port}/api/projects`)).json();
    const summary = api.projects.find((p) => p.id === 'cccc3333').summary;
    // P0 before P2 before P3; T-4 waits on T-5, which is done together with the user, so not next for an agent
    assert.deepEqual(
      summary.next.map((t) => t.id),
      ['T-3', 'T-2'],
    );
    assert.equal(summary.next[0].name.length, 80);
    assert.ok(summary.next[0].name.endsWith('…'));
    assert.deepEqual(summary.last_done, { id: 'T-6', name: 'T-6', completed_at: '2026-03-02T10:00:00Z' });
    atm(['serve', 'stop'], root);
  });

  test("stats: started, tasks and agent time, archived tasks included; a symlinked archive isn't read (T-255)", async () => {
    const atmHome = home();
    const env = { ATM_HOME: atmHome, ATM_NOW: '2026-03-15T12:00:00Z' };
    const atm = (args, cwd) => runAtm(args, cwd, env);
    const task = (id, status, extra = {}) => ({ id, name: id, type: 'feature', status, ...extra });
    const tasks = [
      // claude:x on two tasks at once, 10:00–11:00 (until the review request) and 10:30–11:30: 90 minutes
      task('A', 'done', {
        created_at: '2026-03-01T09:00:00Z',
        started_by: 'claude:x',
        started_at: '2026-03-10T10:00:00Z',
        review_requested_at: '2026-03-10T11:00:00Z',
        completed_at: '2026-03-10T12:00:00Z',
      }),
      task('B', 'done', {
        started_by: 'claude:x',
        started_at: '2026-03-10T10:30:00Z',
        completed_at: '2026-03-10T11:30:00Z',
      }),
      // A person's work isn't agent time
      task('C', 'done', {
        started_by: 'Hakan',
        started_at: '2026-03-10T08:00:00Z',
        completed_at: '2026-03-10T09:00:00Z',
      }),
      // Nobody recorded (before v4), still at work: 11:00 until now, 60 minutes
      task('D', 'in_progress', { started_at: '2026-03-15T11:00:00Z' }),
    ];
    const archive = {
      meta: {},
      tasks: [
        // 30 minutes, and the project's first task
        task('E', 'done', {
          created_at: '2026-02-20T08:00:00Z',
          started_by: 'claude:y',
          started_at: '2026-02-20T10:00:00Z',
          completed_at: '2026-02-20T10:30:00Z',
        }),
        // Also in tasks.json: counted once, from there
        task('A', 'done', {
          started_by: 'claude:z',
          started_at: '2026-01-01T00:00:00Z',
          completed_at: '2026-01-02T00:00:00Z',
        }),
      ],
    };
    const root = project(
      { meta: { id: 'abcd1234', project: 'Stats' }, tasks },
      { '.agentic-task-manager/tasks-archive.json': archive },
    );
    const linked = project({ meta: { id: 'eeee5555', project: 'Linked' }, tasks: [task('L', 'todo')] });
    assert.equal(atm(['projects', 'add'], linked).code, 0);
    const elsewhere = path.join(atmHome, 'archive.json');
    fs.writeFileSync(elsewhere, JSON.stringify(archive));
    fs.symlinkSync(elsewhere, path.join(linked, '.agentic-task-manager/tasks-archive.json'));
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const api = await (await fetch(`http://localhost:${port}/api/projects`)).json();
    const stats = (id) => api.projects.find((p) => p.id === id).summary.stats;
    assert.deepEqual(stats('abcd1234'), { started: '2026-02-20T08:00:00Z', tasks: 5, agent_minutes: 180 });
    // A task folder with a linked file is refused whole: listed without a summary, nothing read through the link
    assert.equal(api.projects.find((p) => p.id === 'eeee5555').summary, null);
    atm(['serve', 'stop'], root);
  });

  test('board: to-do tasks, the ready ones in the loop’s order (10 at most), the longest running first, 30 done today at most (T-257)', async () => {
    const atmHome = home();
    const env = { ATM_HOME: atmHome, ATM_NOW: '2026-03-15T12:00:00Z' };
    const atm = (args, cwd) => runAtm(args, cwd, env);
    const task = (id, status, extra = {}) => ({ id, name: id, type: 'feature', status, ...extra });
    const ready = Array.from({ length: 12 }, (_, i) => task(`T-${String(i + 1).padStart(2, '0')}`, 'todo'));
    const done = Array.from({ length: 31 }, (_, i) =>
      task(`D-${String(i + 1).padStart(2, '0')}`, 'done', {
        completed_at: `2026-03-15T${String(Math.floor(i / 3)).padStart(2, '0')}:${String((i % 3) * 10).padStart(2, '0')}:00Z`,
      }),
    );
    const root = project({
      meta: { id: 'bbbb2222', project: 'Board' },
      tasks: [
        ...ready,
        // Waiting on a dependency: on the board after the ready ones (T-349), so not among the first 10
        task('T-20', 'todo', { dependencies: ['A'] }),
        task('A', 'in_progress', { started_by: 'claude:x', started_at: '2026-03-15T10:00:00Z' }),
        task('B', 'in_progress', { started_by: 'claude:y', started_at: '2026-03-15T09:00:00Z', labels: ['ux'] }),
        // Finished yesterday morning: more than 24 hours ago
        task('OLD', 'done', { completed_at: '2026-03-14T11:00:00Z' }),
        ...done,
      ],
    });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const api = await (await fetch(`http://localhost:${port}/api/projects`)).json();
    const { board } = api.projects.find((p) => p.id === 'bbbb2222').summary;
    assert.equal(board.todo.count, 13);
    assert.deepEqual(
      board.todo.cards.map((c) => c.id),
      ['T-01', 'T-02', 'T-03', 'T-04', 'T-05', 'T-06', 'T-07', 'T-08', 'T-09', 'T-10'],
    );
    assert.deepEqual(board.active.cards, [
      {
        id: 'B',
        name: 'B',
        type: 'feature',
        sprint: null,
        priority: 'P2',
        labels: ['ux'],
        by: 'claude:y',
        since: '2026-03-15T09:00:00Z',
      },
      {
        id: 'A',
        name: 'A',
        type: 'feature',
        sprint: null,
        priority: 'P2',
        labels: [],
        by: 'claude:x',
        since: '2026-03-15T10:00:00Z',
      },
    ]);
    assert.equal(board.done.count, 31);
    assert.equal(board.done.cards.length, 30);
    assert.equal(board.done.cards[0].id, 'D-31');
    assert.ok(!board.done.cards.some((c) => c.id === 'OLD'));
    atm(['serve', 'stop'], root);
  });

  test('git: the branch and the latest PR recorded on the tasks; a branch switch changes the ETag', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const task = (id, extra = {}) => ({ id, name: id, type: 'feature', status: 'done', ...extra });
    const url = (n) => `https://github.com/acme/demo/pull/${n}`;
    const root = project({
      meta: { id: 'dddd4444', project: 'Git', description: '  A tool for agents. '.repeat(20) },
      tasks: [
        task('T-1', { pr: { number: 3, url: url(3) }, merged_at: '2026-03-01T10:00:00Z' }),
        task('T-2', { pr: { number: 5 } }),
        task('T-3', { pr: { number: 5, url: url(5) } }),
        task('T-4'),
      ],
    });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const api = `http://localhost:${port}/api/projects`;
    const first = await fetch(api);
    const gitOf = (body) => body.projects.find((p) => p.id === 'dddd4444').summary.git;
    const body = await first.json();
    assert.deepEqual(gitOf(body), { branch: 'main', pr: { number: 5, url: url(5), state: 'open' } });
    // meta.description, trimmed and capped at 200 characters (T-209: the name's tooltip on the card)
    const summaryDescription = body.projects.find((p) => p.id === 'dddd4444').summary.description;
    assert.equal(summaryDescription.length, 200);
    assert.ok(summaryDescription.startsWith('A tool for agents.') && summaryDescription.endsWith('…'));
    git(['checkout', '-q', '-b', 'feature'], root);
    const switched = await fetch(api, { headers: { 'If-None-Match': first.headers.get('etag') } });
    assert.equal(switched.status, 200);
    assert.equal(gitOf(await switched.json()).branch, 'feature');
    atm(['serve', 'stop'], root);
  });

  test('a project with sprints: the active sprint with its progress, recent sprints for the rings, next from the active sprint (SR-7.7)', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const task = (id, status, sprint) => ({ id, name: id, type: 'feature', status, ...(sprint ? { sprint } : {}) });
    const sprints = [
      { id: 'Old', state: 'closed', end: '2026-01-01T00:00:00Z' },
      { id: 'Older', state: 'closed', end: '2025-12-01T00:00:00Z' },
      { id: 'Now', state: 'active', goal: 'Ship', due: '2026-04-01' },
      { id: 'Next', state: 'planned' },
      { id: 'Empty', state: 'planned' },
    ];
    const tasks = [
      task('T-1', 'done', 'Old'),
      task('T-2', 'done', 'Older'),
      task('T-3', 'done', 'Now'),
      task('T-4', 'skipped', 'Now'),
      task('T-5', 'todo', 'Now'),
      task('T-6', 'todo', 'Next'),
      // In the backlog, and first by ID: the loop doesn't take it while a sprint is active
      task('T-0', 'todo'),
    ];
    const root = project({ meta: { id: 'sprinted', project: 'sprinted', sprints }, tasks });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const api = await (await fetch(`http://localhost:${port}/api/projects`)).json();
    const summary = api.projects.find((p) => p.id === 'sprinted').summary;
    assert.deepEqual(summary.active_sprint, {
      id: 'Now',
      state: 'active',
      due: '2026-04-01',
      release: null,
      done: 1,
      total: 2,
      percent: 50,
      goal: 'Ship',
    });
    // The open ones with tasks (Now, Next; Empty has none), filled up to four with the latest closed, in the project's order
    assert.deepEqual(
      summary.recent_sprints.map((s) => [s.id, s.state, s.done, s.total]),
      [
        ['Old', 'closed', 1, 1],
        ['Older', 'closed', 1, 1],
        ['Now', 'active', 1, 2],
        ['Next', 'planned', 0, 1],
      ],
    );
    assert.deepEqual(
      summary.next.map((n) => n.id),
      ['T-5'],
    );
    atm(['serve', 'stop'], root);
  });

  test('a project logo: served at p/<id>/logo with its type, listed in api/projects; none, outside or too large is 404', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>';
    const withLogo = project(
      { meta: { id: 'logo0001', project: 'Set', logo: 'brand/mark.svg' }, tasks: [] },
      { 'brand/mark.svg': svg },
    );
    const found = project({ meta: { id: 'logo0002', project: 'Found' }, tasks: [] }, { '.github/logo.png': 'png' });
    const none = project({ meta: { id: 'logo0003', project: 'None' }, tasks: [] });
    const elsewhere = tempDir('atm-elsewhere-');
    cleanups.push(() => removeDir(elsewhere));
    fs.writeFileSync(path.join(elsewhere, 'logo.svg'), svg);
    const outside = project({
      meta: { id: 'logo0004', project: 'Outside', logo: path.join(elsewhere, 'logo.svg') },
      tasks: [],
    });
    const big = project(
      { meta: { id: 'logo0005', project: 'Big' }, tasks: [] },
      { 'logo.png': 'x'.repeat(513 * 1024) },
    );
    for (const p of [withLogo, found, none, outside, big]) assert.equal(atm(['projects', 'add'], p).code, 0);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], withLogo).code, 0);
    const base = `http://localhost:${port}`;

    const api = await (await fetch(`${base}/api/projects`)).json();
    assert.deepEqual(Object.fromEntries(api.projects.map((p) => [p.id, p.logo])), {
      logo0001: 'p/logo0001/logo',
      logo0002: 'p/logo0002/logo',
      logo0003: null,
      logo0004: null,
      logo0005: null,
    });

    const logo = await fetch(`${base}/p/logo0001/logo`);
    assert.equal(logo.status, 200);
    assert.equal(logo.headers.get('content-type'), 'image/svg+xml');
    assert.equal(logo.headers.get('cache-control'), 'no-cache');
    assert.match(logo.headers.get('content-security-policy'), /default-src 'none'.*sandbox/);
    assert.equal(await logo.text(), svg);
    const again = await fetch(`${base}/p/logo0001/logo`, { headers: { 'If-None-Match': logo.headers.get('etag') } });
    assert.equal(again.status, 304);
    assert.equal((await fetch(`${base}/p/logo0002/logo`)).headers.get('content-type'), 'image/png');
    for (const id of ['logo0003', 'logo0004', 'logo0005']) {
      const r = await fetch(`${base}/p/${id}/logo`);
      assert.deepEqual([id, r.status, (await r.json()).error], [id, 404, 'no logo']);
    }

    // A logo added later changes the ETag of api/projects
    const etag = (await fetch(`${base}/api/projects`)).headers.get('etag');
    fs.writeFileSync(path.join(none, 'logo.svg'), svg);
    const changed = await fetch(`${base}/api/projects`, { headers: { 'If-None-Match': etag } });
    assert.equal(changed.status, 200);
    assert.equal((await changed.json()).projects.find((p) => p.id === 'logo0003').logo, 'p/logo0003/logo');
    atm(['serve', 'stop'], withLogo);
  });

  test('tasks and /api/projects have ETags; a poll with one gets 304 until something changes', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const url = `http://localhost:${port}/api/v1/projects/abcd1234/tasks`;
    const first = await fetch(url);
    const etag = first.headers.get('etag');
    assert.match(etag, /^"[0-9a-f]+"$/);
    const unchanged = await fetch(url, { headers: { 'If-None-Match': etag } });
    assert.equal(unchanged.status, 304);
    assert.equal(unchanged.headers.get('etag'), etag);
    const apiUrl = `http://localhost:${port}/api/projects`;
    const apiEtag = (await fetch(apiUrl)).headers.get('etag');
    assert.equal((await fetch(apiUrl, { headers: { 'If-None-Match': apiEtag } })).status, 304);
    // An atm change right away (same second): new ETags and the new content
    assert.equal(atm(['add', '--name', 'New', '--type', 'feature'], root).code, 0);
    const changed = await fetch(url, { headers: { 'If-None-Match': etag } });
    assert.equal(changed.status, 200);
    assert.notEqual(changed.headers.get('etag'), etag);
    assert.equal((await changed.json()).tasks.length, 1);
    assert.equal((await fetch(apiUrl, { headers: { 'If-None-Match': apiEtag } })).status, 200);
    // Not for the dashboard
    assert.equal((await fetch(`http://localhost:${port}/`)).headers.get('etag'), null);
    atm(['serve', 'stop'], root);
  });

  test("a project's git worktrees: listed, their changed tasks in worktrees.json, their claims in claims.json", async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const task = (id) => ({ id, name: id, type: 'feature', status: 'todo' });
    const root = makeProject({
      commit: true,
      files: {
        '.agentic-task-manager/tasks.json': {
          meta: { id: 'abcd1234', project: 'Trees' },
          tasks: [task('T-1'), task('T-2')],
        },
      },
    });
    const linked = tempDir('atm-worktree-');
    cleanups.push(
      () => removeDir(root),
      () => removeDir(linked),
    );
    git(['worktree', 'add', '-q', linked, '-b', 'agent-2'], root);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const base = `http://localhost:${port}`;
    const api = await (await fetch(`${base}/api/projects`)).json();
    assert.deepEqual(api.projects[0].worktrees, [
      { path: root, branch: 'main', main: true },
      { path: linked, branch: 'agent-2', main: false },
    ]);
    const before = await fetch(`${base}/p/abcd1234/worktrees.json`);
    const etag = before.headers.get('etag');
    assert.deepEqual((await before.json()).worktrees[1].tasks, []); // nothing differs yet

    // An agent starts T-1 in the linked worktree: only that worktree's tasks.json says so, but the main project's
    // claims.json (from the shared claims store) and worktrees.json both show it
    assert.equal(atm(['start', 'T-1', '--agent', 'claude:tree'], linked).code, 0);
    const after = await fetch(`${base}/p/abcd1234/worktrees.json`, { headers: { 'If-None-Match': etag } });
    assert.equal(after.status, 200);
    const changed = (await after.json()).worktrees[1].tasks;
    assert.deepEqual(
      changed.map((t) => [t.id, t.status]),
      [['T-1', 'in_progress']],
    );
    const claims = await (await fetch(`${base}/p/abcd1234/claims.json`)).json();
    assert.deepEqual(
      claims.claims.map((c) => [c.task, c.agent, c.branch]),
      [['T-1', 'claude:tree', 'agent-2']],
    );
    assert.equal((await v1Tasks(port, 'abcd1234')).tasks[0].status, 'todo');

    // T-1 is blocked in the linked worktree and answered in the main one (T-204): the linked worktree's blocked copy
    // is stale until atm runs there, so it isn't listed (it would outrank the answer on the board)
    const block = ['block', 'T-1', '--agent', 'claude:tree', '--on', 'user', '--question', 'Which?'];
    assert.equal(atm(block, linked).code, 0);
    const blocked = (await (await fetch(`${base}/p/abcd1234/worktrees.json`)).json()).worktrees[1].tasks;
    assert.deepEqual(
      blocked.map((t) => [t.id, t.status]),
      [['T-1', 'blocked']],
    );
    assert.equal(atm(['unblock', 'T-1', '--answer', 'This one'], root).code, 0);
    assert.deepEqual((await (await fetch(`${base}/p/abcd1234/worktrees.json`)).json()).worktrees[1].tasks, []);
    atm(['serve', 'stop'], root);
  });

  test('a registered project has its v1 tasks, claims.json and worktrees.json; no task file is served; the rest is 404 or 405', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project(TASKS, {
      '.agentic-task-manager/secret.json': { secret: true },
      '.agentic-task-manager/tasks-archive.json': { meta: {}, tasks: [] },
    });
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const get = async (p, init) => {
      const r = await fetch(`http://localhost:${port}${p}`, init);
      return [r.status, (await r.json()).error];
    };
    assert.deepEqual(await get('/api/v1/projects/ffff0000/tasks'), [404, 'unknown project']);
    assert.deepEqual(await get('/p/ffff0000/claims.json'), [404, 'unknown project']);
    assert.deepEqual(await get('/p/abcd1234/secret.json'), [404, 'not found']);
    // The task files themselves only through atm-lib: the v1 route (T-374)
    assert.deepEqual(await get('/p/abcd1234/tasks.json'), [404, 'not found']);
    assert.deepEqual(await get('/p/abcd1234/tasks-archive.json'), [404, 'not found']);
    assert.equal((await fetch(`http://localhost:${port}/api/v1/projects/abcd1234/tasks`)).status, 200);
    assert.equal((await fetch(`http://localhost:${port}/p/abcd1234/claims.json`)).status, 200);
    assert.equal((await fetch(`http://localhost:${port}/p/abcd1234/worktrees.json`)).status, 200);
    assert.deepEqual(await get('/p/abcd1234/tasks.json/x'), [404, 'not found']);
    assert.deepEqual(await get('/tasks.json'), [404, 'not found']);
    assert.deepEqual(await get('/tasks-dashboard.html'), [404, 'not found']);
    assert.deepEqual(await get('/p/'), [404, 'not found']);
    assert.deepEqual(await get('/api/projects', { method: 'POST' }), [405, 'only GET and HEAD']);
    atm(['serve', 'stop'], root);
  });

  test('security headers: the page has the CSP the e2e suite runs under; every response is nosniff, same-origin (T-226)', async () => {
    const atm = atmIn(home());
    const root = project(TASKS);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const page = await fetch(`http://localhost:${port}/`);
    const body = Buffer.from(await page.arrayBuffer());
    assert.equal(page.headers.get('content-security-policy'), pageCsp(body));
    assert.match(page.headers.get('content-security-policy'), /script-src 'sha256-[^']+' 'sha256-[^']+';/);
    for (const p of ['/', '/api/projects', '/api/v1/projects/abcd1234/tasks', '/p/abcd1234/claims.json', '/nope']) {
      const r = await fetch(`http://localhost:${port}${p}`);
      assert.equal(r.headers.get('x-content-type-options'), 'nosniff', p);
      assert.equal(r.headers.get('cross-origin-resource-policy'), 'same-origin', p);
    }
    atm(['serve', 'stop'], root);
  });

  test('[::1] on the port is the server too, so nobody else can answer for localhost; a taken [::1] moves it on (T-227, F10)', async () => {
    const atm = atmIn(home());
    const root = project(TASKS);
    const port = ports();
    // Another process already listens on [::1]:port (skipped where IPv6 is missing)
    const squatter = net.createServer();
    const v6 = await new Promise((resolve) => {
      squatter.once('error', () => resolve(false));
      squatter.listen(port, '::1', () => resolve(true));
    });
    try {
      assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
      const status = JSON.parse(atm(['serve', 'status', '--json'], root).stdout);
      if (v6) assert.notEqual(status.port, port, 'moved past the taken [::1] port');
      if (v6) {
        // [::1] on the server's own port answers as atm, and can't be taken by anyone else
        const res = await new Promise((resolve, reject) => {
          const req = http.request(
            { host: '::1', port: status.port, path: '/__atm/whoami', headers: { Host: `localhost:${status.port}` } },
            (r2) => {
              let body = '';
              r2.on('data', (c) => (body += c));
              r2.on('end', () => resolve({ status: r2.statusCode, body }));
            },
          );
          req.on('error', reject);
          req.end();
        });
        assert.equal(res.status, 200);
        assert.equal(JSON.parse(res.body).atm, true);
        const second = net.createServer();
        const taken = await new Promise((resolve) => {
          second.once('error', (e) => resolve(e.code));
          second.listen(status.port, '::1', () => resolve('listening'));
        });
        second.close();
        assert.equal(taken, 'EADDRINUSE');
      }
      atm(['serve', 'stop'], root);
    } finally {
      squatter.close();
    }
  });

  test('one project with unreadable (deeply nested) JSON is listed as missing; the others are fine (T-227, F18)', async () => {
    const atm = atmIn(home());
    const good = project(TASKS);
    const bad = project({ meta: { id: 'bad00000', project: 'Bad' }, tasks: [] });
    const port = ports();
    assert.equal(atm(['projects', 'add'], bad).code, 0);
    fs.writeFileSync(path.join(bad, '.agentic-task-manager/tasks.json'), '['.repeat(100000) + ']'.repeat(100000));
    assert.equal(atm(['serve', '--port', String(port)], good).code, 0);
    const r = await fetch(`http://localhost:${port}/api/projects`);
    assert.equal(r.status, 200);
    const projects = Object.fromEntries((await r.json()).projects.map((p) => [p.id, p]));
    assert.equal(projects.abcd1234.missing, false);
    assert.equal(projects.bad00000.missing, true);
    atm(['serve', 'stop'], good);
  });

  test('two projects with the same meta.id both stay reachable; ATM_HOME is private (T-227, F18)', async () => {
    const atmHome = path.join(tempDir('atm-parent-'), 'home');
    const atm = atmIn(atmHome);
    // Stopped and removed when the tests end even if this one fails, so its server can't answer a later run's port
    cleanups.push(() => {
      const r = record(atmHome);
      if (r && alive(r.pid)) process.kill(r.pid, 'SIGTERM');
      removeDir(path.dirname(atmHome));
    });
    const first = project(TASKS);
    const second = project({
      meta: { id: 'abcd1234', project: 'Copy' },
      tasks: [{ id: 'T-1', name: 'x', phase: 0, type: 'feature', status: 'todo' }],
    });
    assert.equal(atm(['projects', 'add'], first).code, 0);
    assert.equal(atm(['projects', 'add'], second).code, 0);
    assert.equal(fs.statSync(atmHome).mode & 0o777, 0o700);
    const listed = JSON.parse(atm(['projects', '--json'], first).stdout);
    const ids = listed.projects.map((p) => p.id);
    assert.equal(new Set(ids).size, 2, ids.join(', '));
    assert.ok(ids.includes('abcd1234'));
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], first).code, 0);
    for (const id of ids) {
      assert.equal((await fetch(`http://localhost:${port}/api/v1/projects/${id}/tasks`)).status, 200, id);
    }
    atm(['serve', 'stop'], first);
    removeDir(path.dirname(atmHome));
  });

  test('a task file that is a symlink is never read, wherever it points: the project is refused (T-224, F8)', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project(TASKS);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const v1 = `http://localhost:${port}/api/v1/projects/abcd1234/tasks`;
    assert.equal((await fetch(v1)).status, 200);
    // Linked after the server started (atm itself refuses such a folder): an archive pointing at a private file
    const secret = path.join(atmHome, 'id_rsa.json');
    fs.writeFileSync(secret, '{"tasks": [], "secret": "PRIVATE KEY"}');
    fs.symlinkSync(secret, path.join(root, '.agentic-task-manager/tasks-archive.json'));
    for (const url of [`${v1}?archived=true`, v1, `http://localhost:${port}/p/abcd1234/tasks-archive.json`]) {
      const r = await fetch(url);
      assert.equal(r.status, 404, url);
      assert.doesNotMatch(await r.text(), /PRIVATE KEY/, url);
    }
    atm(['serve', 'stop'], root);
  });

  test('a request for another Host is refused (DNS rebinding)', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    assert.equal((await rawGet(port, '/', `127.0.0.1:${port}`)).status, 200);
    const refused = await rawGet(port, '/api/v1/projects/abcd1234/tasks', `evil.example:${port}`);
    assert.equal(refused.status, 403);
    assert.deepEqual(JSON.parse(refused.body), { error: 'unexpected Host header' });
    assert.equal((await rawGet(port, '/', `localhost:${port + 1}`)).status, 403);
    atm(['serve', 'stop'], root);
  });

  test('api/search: every word, in any field or the project name, case-insensitive, read-only, capped (T-258)', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const task = (id, extra = {}) => ({ id, name: `Task ${id}`, phase: 0, type: 'feature', status: 'todo', ...extra });
    const shop = project({
      meta: { id: 'aaaa1111', project: 'Shop' },
      tasks: [
        task('T-1', { name: 'Checkout page' }),
        task('T-2', { description: 'Pay with a CARD' }),
        task('T-3', { type: 'bugfix', labels: ['payments'] }),
        task('T-4', { notes: [{ at: '2026-01-01T00:00:00Z', kind: 'note', text: 'Ask about refunds' }] }),
      ],
    });
    const blog = project({
      meta: { id: 'bbbb2222', project: 'Blog' },
      tasks: Array.from({ length: 205 }, (_, i) => task(`B-${i + 1}`, { name: 'Write a post' })),
    });
    for (const p of [shop, blog]) assert.equal(atm(['projects', 'add'], p).code, 0);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], shop).code, 0);
    const search = async (q, extra = '') =>
      (await fetch(`http://localhost:${port}/api/search?q=${encodeURIComponent(q)}${extra}`)).json();

    assert.deepEqual(await search('checkout'), { aaaa1111: ['T-1'] });
    assert.deepEqual(await search('card'), { aaaa1111: ['T-2'] });
    assert.deepEqual(await search('BUGFIX payments'), { aaaa1111: ['T-3'] });
    assert.deepEqual(await search('refunds'), { aaaa1111: ['T-4'] });
    assert.deepEqual(await search('t-1'), { aaaa1111: ['T-1'] });
    // Every word must appear; the project's name counts for all its tasks
    assert.deepEqual(await search('checkout refunds'), {});
    assert.deepEqual(await search('shop card'), { aaaa1111: ['T-2'] });
    assert.deepEqual((await search('shop')).aaaa1111, ['T-1', 'T-2', 'T-3', 'T-4']);
    // At most 200 per project; an empty query finds nothing; other parameters are ignored
    assert.equal((await search('post')).bbbb2222.length, 200);
    assert.deepEqual(await search('  '), {});
    assert.deepEqual(await search('checkout', '&x=1&project=bbbb2222'), { aaaa1111: ['T-1'] });
    // Only the first 200 characters of the query count
    assert.deepEqual(await search('checkout' + ' '.repeat(192) + 'nothing-like-this'), { aaaa1111: ['T-1'] });

    // The same checks as every route: another Host is refused, and nothing but GET and HEAD
    const refused = await rawGet(port, '/api/search?q=checkout', `evil.example:${port}`);
    assert.equal(refused.status, 403);
    assert.equal((await fetch(`http://localhost:${port}/api/search?q=x`, { method: 'POST' })).status, 405);
    atm(['serve', 'stop'], shop);
  });

  test('api/v1/projects/<id>/tasks: the tasks with what atm derives, filtered, conditional (T-364)', async () => {
    const atm = atmIn(home());
    const task = (id, extra = {}) => ({ id, name: `Task ${id}`, type: 'feature', status: 'todo', ...extra });
    const root = project({
      meta: { id: 'cccc3333', project: 'Shop', sprints: [{ id: 'One', state: 'active' }] },
      tasks: [
        task('T-1', { status: 'done', sprint: 'One', pr: { number: 2 } }),
        task('T-2', { dependencies: ['T-1'], sprint: 'One' }),
        task('T-3', { dependencies: ['T-2'] }),
      ],
    });
    assert.equal(atm(['projects', 'add'], root).code, 0);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const url = (query = '') => `http://localhost:${port}/api/v1/projects/cccc3333/tasks${query}`;

    const response = await fetch(url());
    assert.equal(response.status, 200);
    const body = await response.json();
    // The project's meta and its task data's size come in the same snapshot
    assert.equal(body.meta.project, 'Shop');
    assert.ok(body.bytes > 0);
    assert.deepEqual(
      body.tasks.map((v) => [v.task.id, v.derived.kind, v.derived.ready, v.derived.waiting_for, v.derived.stage]),
      [
        ['T-1', 'done', false, [], 'in_pr'],
        ['T-2', 'todo', true, [], undefined],
        ['T-3', 'todo', false, ['T-2'], undefined],
      ],
    );
    // Filters as query parameters, as atm list takes them
    const ids = async (query) => (await (await fetch(url(query))).json()).tasks.map((v) => v.task.id);
    assert.deepEqual(await ids('?sprint=One'), ['T-1', 'T-2']);
    assert.deepEqual(await ids('?ready=true'), ['T-2']);
    assert.deepEqual(await ids('?status=done&status=todo&backlog=1'), ['T-3']);
    assert.equal((await fetch(url('?sprint=Nope'))).status, 400);
    // Unchanged files: 304, as for every conditional route
    const again = await fetch(url(), { headers: { 'If-None-Match': response.headers.get('etag') } });
    assert.equal(again.status, 304);
    // An unknown project; another Host
    assert.equal((await fetch(`http://localhost:${port}/api/v1/projects/nope/tasks`)).status, 404);
    assert.equal((await rawGet(port, '/api/v1/projects/cccc3333/tasks', `evil.example:${port}`)).status, 403);
    atm(['serve', 'stop'], root);
  });

  test('api/v1/projects/<id>/tasks reads a malformed task file as far as it can, and says what is wrong (T-377)', async () => {
    const atm = atmIn(home());
    const task = (id, extra = {}) => ({ id, name: `Task ${id}`, type: 'feature', status: 'todo', ...extra });
    const root = project({
      meta: { id: 'dddd4444', project: 'Broken', priorities: 'not a list' },
      tasks: [task('T-1'), 'not a task', task('T-2', { dependencies: 'T-1' })],
    });
    assert.equal(atm(['projects', 'add'], root).code, 0);
    // Every atm command still refuses it
    assert.notEqual(atm(['list'], root).code, 0);
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const response = await fetch(`http://localhost:${port}/api/v1/projects/dddd4444/tasks`);
    assert.equal(response.status, 200);
    const body = await response.json();
    assert.deepEqual(
      body.tasks.map((v) => [v.task.id, v.derived.ready, v.derived.problems]),
      [
        ['T-1', true, undefined],
        ['T-2', false, ["its dependencies aren't a list of task IDs (shown with the IDs that can be read)"]],
      ],
    );
    assert.equal(body.problems.length, 2); // the entry left out, and the priorities
    assert.match(body.problems.join('\n'), /isn't a task/);
    assert.match(body.problems.join('\n'), /meta\.priorities/);
    atm(['serve', 'stop'], root);
  });

  test('a busy port moves the server to the next free one', async () => {
    const atm = atmIn(home());
    const root = project();
    const port = ports();
    const blocker = net.createServer();
    await new Promise((resolve) => blocker.listen(port, '127.0.0.1', resolve));
    try {
      const started = atm(['serve', '--port', String(port)], root);
      assert.equal(portOf(started.stdout), port + 1, started.stdout + started.stderr);
    } finally {
      blocker.close();
      atm(['serve', 'stop'], root);
    }
  });

  test('a stale record (process gone) is cleaned up, and start works', () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    const stale = { pid: 999999, port, version: '0.0.0', build: 'x', started_at: 'x' };
    fs.writeFileSync(path.join(atmHome, 'server.json'), JSON.stringify(stale));
    assert.equal(atm(['serve', 'status'], root).stdout, 'Dashboard server is not running.\n');
    assert.equal(record(atmHome), null);
    fs.writeFileSync(path.join(atmHome, 'server.json'), JSON.stringify(stale));
    assert.match(atm(['serve', '--port', String(port)], root).stdout, /Dashboard server started/);
    assert.notEqual(record(atmHome).pid, 999999);
    atm(['serve', 'stop'], root);
  });

  test('stop never kills a process that is not the recorded server', () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    const sleeper = spawn('sleep', ['30'], { stdio: 'ignore' });
    cleanups.push(() => sleeper.kill());
    const fake = { pid: sleeper.pid, port, version: '0.0.0', build: 'x', started_at: 'x' };
    fs.writeFileSync(path.join(atmHome, 'server.json'), JSON.stringify(fake));
    assert.match(atm(['serve', 'stop'], root).stdout, /not running \(removed a stale record for pid \d+\)/);
    assert.equal(alive(sleeper.pid), true);

    // Another user's (another ATM_HOME's) real server, wrongly recorded as ours
    const otherHome = home();
    atmIn(otherHome)(['serve', '--port', String(port + 1)], root);
    const theirs = record(otherHome);
    fs.writeFileSync(path.join(atmHome, 'server.json'), JSON.stringify({ ...theirs, pid: sleeper.pid }));
    assert.match(atm(['serve', 'stop'], root).stdout, /removed a stale record/);
    assert.equal(alive(theirs.pid), true);
    atmIn(otherHome)(['serve', 'stop'], root);
    assert.equal(alive(theirs.pid), false);
  });

  test('keeps serving after the directory it was started from is deleted', async () => {
    const atm = atmIn(home());
    const root = project(TASKS, { 'sub/x.txt': 'x' });
    const port = ports();
    atm(['serve', '--port', String(port)], path.join(root, 'sub'));
    removeDir(path.join(root, 'sub'));
    assert.equal((await fetch(`http://localhost:${port}/api/v1/projects/abcd1234/tasks`)).status, 200);
    atm(['serve', 'stop'], root);
  });

  test('the dashboard is served from memory: deleting the plugin folder later changes nothing', async () => {
    // A copy of the plugin, so its dashboard file can be deleted (as Claude Code removes old plugin versions)
    const plugin = path.join(tempDir('atm-plugin-'), 'plugin');
    fs.cpSync(path.join(REPO, 'plugin'), plugin, { recursive: true });
    cleanups.push(() => removeDir(path.dirname(plugin)));
    const atmHome = home();
    const root = project();
    const port = ports();
    const run = (args) =>
      spawnSync(path.join(plugin, 'scripts/atm'), args, {
        cwd: root,
        encoding: 'utf8',
        env: { ...baseEnv(), ATM_HOME: atmHome },
      });
    assert.equal(run(['serve', '--port', String(port)]).status, 0);
    fs.rmSync(path.join(plugin, 'templates/tasks-dashboard.html'));
    const page = await fetch(`http://localhost:${port}/`);
    assert.equal(page.status, 200);
    assert.match(await page.text(), /atm-dashboard-version/);
    run(['serve', 'stop']);
  });

  /** Stamps `version` and `build` into the dashboard of the plugin copy at `plugin`, as a rebuild would. */
  function stamp(plugin, version, build) {
    const page = path.join(plugin, 'templates/tasks-dashboard.html');
    fs.writeFileSync(
      page,
      fs
        .readFileSync(page, 'utf8')
        .replace(/(name="atm-dashboard-version" content=")[^"]*/, `$1${version}`)
        .replace(/(name="atm-dashboard-build" content=")[^"]*/, `$1${build}`),
    );
  }

  /** A copy of the plugin whose dashboard claims to be `version` (and `build`), as another install would be. */
  function pluginAt(version, build = 'old-build') {
    const plugin = path.join(tempDir('atm-plugin-'), 'plugin');
    fs.cpSync(path.join(REPO, 'plugin'), plugin, { recursive: true });
    cleanups.push(() => removeDir(path.dirname(plugin)));
    stamp(plugin, version, build);
    const run = (args, cwd, atmHome) =>
      spawnSync(path.join(plugin, 'scripts/atm'), args, {
        cwd,
        encoding: 'utf8',
        env: { ...baseEnv(), ATM_HOME: atmHome },
      });
    run.plugin = plugin;
    return run;
  }

  const whoami = async (port) => (await fetch(`http://localhost:${port}/__atm/whoami`)).json();
  /** The build in the <meta> of the page the server serves. */
  const pageBuild = async (port) =>
    /name="atm-dashboard-build" content="([^"]*)"/.exec(await (await fetch(`http://localhost:${port}/`)).text())?.[1];

  test("the page's build is the build whoami reports, so a current page never offers a reload", async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    assert.equal(await pageBuild(port), PLUGIN_BUILD);
    assert.equal((await whoami(port)).build, await pageBuild(port));
    atm(['serve', 'stop'], root);
  });

  test("another install of the same version doesn't take the server over, so they don't restart it in turn", async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    const other = pluginAt(PLUGIN_VERSION, 'other-build');
    assert.equal(other(['serve', '--port', String(port)], root, atmHome).status, 0);
    const pid = record(atmHome).pid;
    assert.equal(atm(['serve', '--refresh'], root).stdout, '');
    assert.equal(atm(['status'], root).code, 0);
    assert.equal(record(atmHome).pid, pid, 'left alone');
    assert.equal(other(['serve', '--refresh'], root, atmHome).stdout, '');
    assert.equal(record(atmHome).pid, pid);
    assert.equal((await whoami(port)).build, await pageBuild(port));
    atm(['serve', 'stop'], root);
  });

  test('serve --refresh restarts a server whose own dashboard was rebuilt under the same version', async () => {
    const atmHome = home();
    const root = project();
    const port = ports();
    const install = pluginAt(PLUGIN_VERSION, 'first-build');
    assert.equal(install(['serve', '--port', String(port)], root, atmHome).status, 0);
    const old = await whoami(port);
    assert.equal(old.build, 'first-build');
    stamp(install.plugin, PLUGIN_VERSION, 'second-build');
    const refreshed = install(['serve', '--refresh'], root, atmHome);
    assert.equal(refreshed.status, 0, refreshed.stderr);
    assert.match(refreshed.stdout, /restarted/);
    const now = await whoami(port);
    assert.notEqual(now.pid, old.pid);
    assert.equal(now.build, 'second-build');
    assert.equal(await pageBuild(port), 'second-build');
    install(['serve', 'stop'], root, atmHome);
  });

  test('serve --refresh restarts a server with an older dashboard, on the same port', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    assert.equal(pluginAt('0.0.1')(['serve', '--port', String(port)], root, atmHome).status, 0);
    const old = await whoami(port);
    assert.equal(old.version, '0.0.1');
    const refreshed = atm(['serve', '--refresh'], root);
    assert.equal(refreshed.code, 0, refreshed.stderr);
    assert.equal(
      refreshed.stdout,
      `Dashboard server restarted with dashboard ${PLUGIN_VERSION}: http://localhost:${port}/\n`,
    );
    const now = await whoami(port);
    assert.deepEqual([now.version, now.build, now.port], [PLUGIN_VERSION, PLUGIN_BUILD, port]);
    assert.notEqual(now.pid, old.pid);
    assert.equal(alive(old.pid), false);
    atm(['serve', 'stop'], root);
  });

  test('serve --refresh leaves a current or newer server alone, and starts none', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    assert.equal(atm(['serve', '--refresh'], root).stdout, '');
    assert.equal(record(atmHome), null, 'nothing started');

    assert.equal(atm(['serve', '--port', String(port)], root).code, 0);
    const current = record(atmHome).pid;
    assert.equal(atm(['serve', '--refresh'], root).stdout, '');
    assert.equal(record(atmHome).pid, current);
    atm(['serve', 'stop'], root);

    // A newer plugin's server (another session already updated) is never downgraded
    assert.equal(pluginAt('99.0.0')(['serve', '--port', String(port + 1)], root, atmHome).status, 0);
    const newer = record(atmHome).pid;
    assert.equal(atm(['serve', '--refresh'], root).stdout, '');
    assert.equal(atm(['status'], root).code, 0);
    assert.equal(record(atmHome).pid, newer);
    atm(['serve', 'stop'], root);
  });

  test('any atm command, and the PostToolUse hook, bring an outdated server along', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const port = ports();
    const older = pluginAt('0.0.1');
    assert.equal(older(['serve', '--port', String(port)], root, atmHome).status, 0);
    const status = atm(['status'], root);
    assert.equal(status.code, 0, status.stderr);
    assert.doesNotMatch(status.stdout, /restarted/, 'the command prints only its own output');
    assert.equal((await whoami(port)).version, PLUGIN_VERSION);
    atm(['serve', 'stop'], root);

    assert.equal(older(['serve', '--port', String(port + 1)], root, atmHome).status, 0);
    const hook = atm(['heartbeat', '--auto'], root);
    assert.equal(hook.stdout + hook.stderr, '');
    assert.equal((await whoami(port + 1)).version, PLUGIN_VERSION);
    atm(['serve', 'stop'], root);
  });

  /** A stand-in for a 0.10.0-0.14.0 per-project server: answers whoami with `folder` and its own pid. */
  async function oldServer(folder) {
    const port = ports();
    const script = `require('http').createServer((q, s) => s.end(JSON.stringify({ atm: true, folder: ${JSON.stringify(
      folder,
    )}, pid: process.pid }))).listen(${port}, '127.0.0.1')`;
    const child = spawn(process.execPath, ['-e', script], { stdio: 'ignore' });
    cleanups.push(() => child.kill());
    for (let i = 0; i < 100 && !(await fetch(`http://localhost:${port}/__atm/whoami`).catch(() => null)); i++) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return { pid: child.pid, port };
  }

  test("migrate stops a project's old per-project server, and only that project's", async () => {
    const atm = atmIn(home());
    const root = project();
    const taskDir = path.join(root, '.agentic-task-manager');
    const mine = await oldServer(taskDir);
    fs.writeFileSync(path.join(taskDir, '.server.json'), JSON.stringify({ ...mine, folder: taskDir }));
    fs.writeFileSync(path.join(taskDir, 'tasks-dashboard.html'), 'old copy');
    const r = atm(['migrate'], root);
    assert.match(r.stdout, new RegExp(`stopped its old dashboard server \\(pid ${mine.pid}\\)`));
    for (let i = 0; i < 50 && alive(mine.pid); i++) await new Promise((resolve) => setTimeout(resolve, 50));
    assert.equal(alive(mine.pid), false);
    assert.equal(fs.existsSync(path.join(taskDir, '.server.json')), false);
    assert.equal(fs.existsSync(path.join(taskDir, 'tasks-dashboard.html')), false);

    // A record pointing at another project's server: the record goes, the server stays
    const theirs = await oldServer('/somewhere/else/.agentic-task-manager');
    fs.writeFileSync(path.join(taskDir, '.server.json'), JSON.stringify({ ...theirs, folder: taskDir }));
    assert.doesNotMatch(atm(['migrate'], root).stdout, /stopped/);
    assert.equal(alive(theirs.pid), true);
    assert.equal(fs.existsSync(path.join(taskDir, '.server.json')), false);
  });

  test('migrates an old docs/tasks.json before serving', () => {
    const atm = atmIn(home());
    const root = makeProject({ commit: true, files: { 'docs/tasks.json': TASKS } });
    cleanups.push(() => removeDir(root));
    const r = atm(['serve', '--port', String(ports())], root);
    assert.match(r.stdout, /Moved docs\/tasks.json to .agentic-task-manager\/tasks.json/);
    assert.match(r.stdout, /#project=abcd1234/);
    atm(['serve', 'stop'], root);
  });

  test('--dir shows another project: start, status and stop from elsewhere', async () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = project();
    const elsewhere = makeProject({});
    cleanups.push(() => removeDir(elsewhere));
    const port = ports();
    const started = atm(['serve', '--dir', root, '--port', String(port)], elsewhere);
    assert.equal(started.code, 0, started.stderr);
    assert.match(started.stdout, new RegExp(`started: http://localhost:${port}/#project=abcd1234`));
    assert.match(atm(['serve', 'status', '--dir', root], elsewhere).stdout, new RegExp(`localhost:${port}/`));
    assert.match(atm(['serve', 'stop', '--dir', root], elsewhere).stdout, /Dashboard server stopped/);
    assert.equal(record(atmHome), null);
    assert.equal(atm(['serve', 'status', '--dir', path.join(root, 'nope')], elsewhere).code, 2);
  });

  test('--all starts the server for the overview, from anywhere', async () => {
    const atm = atmIn(home());
    const elsewhere = makeProject({});
    cleanups.push(() => removeDir(elsewhere));
    const port = ports();
    const started = atm(['serve', '--all', '--port', String(port)], elsewhere);
    assert.equal(started.code, 0, started.stderr);
    assert.equal(started.stdout, `Dashboard server started: http://localhost:${port}/\n`);
    assert.deepEqual((await (await fetch(`http://localhost:${port}/api/projects`)).json()).projects, []);
    atm(['serve', 'stop'], elsewhere);
  });

  test('a project that opted out is not listed; the overview is shown', () => {
    const atm = atmIn(home());
    const root = project({ meta: { id: 'abcd1234', dashboard: { listed: false } }, tasks: [] });
    const port = ports();
    const started = atm(['serve', '--port', String(port)], root);
    assert.match(started.stdout, /isn't listed on the dashboard/);
    assert.match(started.stdout, new RegExp(`started: http://localhost:${port}/\n`));
    atm(['serve', 'stop'], root);
  });

  test('without a task file: an error, nothing started', () => {
    const atmHome = home();
    const atm = atmIn(atmHome);
    const root = makeProject({ git: true });
    cleanups.push(() => removeDir(root));
    const r = atm(['serve'], root);
    assert.equal(r.code, 1);
    assert.match(r.stderr, /no tasks.json to serve/);
    assert.equal(record(atmHome), null);
    assert.equal(atm(['serve', 'restart'], root).code, 2);
  });
});
