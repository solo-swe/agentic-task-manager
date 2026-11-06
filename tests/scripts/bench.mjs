// How atm and the dashboard cope with large task files (docs/v4/atm-cli.md §7, data-model.md §5): generates 1k- and
// 10k-task files with realistic fields and times
//   --cli        an atm status change (start, then done) and the merge driver on three versions (needs python3)
//   --dashboard  the dashboard's first load and a reload that changes one task (needs the build and Playwright's
//                Chromium, as for the e2e tests)
// Prints a table. With --check it fails when a timing is far over its limit: the limits are generous, to catch large
// regressions on slow CI runners, not to measure precisely.
//
// Usage: node tests/scripts/bench.mjs [--cli] [--dashboard] [--check] [--sizes 1000,10000]
//        (no --cli or --dashboard: both). `pnpm run bench` in dashboard/ runs both.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { ATM, REPO, baseEnv, removeDir, tempDir } from './helpers.mjs';

const args = process.argv.slice(2);
const wantCli = args.includes('--cli') || !args.includes('--dashboard');
const wantDashboard = args.includes('--dashboard') || !args.includes('--cli');
const check = args.includes('--check');
const sizes = (args.includes('--sizes') ? args[args.indexOf('--sizes') + 1] : '1000,10000').split(',').map(Number);

// Seconds; generous on purpose (shared CI runners are slow and noisy)
const LIMITS = {
  1000: { write: 2, merge: 3, load: 10, reload: 5 },
  10000: { write: 8, merge: 20, load: 60, reload: 30 },
};

const TYPES = ['feature', 'bugfix', 'test', 'infra', 'docs', 'refactor', 'polish'];
const PRIORITIES = ['P0', 'P1', 'P2', 'P3'];
const at = (minutes) => new Date(Date.UTC(2026, 0, 1) + minutes * 60_000).toISOString().replace('.000Z', 'Z');
const words = (n, seed) =>
  Array.from({ length: n }, (_, i) => ['task', 'dashboard', 'agent', 'review', 'merge', 'claim'][(seed + i) % 6]).join(
    ' ',
  );

/** A task file with `n` tasks: 90% done (with notes, actors and a commit), the rest to do; 100 tasks per phase. */
export function generate(n) {
  const tasks = [];
  for (let i = 1; i <= n; i++) {
    const done = i <= n * 0.9;
    const start = i * 10;
    tasks.push({
      id: `T-${String(i).padStart(5, '0')}`,
      name: `Task ${i}: ${words(5, i)}`,
      phase: Math.floor((i - 1) / 100),
      type: TYPES[i % TYPES.length],
      priority: PRIORITIES[i % PRIORITIES.length],
      labels: i % 3 ? ['area-' + (i % 5)] : [],
      description: words(45, i),
      requirements: [`FR-${i % 20}.${i % 7}`],
      use_cases: [],
      status: done ? 'done' : 'todo',
      dependencies: i > 1 ? [`T-${String(i - 1).padStart(5, '0')}`] : [],
      files: [`src/module-${i % 40}.ts`, `tests/module-${i % 40}.test.ts`],
      created_at: at(0),
      created_by: 'Hakan Serce',
      updated_at: at(start + 8),
      ...(done
        ? {
            started_at: at(start),
            started_by: `claude:${(i % 4).toString(16).padStart(8, '0')}`,
            completed_at: at(start + 8),
            completed_by: `claude:${(i % 4).toString(16).padStart(8, '0')}`,
            updated_by: `claude:${(i % 4).toString(16).padStart(8, '0')}`,
            commit: (i * 2654435761).toString(16).padStart(7, '0').slice(0, 7),
            notes: [
              { at: at(start + 8), by: 'claude:0', kind: 'outcome', text: words(20, i) },
              ...(i % 4 === 0 ? [{ at: at(start + 4), by: 'claude:1', kind: 'handoff', text: words(25, i) }] : []),
            ],
          }
        : {}),
    });
  }
  return { meta: { project: `Bench ${n}`, id: `bench${n}` }, tasks };
}

const json = (data) => JSON.stringify(data, null, 2) + '\n';
const seconds = (ms) => (ms / 1000).toFixed(2);
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

function timeAtm(argv, cwd, env = {}) {
  const t = performance.now();
  const r = spawnSync(ATM, argv, { cwd, encoding: 'utf8', env: { ...baseEnv(), ATM_AGENT: 'bench', ...env } });
  const elapsed = performance.now() - t;
  if (r.status !== 0) throw new Error(`atm ${argv.join(' ')} failed (${r.status}): ${r.stderr}`);
  return elapsed;
}

function benchCli(n) {
  const data = generate(n);
  const root = tempDir('atm-bench-');
  try {
    const dir = path.join(root, '.agentic-task-manager');
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'tasks.json'), json(data));
    const size = fs.statSync(path.join(dir, 'tasks.json')).size;
    // A status change rewrites the whole file under the lock: start a task, then finish it (three rounds)
    const todo = data.tasks.filter((t) => t.status === 'todo').slice(0, 3);
    const writes = [];
    for (const t of todo) {
      // Each round's task depends on the previous one, which the round before finished
      writes.push(timeAtm(['start', t.id], root), timeAtm(['done', t.id], root));
    }
    // The merge driver parses three versions: each side finished a different task
    const ours = structuredClone(data);
    const theirs = structuredClone(data);
    ours.tasks[n - 1].status = 'in_progress';
    theirs.tasks[n - 2].priority = 'P0';
    for (const [name, value] of [
      ['base.json', data],
      ['ours.json', ours],
      ['theirs.json', theirs],
    ])
      fs.writeFileSync(path.join(root, name), json(value));
    const merge = timeAtm(['merge-driver', 'base.json', 'ours.json', 'theirs.json'], root);
    return { size, write: median(writes), merge };
  } finally {
    removeDir(root);
  }
}

/**
 * The answer of GET api/v1/projects/<id>/tasks for a project holding `data` (docs/multi-project-api.md §2.5): its meta
 * and size, and each task with what atm itself derives for it (`atm list --derived --tolerant`). Worked out
 * before the clock starts: the benchmark measures the dashboard, not the server.
 */
function taskViews(data) {
  const root = tempDir('atm-bench-views-');
  try {
    fs.mkdirSync(path.join(root, '.agentic-task-manager'));
    const file = json(data);
    fs.writeFileSync(path.join(root, '.agentic-task-manager', 'tasks.json'), file);
    // --tolerant: read as atm serve's v1 route reads it, and answered in its shape (meta, tasks, problems)
    const r = spawnSync(ATM, ['list', '--derived', '--tolerant'], {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 1 << 30, // 10k tasks are tens of megabytes of views
      env: baseEnv(),
    });
    if (r.status !== 0) throw new Error(`atm list --derived failed (${r.status}): ${r.stderr}`);
    return JSON.stringify({ generated_at: at(0), bytes: file.length, ...JSON.parse(r.stdout) });
  } finally {
    removeDir(root);
  }
}

async function benchDashboard(n) {
  const require = createRequire(path.join(REPO, 'dashboard/package.json'));
  const { chromium } = require('@playwright/test');
  const html = fs.readFileSync(path.join(REPO, 'plugin/templates/tasks-dashboard.html'), 'utf8');
  const data = generate(n);
  let body = taskViews(data);
  const emptyArchive = JSON.stringify({ generated_at: at(0), bytes: 0, tasks: [] });
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    // Everything is served from memory, as atm serve's API (docs/multi-project-api.md) for one project: no server, no
    // request leaves the page
    const projects = JSON.stringify({
      projects: [{ id: 'bench', name: 'Bench', path: '/bench', missing: false, summary: null }],
    });
    await page.route('http://bench.local/**', (route) => {
      const url = new URL(route.request().url());
      if (url.pathname === '/api/projects') return route.fulfill({ body: projects, contentType: 'application/json' });
      if (url.pathname === '/api/v1/projects/bench/tasks')
        // No archive (`?archived=true`): the generated project has none
        return route.fulfill({
          body: url.searchParams.get('archived') === 'true' ? emptyArchive : body,
          contentType: 'application/json',
        });
      if (url.pathname === '/') return route.fulfill({ body: html, contentType: 'text/html' });
      notFound.add(url.pathname);
      return route.fulfill({ status: 404, body: '' }); // claims.json, worktrees.json, whoami
    });
    // What went wrong, when the board never shows: the requests answered 404, and the page's errors
    const notFound = new Set();
    const errors = new Set();
    page.on('pageerror', (e) => errors.add(e.message));
    page.on('console', (m) => m.type() === 'error' && errors.add(m.text()));
    // The Kanban board: the tab people keep open while agents work
    const t = performance.now();
    await page.goto('http://bench.local/#view=kanban');
    try {
      await page.locator('.kanban-card').first().waitFor({ timeout: 120_000 });
    } catch (e) {
      const shown = (await page.locator('body').innerText()).slice(0, 500);
      throw new Error(
        `${n} tasks: no Kanban card. 404: ${[...notFound].join(', ') || 'none'}. ` +
          `Errors: ${[...errors].join(' | ') || 'none'}. ` +
          `The page shows: ${shown}`,
        { cause: e },
      );
    }
    const load = performance.now() - t;

    const target = data.tasks.find((x) => x.status === 'todo');
    target.status = 'in_progress';
    body = taskViews(data);
    const reload = await page.evaluate(async (id) => {
      const start = performance.now();
      document.querySelector('#reload-btn').click();
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      while (!document.querySelector(`.kanban-column[data-status="in_progress"] .kanban-card[data-id="${id}"]`))
        await frame();
      await frame();
      return performance.now() - start;
    }, target.id);
    return { load, reload };
  } finally {
    await browser.close();
  }
}

const rows = [];
let failed = false;
for (const n of sizes) {
  const limits = LIMITS[n] ?? LIMITS[10000];
  const row = { tasks: n };
  if (wantCli) Object.assign(row, benchCli(n));
  if (wantDashboard) Object.assign(row, await benchDashboard(n));
  rows.push(row);
  for (const key of ['write', 'merge', 'load', 'reload']) {
    if (row[key] !== undefined && check && row[key] / 1000 > limits[key]) {
      console.error(`bench: ${n} tasks: ${key} took ${seconds(row[key])} s, over the ${limits[key]} s limit`);
      failed = true;
    }
  }
}

console.log('tasks    file      atm write  merge   dashboard load  reload');
for (const r of rows) {
  const col = (v, w) => String(v ?? '–').padEnd(w);
  console.log(
    col(r.tasks, 9) +
      col(r.size !== undefined ? `${(r.size / 1e6).toFixed(1)} MB` : undefined, 10) +
      col(r.write !== undefined ? `${seconds(r.write)} s` : undefined, 11) +
      col(r.merge !== undefined ? `${seconds(r.merge)} s` : undefined, 8) +
      col(r.load !== undefined ? `${seconds(r.load)} s` : undefined, 16) +
      col(r.reload !== undefined ? `${seconds(r.reload)} s` : undefined, 8),
  );
}
if (failed) process.exit(1);
