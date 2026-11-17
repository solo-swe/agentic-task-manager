// `pnpm run dev`'s REST API (dashboard/build/dev-api.ts, T-386, docs/layered-architecture.md §11.9): `vite dev` runs
// this checkout's Rust atm (`atm _serve`, T-478) on a private state folder (ATM_STATE_DIR) and forwards the API's paths to it. Started here
// on a temporary project and a folder without a task file; a rebuild restarts it; stopping Vite leaves no server and no
// home behind.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { test } from 'node:test';
import { makeProject, removeDir, REPO, shown, tempDir } from './helpers.mjs';

const DASHBOARD = path.join(REPO, 'dashboard');
const VITE = path.join(DASHBOARD, 'node_modules', 'vite', 'bin', 'vite.js');
/** The Rust atm the dev server runs, whichever implementation the suite tests */
const BINARY = path.join(REPO, 'target/debug', process.platform === 'win32' ? 'atm.exe' : 'atm');

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** A port nothing listens on */
function freePort() {
  return new Promise((resolve) => {
    const server = net.createServer().listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

/** Wait until `check()` gives a value (polling), or fail after `ms` */
async function until(check, what, ms = 20_000) {
  const end = Date.now() + ms;
  for (;;) {
    const value = await check();
    if (value) return value;
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await sleep(100);
  }
}

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const NAME = 'vite dev serves the real API for ATM_DEV_DATA, restarts it on a rebuild, and leaves nothing running';
const skip =
  (!fs.existsSync(VITE) && 'needs `pnpm install` in dashboard/') ||
  (!fs.existsSync(BINARY) && 'needs the Rust atm (`cargo build`)');

test(NAME, { skip }, async () => {
  const project = makeProject({
    files: {
      '.agentic-task-manager/tasks.json': {
        meta: { id: 'devapi01', project: 'Dev API' },
        tasks: [{ id: 'T-1', name: 'One', type: 'feature', status: 'todo' }],
      },
    },
  });
  const gone = tempDir('atm-dev-gone-');
  // A copy of the binary, so the rebuild below touches the copy, never target/
  const bin = tempDir('atm-dev-bin-');
  const atm = path.join(bin, path.basename(BINARY));
  fs.copyFileSync(BINARY, atm);
  const port = await freePort();
  const data = [path.join(project, '.agentic-task-manager'), path.join(gone, '.agentic-task-manager')];
  const vite = spawn(process.execPath, [VITE, '--port', String(port), '--strictPort'], {
    cwd: DASHBOARD,
    env: { ...process.env, ATM_DEV_DATA: data.join(path.delimiter), ATM_DEV_BIN: atm },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  vite.stdout.on('data', (chunk) => (output += chunk));
  vite.stderr.on('data', (chunk) => (output += chunk));
  const exited = new Promise((resolve) => vite.once('exit', resolve));
  try {
    // Vite's first start on a fresh checkout builds its dependency cache (node_modules/.vite): 5 s, or past 20 s on a
    // slow Windows runner (CI run 44016638568). A wait for a cold tool to come up, not a budget: 60 s there (Hakan,
    // 2026-11-17), every wait after it as before
    const startWait = process.platform === 'win32' ? 60_000 : 20_000;
    const home = await until(() => /on ATM_STATE_DIR (\S+)/.exec(output)?.[1], 'the dev API to start', startWait);
    const base = `http://localhost:${port}`;
    const projects = async () => {
      const answer = await fetch(`${base}/api/projects`).catch(() => null);
      return answer?.ok ? (await answer.json()).projects : null;
    };
    const listed = await until(projects, 'api/projects through vite');
    // By the path as atm shows it (on Windows, the dev server lists the gone folder with `\`)
    const byPath = Object.fromEntries(listed.map((p) => [shown(p.path), p]));
    assert.equal(byPath[shown(project)].missing, false);
    assert.equal(byPath[shown(project)].summary.total, 1);
    assert.equal(byPath[shown(gone)].missing, true); // no task file: listed as missing, as atm serve lists a gone folder
    const tasks = await (await fetch(`${base}/api/v1/projects/devapi01/tasks`)).json();
    assert.deepEqual(
      tasks.tasks.map((v) => [v.task.id, v.derived.ready]),
      [['T-1', true]],
    );
    // The assets too: the CompMD runtime the details panel loads (T-284), which atm embeds from the build
    const [version] = fs.readdirSync(path.join(DASHBOARD, 'dist/assets/compmd'));
    const runtime = await fetch(`${base}/assets/compmd/${version}/compmd.js`);
    assert.equal(runtime.status, 200);
    assert.equal(runtime.headers.get('content-type'), 'text/javascript; charset=utf-8');

    // The page SvelteKit renders (T-428) runs under the policy of the page as sent: each of its inline scripts by
    // hash, as `atm serve` allows the built page's, and no 'unsafe-inline' for scripts
    const page = await fetch(`${base}/`);
    assert.equal(page.status, 200);
    const html = await page.text();
    const csp = page.headers.get('content-security-policy') ?? '';
    const scriptSrc = csp.split(';').find((d) => d.trim().startsWith('script-src')) ?? '';
    const inline = [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)].filter(
      ([, attrs]) => !attrs.includes('src='),
    );
    assert.ok(inline.length >= 2, 'the theme script and SvelteKit’s start-up script');
    for (const [, , body] of inline)
      assert.ok(scriptSrc.includes(`'sha256-${createHash('sha256').update(body).digest('base64')}'`), scriptSrc);
    assert.doesNotMatch(scriptSrc, /unsafe-inline/);

    // The modules the page then imports come with their head: Vite's own and SvelteKit's start-up module, the first
    // the page loads. Node sends a head through `writeHead` from inside `end`, which the page policy's hold on
    // `writeHead` once swallowed, so every module came without one and the page stayed blank
    const entry = /import\("([^"]+\/kit\/src\/runtime\/client\/entry\.js)"\)/.exec(html)?.[1];
    assert.ok(entry, 'SvelteKit’s start-up module, in the page');
    for (const module of ['/@vite/client', new URL(entry, `${base}/`).pathname]) {
      const answer = await fetch(`${base}${module}`);
      assert.equal(answer.status, 200, module);
      assert.match(answer.headers.get('content-type') ?? '', /javascript/, module);
      assert.equal(answer.headers.get('x-content-type-options'), 'nosniff', module);
      // Asked again with its ETag: a 304 with no policy, so the one the browser stored with it stands
      const again = await fetch(`${base}${module}`, { headers: { 'if-none-match': answer.headers.get('etag') ?? '' } });
      assert.equal(again.status, 304, module);
      assert.equal(again.headers.get('content-security-policy'), null, module);
    }

    // A reload: the browser asks for the page with its ETag. It is rendered again, with its inline scripts' hashes in
    // its policy, never a 304 whose policy lacks them (which the browser would put on the stored page, blocking its
    // inline scripts)
    const reload = await fetch(`${base}/`, {
      headers: { accept: 'text/html', 'if-none-match': page.headers.get('etag') ?? '' },
    });
    assert.equal(reload.status, 200);
    assert.equal(reload.headers.get('content-security-policy'), csp);

    // A rebuild (the binary replaced, as cargo does): the API server restarts on it, and answers again
    const record = path.join(home, 'server.json');
    const first = JSON.parse(fs.readFileSync(record, 'utf8')).pid;
    fs.copyFileSync(BINARY, `${atm}.new`);
    // Windows can't replace a running program's file, only move it aside
    if (process.platform === 'win32') fs.renameSync(atm, `${atm}.old`);
    fs.renameSync(`${atm}.new`, atm);
    const restarted = () => {
      try {
        const pid = JSON.parse(fs.readFileSync(record, 'utf8')).pid;
        return pid !== first && pid;
      } catch {
        return null;
      }
    };
    const second = await until(restarted, 'the API server to restart');
    assert.equal(alive(first), false);
    assert.ok(await until(projects, 'api/projects after the restart'));

    // Stopping Vite stops the API server and removes its home. Windows has no SIGTERM: Vite is ended at once, its exit
    // handler never runs, so there the test stops the server itself, unless it has ended with Vite already (ESRCH)
    vite.kill('SIGTERM');
    await exited;
    if (process.platform === 'win32') {
      try {
        process.kill(second);
      } catch (e) {
        if (e.code !== 'ESRCH') throw e;
      }
      await until(() => !alive(second), 'the API server to end', 5_000);
      removeDir(home);
    }
    await until(() => !alive(second), 'the API server to stop', 5_000);
    assert.equal(fs.existsSync(home), false);
  } catch (e) {
    e.message += `\n--- vite's output ---\n${output}`;
    throw e;
  } finally {
    if (vite.exitCode === null) vite.kill('SIGKILL');
    removeDir(project);
    removeDir(gone);
    removeDir(bin);
  }
});
