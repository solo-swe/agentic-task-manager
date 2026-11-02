// Dev server only: `npm run dev` runs the real REST API (docs/layered-architecture.md §9, step 15), this checkout's
// atm-cli, so the dashboard in development talks to exactly what atm serve answers. ATM_DEV_DATA picks the task
// folders (.agentic-task-manager paths, separated like PATH); the default is this repo's own.
//
// The API server runs on a private ATM_HOME (a temporary folder, removed when Vite stops) that lists only those
// projects, so the developer's own `atm serve`, its registry and its port are never touched. Vite forwards the API's
// paths to it; everything else (the page, its modules, hot reload) stays Vite's.
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { request } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

/** This checkout's atm-cli, as the plugin ships it */
const ATM = fileURLToPath(new URL('../../plugin/scripts/atm.py', import.meta.url));
/** The paths atm serve answers (docs/multi-project-api.md §2); the rest is the dev server's own */
const API_PATHS = /^\/(api\/|p\/|__atm\/)/;
/** Where the dev API server starts looking for a free port: away from atm serve's 8765 */
const FIRST_PORT = 8865;

/** Start the API server for `dirs` on a private ATM_HOME. Returns how to reach it and how to stop it. */
function startApi(dirs: string[], log: (line: string) => void): { port: () => number | null; stop: () => void } {
  const home = mkdtempSync(path.join(tmpdir(), 'atm-dev-home-'));
  const env = { ...process.env, ATM_HOME: home };
  for (const dir of dirs) {
    try {
      execFileSync('python3', [ATM, 'projects', 'add', path.dirname(dir)], { env, stdio: 'pipe' });
    } catch (e) {
      log(`  atm: can't list ${dir}: ${String((e as { stderr?: Buffer }).stderr ?? e).trim()}`);
    }
  }
  const child: ChildProcess = spawn('python3', [ATM, '_serve-child', home, String(FIRST_PORT)], {
    env,
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  let port: number | null = null;
  const record = path.join(home, 'server.json');
  return {
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
    stop: () => {
      child.kill('SIGTERM');
      rmSync(home, { recursive: true, force: true });
    },
  };
}

export function devApi(defaultDir: string): Plugin {
  const dirs = (process.env.ATM_DEV_DATA || defaultDir).split(path.delimiter).filter(Boolean);
  return {
    name: 'atm-dev-api',
    apply: 'serve',
    configureServer(server) {
      const log = (line: string) => server.config.logger.info(line);
      const api = startApi(dirs, log);
      log(`  atm: the REST API (this checkout's atm-cli) for ${dirs.join(', ')}`);
      server.httpServer?.once('close', api.stop);
      process.once('exit', api.stop);
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
