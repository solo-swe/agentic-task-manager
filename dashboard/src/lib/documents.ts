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

/** A file and, optionally, the lines to show (#doc=<path>:L<n> or :L<n>-L<m>) or the heading to open at (by its
 * slug, #doc=<path>#<slug>) */
export interface DocumentRef {
  path: string;
  lines?: [number, number];
  heading?: string;
  /** A text fragment, as written after the `#` (`:~:text=SR%2D2.1`): the document opens at the text (T-391) */
  text?: string;
}

/** One `text=` directive of a text fragment, decoded: [prefix-,]start[,end][,-suffix] */
export interface TextDirective {
  prefix?: string;
  start: string;
  end?: string;
  suffix?: string;
}

/**
 * The directives of a text fragment (https://web.dev/articles/text-fragments): what follows `:~:` in a URL's
 * fragment, `text=…` directives joined by `&`, each `[prefix-,]start[,end][,-suffix]` with its parts percent-encoded
 * (`-`, `,` and `&` in the text are `%2D`, `%2C`, `%26`). Anything else is left out; none: an empty list.
 */
export function parseTextFragment(fragment: string): TextDirective[] {
  const at = fragment.indexOf(':~:');
  if (at < 0) return [];
  const directives: TextDirective[] = [];
  for (const part of fragment.slice(at + 3).split('&')) {
    if (!part.startsWith('text=')) continue;
    const raw = part.slice(5).split(',');
    let decoded: string[];
    try {
      decoded = raw.map((p) => decodeURIComponent(p));
    } catch {
      continue;
    }
    const pieces = raw.map((p, i) => ({ raw: p, text: decoded[i] }));
    const directive: Partial<TextDirective> = {};
    if (pieces.length > 1 && pieces[0].raw.endsWith('-')) directive.prefix = pieces.shift()!.text.slice(0, -1);
    if (pieces.length > 1 && pieces[pieces.length - 1].raw.startsWith('-'))
      directive.suffix = pieces.pop()!.text.slice(1);
    if (pieces.length === 0 || pieces.length > 2 || !pieces[0].text) continue;
    directive.start = pieces[0].text;
    if (pieces[1]?.text) directive.end = pieces[1].text;
    directives.push(directive as TextDirective);
  }
  return directives;
}

/** What a text fragment matches, and the page's text it was matched in: a character's text node and offset */
const BLOCKS = /^(P|LI|H[1-6]|TD|TH|PRE|BLOCKQUOTE|DIV|SECTION|ARTICLE|TR|DT|DD|FIGCAPTION|SUMMARY)$/;

/**
 * The first place in `root`'s text a directive matches, as a Range, or null: case-insensitive, whitespace collapsed,
 * across inline elements; text in different blocks is apart. `end` makes it a range from the start to the end's
 * first match after it; `prefix` and `suffix` must be just before and after it (whitespace between allowed).
 */
export function findTextFragment(root: Node, directive: TextDirective): Range | null {
  const doc = root.ownerDocument ?? (root as Document);
  const walker = doc.createTreeWalker(root, 4 /* NodeFilter.SHOW_TEXT */);
  let text = '';
  const where: [Text, number][] = [];
  let lastBlock: Element | null = null;
  for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null) {
    let block: Element | null = node.parentElement;
    while (block && block !== root && !BLOCKS.test(block.tagName)) block = block.parentElement;
    if (lastBlock && block !== lastBlock && !text.endsWith(' ')) {
      text += ' ';
      where.push([node, 0]);
    }
    lastBlock = block;
    const value = node.data;
    for (let i = 0; i < value.length; i++) {
      const ch = /\s/.test(value[i]) ? ' ' : value[i].toLowerCase();
      if (ch === ' ' && (text.endsWith(' ') || text === '')) continue;
      text += ch;
      where.push([node, i]);
    }
  }
  const norm = (s: string) => s.replace(/\s+/g, ' ').trim().toLowerCase();
  const start = norm(directive.start);
  if (!start) return null;
  const end = directive.end ? norm(directive.end) : '';
  const prefix = directive.prefix ? norm(directive.prefix) : '';
  const suffix = directive.suffix ? norm(directive.suffix) : '';
  for (let i = text.indexOf(start); i >= 0; i = text.indexOf(start, i + 1)) {
    if (prefix && !text.slice(0, i).trimEnd().endsWith(prefix)) continue;
    let last = i + start.length;
    if (end) {
      const j = text.indexOf(end, last);
      if (j < 0) break;
      last = j + end.length;
    }
    if (suffix && !text.slice(last).trimStart().startsWith(suffix)) continue;
    const range = doc.createRange();
    range.setStart(where[i][0], where[i][1]);
    const [endNode, endOffset] = where[last - 1];
    range.setEnd(endNode, endOffset + 1);
    return range;
  }
  return null;
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

/** The document a location hash opens (#doc=<path>[:L<n>[-L<m>]][#<heading>]), if any */
export function parseDocHash(hash: string): DocumentRef | null {
  const value = hashParams(hash).get('doc');
  if (!value) return null;
  const sharp = value.indexOf('#');
  const target = sharp >= 0 ? value.slice(0, sharp) : value;
  const heading = sharp >= 0 ? value.slice(sharp + 1) : '';
  const colon = target.lastIndexOf(':');
  const lines = colon > 0 ? parseLines(target.slice(colon + 1)) : undefined;
  const path = resolvePath(lines ? target.slice(0, colon) : target);
  if (!path) return null;
  if (heading.startsWith(':~:')) return parseTextFragment(heading).length ? { path, text: heading } : { path };
  return { path, ...(lines ? { lines } : {}), ...(heading && !lines ? { heading } : {}) };
}

/** A ref as the value of #doc= */
export function docHashValue(ref: DocumentRef): string {
  const fragment = ref.text ?? ref.heading;
  return ref.path + (ref.lines ? `:${formatLines(ref.lines)}` : '') + (fragment ? `#${fragment}` : '');
}

/** `hash` with `ref` open (or none, for null), keeping the other parameters */
export function withDocHash(hash: string, ref: DocumentRef | null): string {
  const params = hashParams(hash);
  params.delete('doc');
  if (ref) params.set('doc', docHashValue(ref));
  return formatHash(params).replace(/%2F/g, '/').replace(/%3A/g, ':');
}

/**
 * The document a relative link names (V1a–V1c), or null for a link that isn't one: `href` resolved from `from` (the
 * open document's path; the project's root for task text), with what its fragment says: `#L40` or `#L40-L45` lines
 * (GitHub's form), any other fragment a heading, by its slug.
 */
export function referenceTo(href: string, from?: string): DocumentRef | null {
  const path = resolvePath(href, from);
  if (!path) return null;
  const sharp = href.indexOf('#');
  let fragment = sharp >= 0 ? href.slice(sharp + 1) : '';
  // A text fragment (`#:~:text=…`, after a heading or alone) wins; its parts stay encoded until parsed
  const directive = fragment.indexOf(':~:');
  if (directive >= 0) return parseTextFragment(fragment).length ? { path, text: fragment.slice(directive) } : { path };
  try {
    fragment = decodeURIComponent(fragment);
  } catch {
    fragment = '';
  }
  const lines = parseLines(fragment);
  if (lines) return { path, lines };
  return fragment ? { path, heading: fragment } : { path };
}

/** What Copy path copies: the path from the project's root, with the lines when some are selected (`docs/x.md#L40`),
 * to paste to an agent */
export function pathToCopy(ref: DocumentRef): string {
  if (ref.lines) return `${ref.path}#${formatLines(ref.lines)}`;
  const fragment = ref.text ?? ref.heading;
  return fragment ? `${ref.path}#${fragment}` : ref.path;
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
