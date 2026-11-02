// Dev server only: play atm serve's API (docs/multi-project-api.md) for one or more task folders, so `npm run dev`
// shows real data with hot reload. ATM_DEV_DATA picks the folders (.agentic-task-manager paths, separated like PATH);
// the default is this repo's own .agentic-task-manager/.
import { execFile } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

/** This checkout's atm-cli, as the plugin ships it */
const ATM = fileURLToPath(new URL('../../plugin/scripts/atm.py', import.meta.url));

/** The project's summary from atm-cli itself (`atm summary`, what atm serve lists), so the dev server's numbers and
 * Next up are atm's own (T-321); null when Python or the project can't give one, as atm serve does */
function summary(root: string): Promise<unknown> {
  return new Promise((resolve) => {
    execFile('python3', [ATM, 'summary'], { cwd: root, timeout: 10_000 }, (error, stdout) => {
      try {
        resolve(error ? null : JSON.parse(stdout));
      } catch {
        resolve(null);
      }
    });
  });
}

// worktrees.json is never in a task folder, so it answers 404: the dev server shows no linked worktrees
const FILES = new Set(['tasks.json', 'claims.json', 'tasks-archive.json', 'worktrees.json']);

interface DevProject {
  id: string;
  name: string;
  dir: string;
}

/** Each folder's project: id and name from its tasks.json meta, falling back to the folder's place in the list */
async function devProjects(dirs: string[]): Promise<DevProject[]> {
  return Promise.all(
    dirs.map(async (dir, i) => {
      let meta: { id?: unknown; project?: unknown } = {};
      try {
        meta = JSON.parse(await readFile(path.join(dir, 'tasks.json'), 'utf8')).meta ?? {};
      } catch {
        // missing or unreadable: listed as missing below
      }
      const root = path.dirname(dir);
      return {
        id: typeof meta.id === 'string' && meta.id ? meta.id : `dev${i}`,
        name: typeof meta.project === 'string' && meta.project ? meta.project : path.basename(root),
        dir,
      };
    }),
  );
}

/** atm serve's api/search (server.py api_search, T-258): per project, the tasks in which every word of the query
 * appears, in the ID, name, description, type, labels, notes or the project's name */
async function search(projects: DevProject[], query: string): Promise<Record<string, string[]>> {
  const terms = query.slice(0, 200).toLowerCase().split(/\s+/).filter(Boolean);
  const results: Record<string, string[]> = {};
  if (!terms.length) return results;
  for (const p of projects) {
    let tasks: Record<string, unknown>[];
    try {
      tasks = JSON.parse(await readFile(path.join(p.dir, 'tasks.json'), 'utf8')).tasks ?? [];
    } catch {
      continue;
    }
    const strings = (t: Record<string, unknown>) => [
      p.name,
      t.id,
      t.name,
      t.description,
      t.type,
      ...(Array.isArray(t.labels) ? t.labels : []),
      ...(Array.isArray(t.notes) ? t.notes.map((n: { text?: unknown }) => n?.text) : []),
    ];
    const ids = tasks
      .filter((t) => typeof t.id === 'string')
      .filter((t) => {
        const text = strings(t)
          .filter((s) => typeof s === 'string')
          .join(' ')
          .toLowerCase();
        return terms.every((term) => text.includes(term));
      })
      .map((t) => t.id as string);
    if (ids.length) results[p.id] = ids.slice(0, 200);
  }
  return results;
}

export function devData(defaultDir: string): Plugin {
  const dirs = (process.env.ATM_DEV_DATA || defaultDir).split(path.delimiter).filter(Boolean);
  return {
    name: 'atm-dev-data',
    apply: 'serve',
    configureServer(server) {
      server.config.logger.info(`  atm: serving the API for ${dirs.join(', ')}`);
      server.middlewares.use(async (req, res, next) => {
        const url = req.url?.split('?')[0] ?? '';
        const json = (status: number, body: unknown) => {
          res.statusCode = status;
          res.setHeader('Content-Type', 'application/json');
          res.setHeader('Cache-Control', 'no-store');
          res.end(JSON.stringify(body));
        };
        if (url === '/api/projects') {
          const projects = await devProjects(dirs);
          const listed = await Promise.all(
            projects.map(async (p) => {
              const missing = !(await stat(path.join(p.dir, 'tasks.json')).catch(() => null));
              const root = path.dirname(p.dir);
              return { id: p.id, name: p.name, path: root, missing, summary: missing ? null : await summary(root) };
            }),
          );
          json(200, { generated_at: new Date().toISOString().replace(/\.\d+Z$/, 'Z'), projects: listed });
          return;
        }
        if (url === '/api/search') {
          json(
            200,
            await search(await devProjects(dirs), new URL(req.url ?? '', 'http://x').searchParams.get('q') ?? ''),
          );
          return;
        }
        // atm serve's v1 tasks route (docs/multi-project-api.md §2.5): the tasks as views, with the project's meta and
        // size. Without the derived values for now: the dashboard doesn't read them yet, and the dev server will run
        // the real API instead (docs/layered-architecture.md §9, step 15)
        const v1 = /^\/api\/v1\/projects\/([^/]+)\/tasks$/.exec(url);
        if (v1) {
          const project = (await devProjects(dirs)).find((p) => p.id === decodeURIComponent(v1[1]));
          if (!project) return json(404, { error: 'unknown project' });
          try {
            const file = path.join(project.dir, 'tasks.json');
            const { size, mtimeMs } = await stat(file);
            const etag = `"v1-${size.toString(16)}-${Math.round(mtimeMs * 1e6).toString(16)}"`;
            res.setHeader('ETag', etag);
            if (req.headers['if-none-match'] === etag) {
              res.statusCode = 304;
              res.end();
              return;
            }
            const data = JSON.parse(await readFile(file, 'utf8')) as { meta?: unknown; tasks?: unknown[] };
            const tasks = (Array.isArray(data.tasks) ? data.tasks : []).map((task) => ({ task, derived: {} }));
            json(200, { generated_at: new Date().toISOString(), meta: data.meta ?? {}, bytes: size, tasks });
          } catch {
            json(404, { error: 'not found' });
          }
          return;
        }
        const match = /^\/p\/([^/]+)\/([^/]+)$/.exec(url);
        if (!match || !FILES.has(match[2])) return next();
        const project = (await devProjects(dirs)).find((p) => p.id === decodeURIComponent(match[1]));
        if (!project) return json(404, { error: 'unknown project' });
        try {
          const file = path.join(project.dir, match[2]);
          // Like atm serve: an ETag from size and mtime, and a 304 when the dashboard already has this version (D-5.1)
          const { size, mtimeMs } = await stat(file);
          const etag = `"${size.toString(16)}-${Math.round(mtimeMs * 1e6).toString(16)}"`;
          res.setHeader('ETag', etag);
          res.setHeader('Cache-Control', 'no-store');
          if (req.headers['if-none-match'] === etag) {
            res.statusCode = 304;
            res.end();
            return;
          }
          const body = await readFile(file);
          res.setHeader('Content-Type', 'application/json');
          res.end(body);
        } catch {
          json(404, { error: 'not found' });
        }
      });
    },
  };
}
