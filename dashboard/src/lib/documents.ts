// The document viewer (docs/compmd-viewer-design.md): which file a reference names, the URL hash that says what is
// open (#doc=<path>:L<n>), and loading a file through the documents route (T-392). atm renders nothing itself: the
// route gives the CompMD document atm wrapped the file in, and CompMd.svelte renders it.
import { type Fetch, v1DocumentUrl, type Validators } from './api/http';
import { formatHash, hashParams } from './hash';
import { markdownPlainText } from './plain-text';

/** What the documents route answers (docs/multi-project-api.md §2.6) */
export interface ProjectDocument {
  path: string;
  kind: 'markdown' | 'code' | 'image' | 'binary' | 'text';
  language: string | null;
  /** The file as it is; null for an image or a binary file */
  text: string | null;
  /** The CompMD document to render */
  document: string;
  /** Lines the wrapping adds before the file's first (a fence's opening line) */
  line_offset: number;
  lines: number;
  bytes: number;
  modified_at: string;
  /** Whether git tracks the file; null outside git */
  tracked: boolean | null;
  /** The current commit; null outside git */
  commit: string | null;
}

/** A file and, optionally, the lines to show: from #doc=<path>:L<n> or :L<n>-L<m> */
export interface DocumentRef {
  path: string;
  lines?: [number, number];
}

/** Why a document can't be shown (V6): no such file, one atm doesn't show, or the request failed */
export type DocumentError = 'missing' | 'refused' | 'failed';

export type DocumentResult =
  | { unchanged: true }
  | { unchanged: false; document: ProjectDocument; validators: Validators }
  | { unchanged: false; error: DocumentError; message: string };

/**
 * The project path a reference names (§3): `href` relative to `from` (the open document's path), or to the project's
 * root when there is none (task text). `./` and a leading `/` mean the root's; `..` may not leave the project. A link
 * to anywhere else (a scheme, `//host`, a path leaving the project) is null. A `#` fragment is left out.
 */
export function resolvePath(href: string, from?: string): string | null {
  const target = href.trim().split('#')[0].split('?')[0];
  if (!target || /^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('//') || target.includes('\\')) return null;
  let decoded: string;
  try {
    decoded = decodeURIComponent(target);
  } catch {
    return null;
  }
  const base = decoded.startsWith('/') || !from ? [] : from.split('/').slice(0, -1);
  const parts = [...base];
  for (const segment of decoded.split('/')) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') {
      if (!parts.length) return null;
      parts.pop();
    } else parts.push(segment);
  }
  return parts.length ? parts.join('/') : null;
}

const LINES = /^L(\d+)(?:-L?(\d+))?$/;

/** The lines a `L40` or `L40-L45` names, in order; undefined for anything else */
export function parseLines(text: string): [number, number] | undefined {
  const m = LINES.exec(text);
  if (!m) return undefined;
  const start = Number(m[1]);
  const end = m[2] ? Number(m[2]) : start;
  return start >= 1 && end >= 1 ? [Math.min(start, end), Math.max(start, end)] : undefined;
}

/** `L40` or `L40-L45` */
export function formatLines(lines: [number, number]): string {
  return lines[0] === lines[1] ? `L${lines[0]}` : `L${lines[0]}-L${lines[1]}`;
}

/** The document a location hash opens (#doc=<path>[:L<n>[-L<m>]]), if any */
export function parseDocHash(hash: string): DocumentRef | null {
  const value = hashParams(hash).get('doc');
  if (!value) return null;
  const colon = value.lastIndexOf(':');
  const lines = colon > 0 ? parseLines(value.slice(colon + 1)) : undefined;
  const path = resolvePath(lines ? value.slice(0, colon) : value);
  return path ? { path, ...(lines ? { lines } : {}) } : null;
}

/** `hash` with `ref` open (or none, for null), keeping the other parameters */
export function withDocHash(hash: string, ref: DocumentRef | null): string {
  const params = hashParams(hash);
  params.delete('doc');
  if (ref) params.set('doc', ref.lines ? `${ref.path}:${formatLines(ref.lines)}` : ref.path);
  return formatHash(params).replace(/%2F/g, '/').replace(/%3A/g, ':');
}

/** What Copy path copies: the path from the project's root, with the lines when some are selected (`docs/x.md#L40`),
 * to paste to an agent */
export function pathToCopy(ref: DocumentRef): string {
  return ref.lines ? `${ref.path}#${formatLines(ref.lines)}` : ref.path;
}

/** The viewer's title: a Markdown document's first heading (after its own frontmatter), else the file's name */
export function documentTitle(doc: Pick<ProjectDocument, 'path' | 'kind' | 'text'>): string {
  const name = doc.path.split('/').pop() ?? doc.path;
  if (doc.kind !== 'markdown' || !doc.text) return name;
  let lines = doc.text.split(/\r?\n/);
  if (lines[0] === '---') {
    const end = lines.indexOf('---', 1);
    lines = end > 0 ? lines.slice(end + 1) : lines;
  }
  let fenced = false;
  for (const line of lines) {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
    else if (!fenced && /^#{1,6}\s+\S/.test(line)) return markdownPlainText(line).trim() || name;
  }
  return name;
}

/** The server's reason, from an error answer's {"error": …} */
async function reason(resp: Response): Promise<string> {
  try {
    const body = (await resp.json()) as { error?: unknown };
    return typeof body.error === 'string' ? body.error : `HTTP ${resp.status}`;
  } catch {
    return `HTTP ${resp.status}`;
  }
}

/** A project's file through the documents route; conditional with the last load's ETag (a 304 is `unchanged`). A
 * missing file (404) and one atm won't show (400, 403, 413) are results, not failures: the viewer says which. */
export async function loadDocument(
  project: string,
  path: string,
  fetchFn: Fetch,
  since?: Validators,
): Promise<DocumentResult> {
  const headers: Record<string, string> = {};
  if (since?.etag) headers['If-None-Match'] = since.etag;
  let resp: Response;
  try {
    resp = await fetchFn(v1DocumentUrl(project, path), { cache: 'no-store', headers });
  } catch (e) {
    return { unchanged: false, error: 'failed', message: (e as Error).message };
  }
  if (resp.status === 304 && since) return { unchanged: true };
  if (resp.status === 404) return { unchanged: false, error: 'missing', message: await reason(resp) };
  if ([400, 403, 413].includes(resp.status)) return { unchanged: false, error: 'refused', message: await reason(resp) };
  if (!resp.ok) return { unchanged: false, error: 'failed', message: await reason(resp) };
  const etag = resp.headers.get('ETag');
  return {
    unchanged: false,
    document: (await resp.json()) as ProjectDocument,
    validators: etag ? { etag } : {},
  };
}

/** Where the viewer is and how big, kept per browser */
export interface ViewerGeometry {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const VIEWER_MIN = { w: 360, h: 240 };
/** Below this window width the viewer fills the window, without moving or resizing */
export const VIEWER_NARROW = 720;

/** `g` kept inside a window of `width` × `height`: at least the minimum size, no bigger than the window, and moved
 * back in when it sticks out */
export function clampGeometry(g: ViewerGeometry, width: number, height: number): ViewerGeometry {
  const w = Math.max(Math.min(g.w, width), Math.min(VIEWER_MIN.w, width));
  const h = Math.max(Math.min(g.h, height), Math.min(VIEWER_MIN.h, height));
  return {
    w,
    h,
    x: Math.min(Math.max(0, g.x), Math.max(0, width - w)),
    y: Math.min(Math.max(0, g.y), Math.max(0, height - h)),
  };
}

/** Where the viewer first opens: as wide as the details panel, a little lower and to the right of it, so the task's
 * edge shows beneath */
export function defaultGeometry(width: number, height: number): ViewerGeometry {
  const w = Math.min(680, width - 48);
  const h = Math.round(height * 0.8);
  return clampGeometry({ x: Math.round((width - w) / 2) + 28, y: Math.round(height * 0.1) + 28, w, h }, width, height);
}

/** A stored geometry, if it is one */
export function isGeometry(value: unknown): value is ViewerGeometry {
  if (!value || typeof value !== 'object') return false;
  const g = value as Record<string, unknown>;
  return ['x', 'y', 'w', 'h'].every((k) => typeof g[k] === 'number' && Number.isFinite(g[k]));
}
