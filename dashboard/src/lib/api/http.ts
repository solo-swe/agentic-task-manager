// The dashboard's client layer (docs/layered-architecture.md §1, §9 step 5): the only module that makes requests.
// Everything the dashboard reads comes from atm serve's REST API (docs/multi-project-api.md), through here; the rest
// of the dashboard takes a `Fetch` it can be given (tests pass their own) and never calls the network itself. An
// ESLint rule (eslint.config.js) keeps `fetch` out of every other module. URLs are from the server's root, since the
// page is served at several paths (/projects/<id>/…, T-429).

import { isTauri } from '@tauri-apps/api/core';
import { ipcFetch } from './ipc';
import { workspacePrefix } from './workspace';

/** How the dashboard makes a request: the browser's fetch, or a test's */
export type Fetch = typeof fetch;

/** Whether the dashboard runs in the atm app (Tauri's window), picked once at start: its requests are then the app's
 * commands (api/ipc.ts), not HTTP */
export const inApp = typeof window !== 'undefined' && isTauri();

/** The transport: the browser's fetch to atm serve, or in the app the IPC one */
export const browserFetch: Fetch = (input, init) => (inApp ? ipcFetch(input) : fetch(input, init));

/** A file of atm's own build, in the app from its asset scheme (`atm:`, never the IPC): the packs' modules the page
 * gives CompMD's sandbox as code (lib/compmd.ts, T-503) */
export const assetFetch: Fetch = (input, init) => fetch(input, init);

/** What atm serve computes for a project under p/<id>/ (the task files themselves come only through the v1 route) */
export type ProjectFile = 'claims.json' | 'worktrees.json';

/** A project's own route in the current workspace: `default`'s v1 one, or another workspace's v2 one */
function projectRoute(project: string, route: string): string {
  const prefix = workspacePrefix();
  const id = encodeURIComponent(project);
  return prefix ? `${prefix}/projects/${id}/${route}` : `/api/v1/projects/${id}/${route}`;
}

/** Where a project's file is served */
export function projectFileUrl(project: string, file: ProjectFile): string {
  const prefix = workspacePrefix();
  return prefix
    ? `${prefix}/projects/${encodeURIComponent(project)}/${file.replace(/\.json$/, '')}`
    : `/p/${encodeURIComponent(project)}/${file}`;
}

/** A project's tasks with what atm derives for them, and its meta (GET api/v1/projects/<id>/tasks, T-364) */
export function v1TasksUrl(project: string): string {
  return projectRoute(project, 'tasks');
}

/** A file of the project for the document viewer: what it is, and the CompMD document to render (T-392) */
export function v1DocumentUrl(project: string, path: string): string {
  return `${projectRoute(project, 'documents')}?path=${encodeURIComponent(path)}`;
}

/** An image file of the project, its bytes, for an <img> (T-392) */
export function v1FileUrl(project: string, path: string): string {
  return `${projectRoute(project, 'files')}?path=${encodeURIComponent(path)}`;
}

/** The current workspace's projects, with their summaries (api/projects for `default`) */
export function projectsUrl(): string {
  const prefix = workspacePrefix();
  return prefix ? `${prefix}/projects` : '/api/projects';
}

/** Search across the current workspace's projects (api/search for `default`) */
export function searchUrl(): string {
  const prefix = workspacePrefix();
  return prefix ? `${prefix}/search` : '/api/search';
}

/** The workspaces, and the one opened last (docs/multi-project-api.md §2.8) */
export const WORKSPACES_URL = '/api/v2/workspaces';

/** What the server said identifies this version of a file, sent back to ask "has it changed?" */
export interface Validators {
  etag?: string;
  lastModified?: string;
}

export type FileResult = { unchanged: true } | { unchanged: false; text: string; validators: Validators };

/**
 * Fetches a URL from the server (D-5.1). With the validators of the last load, the request is conditional
 * (If-None-Match / If-Modified-Since): a 304 means unchanged. The browser's cache is bypassed so the 304 reaches us
 * instead of a cached copy; atm serve and the Vite dev server both answer it. `missing` is returned for a 404 when
 * given; other error responses reject with "HTTP <status>".
 */
export async function fetchFile(
  url: string,
  fetchFn: Fetch,
  since?: Validators,
  missing?: string,
): Promise<FileResult> {
  const headers: Record<string, string> = {};
  if (since?.etag) headers['If-None-Match'] = since.etag;
  if (since?.lastModified) headers['If-Modified-Since'] = since.lastModified;
  const resp = await fetchFn(url, { cache: 'no-store', headers });
  if (resp.status === 304 && since) return { unchanged: true };
  if (resp.status === 404 && missing !== undefined) return { unchanged: false, text: missing, validators: {} };
  if (!resp.ok) throw new Error('HTTP ' + resp.status);
  const validators: Validators = {};
  const etag = resp.headers.get('ETag');
  const lastModified = resp.headers.get('Last-Modified');
  if (etag) validators.etag = etag;
  if (lastModified) validators.lastModified = lastModified;
  return { unchanged: false, text: await resp.text(), validators };
}

/** fetchFile for a project's file (p/<id>/<file>) */
export function fetchProjectFile(
  project: string,
  file: ProjectFile,
  fetchFn: Fetch,
  since?: Validators,
  missing?: string,
): Promise<FileResult> {
  return fetchFile(projectFileUrl(project, file), fetchFn, since, missing);
}
