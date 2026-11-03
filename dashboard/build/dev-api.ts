// Dev server only: `npm run dev` runs the real REST API (docs/layered-architecture.md §9, step 15; §11.9), this
// checkout's atm-cli, so the dashboard in development talks to exactly what atm serve answers. ATM_DEV_DATA picks the
// task folders (.agentic-task-manager paths, separated like PATH); the default is this repo's own.
//
// The API server runs on a private ATM_HOME (a temporary folder, removed when Vite stops) that lists only those
// projects, so the developer's own `atm serve`, its registry and its port are never touched. Vite forwards the API's
// paths to it; everything else (the page, its modules, hot reload) stays Vite's. When atm-cli is rebuilt
// (`npm run build:cli` writes plugin/scripts/atm.py), the API server restarts on it: no Vite restart needed.
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, watch, writeFileSync, type FSWatcher } from 'node:fs';
import { request } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

/** This checkout's plugin/scripts (atm.py, and atm-serve, the server process's launcher as `atm serve` starts it);
 * ATM_DEV_SCRIPTS points at another copy, for the dev server's own test */
const SCRIPTS = process.env.ATM_DEV_SCRIPTS || fileURLToPath(new URL('../../plugin/scripts', import.meta.url));
const ATM = path.join(SCRIPTS, 'atm.py');
const ATM_SERVE = path.join(SCRIPTS, 'atm-serve');
/** The paths atm serve answers (docs/multi-project-api.md §2); the rest is the dev server's own */
const API_PATHS = /^\/(api\/|p\/|__atm\/)/;
/** Where the dev API server starts looking for a free port: away from atm serve's 8765 */
const FIRST_PORT = 8865;
/** A rebuild writes atm.py in a few steps: restart once it has been quiet this long */
const RESTART_AFTER_MS = 300;

interface Api {
  home: string;
  port: () => number | null;
  restart: () => void;
  stop: () => void;
}

/** The registry entries for folders without a task file: listed as missing, as atm serve lists a project whose
 * folder is gone (`atm projects add` refuses such a folder) */
function listMissing(home: string, projects: string[]) {
  if (!projects.length) return;
  const file = path.join(home, 'projects.json');
  const registry = existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : { version: 1, projects: [] };
  const stamp = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
  for (const project of projects) {
    const id = `missing-${path.basename(project).replace(/[^A-Za-z0-9._-]/g, '-')}`.slice(0, 64);
    registry.projects.push({ id, name: path.basename(project), path: project, added_at: stamp, seen_at: stamp });
  }
  writeFileSync(file, JSON.stringify(registry, null, 2));
}

/** Start the API server for `dirs` on a private ATM_HOME. Returns how to reach it, restart it and stop it. */
function startApi(dirs: string[], log: (line: string) => void): Api {
  const home = mkdtempSync(path.join(tmpdir(), 'atm-dev-home-'));
  const env = { ...process.env, ATM_HOME: home };
  const missing: string[] = [];
  for (const dir of dirs) {
    const project = path.dirname(dir);
    if (!existsSync(path.join(dir, 'tasks.json'))) {
      missing.push(project);
      continue;
    }
    try {
      execFileSync('python3', [ATM, 'projects', 'add', project], { env, stdio: 'pipe' });
    } catch (e) {
      log(`  atm: can't list ${dir}: ${String((e as { stderr?: Buffer }).stderr ?? e).trim()}`);
    }
  }
  listMissing(home, missing);
  const record = path.join(home, 'server.json');
  let child: ChildProcess | null = null;
  let port: number | null = null;
  const launch = () => {
    rmSync(record, { force: true });
    port = null;
    child = spawn(ATM_SERVE, [home, String(FIRST_PORT)], { env, stdio: ['ignore', 'ignore', 'inherit'] });
  };
  launch();
  return {
    home,
    port: () => {
      if (port === null && existsSync(record)) {
        try {
          port = JSON.parse(readFileSync(record, 'utf8')).port ?? null;
        } catch {
          // being written: asked again on the next request
        }
      }
      return port;
    },
    restart: () => {
      const old = child;
      if (old && old.exitCode === null) {
        old.once('exit', launch);
        old.kill('SIGTERM');
      } else {
        launch();
      }
    },
    stop: () => {
      child?.kill('SIGTERM');
      rmSync(home, { recursive: true, force: true });
    },
  };
}

/** Restart the API server when atm.py changes (a rebuild), once the writes have settled */
function restartOnRebuild(api: Api, log: (line: string) => void): FSWatcher | null {
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    return watch(ATM, () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        log('  atm: atm.py changed, restarting the REST API');
        api.restart();
      }, RESTART_AFTER_MS);
    });
  } catch {
    return null; // no atm.py to watch: the API server says why it doesn't start
  }
}

export function devApi(defaultDir: string): Plugin {
  const dirs = (process.env.ATM_DEV_DATA || defaultDir).split(path.delimiter).filter(Boolean);
  return {
    name: 'atm-dev-api',
    apply: 'serve',
    configureServer(server) {
      const log = (line: string) => server.config.logger.info(line);
      const api = startApi(dirs, log);
      const watcher = restartOnRebuild(api, log);
      log(`  atm: the REST API (this checkout's atm-cli) for ${dirs.join(', ')}, on ATM_HOME ${api.home}`);
      const stop = () => {
        watcher?.close();
        api.stop();
      };
      server.httpServer?.once('close', stop);
      process.once('exit', stop);
      server.middlewares.use((req, res, next) => {
        if (!API_PATHS.test(req.url ?? '')) return next();
        const port = api.port();
        if (port === null) {
          res.statusCode = 503;
          res.setHeader('Content-Type', 'application/json');
          res.end(JSON.stringify({ error: 'the atm API is starting' }));
          return;
        }
        // The API checks the Host header (DNS rebinding): it is asked as itself
        const headers = { ...req.headers, host: `localhost:${port}` };
        const forwarded = request({ host: '127.0.0.1', port, path: req.url, method: req.method, headers }, (answer) => {
          res.writeHead(answer.statusCode ?? 502, answer.headers);
          answer.pipe(res);
        });
        forwarded.on('error', (e) => {
          res.statusCode = 502;
          res.end(JSON.stringify({ error: `the atm API: ${e.message}` }));
        });
        req.pipe(forwarded);
      });
    },
  };
}
