// The dashboard's client layer (docs/layered-architecture.md §1, §9 step 5): the only module that makes requests.
// Everything the dashboard reads comes from atm serve's REST API (docs/multi-project-api.md), through here; the rest
// of the dashboard takes a `Fetch` it can be given (tests pass their own) and never calls the network itself. An
// ESLint rule (eslint.config.js) keeps `fetch` out of every other module. URLs are relative to the page, which the
// server serves at /.

/** How the dashboard makes a request: the browser's fetch, or a test's */
export type Fetch = typeof fetch;

/** The browser's fetch, the default transport */
export const browserFetch: Fetch = (input, init) => fetch(input, init);

export type ProjectFile = 'tasks.json' | 'claims.json' | 'tasks-archive.json' | 'worktrees.json';

/** Where a project's file is served */
export function projectFileUrl(project: string, file: ProjectFile): string {
  return `p/${encodeURIComponent(project)}/${file}`;
}

/** A project's tasks with what atm derives for them, and its meta (GET api/v1/projects/<id>/tasks, T-364) */
export function v1TasksUrl(project: string): string {
  return `api/v1/projects/${encodeURIComponent(project)}/tasks`;
}

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
