// CompMD renders all Markdown the dashboard shows (docs/compmd-rendering-design.md): task text in the details panel,
// and Help and What's new, which come with the plugin. Task text is untrusted: anyone who can change tasks.json wrote
// it. This module builds what CompMD is given (the frontmatter the dashboard puts in front of every text, and the
// request, §3 and §3b), loads CompMD from atm serve's asset folder (§2), and makes the rendered links safe (§3).
// Types only: the processor itself is in the module served from the asset folder, never in the dashboard's bundle
import type { Request } from '@compmd/processor';
import { version as compmdVersion } from 'virtual:atm-compmd';
import type { Task } from './types';
import { addressOf, isPagePath } from './address';

/** Links that are safe to follow: web and mail addresses, and anchors in the page. */
export function safeHref(href: string): boolean {
  return /^(https?:|mailto:|#)/i.test(href.trim());
}

/** "Filters and saved views" → "filters-and-saved-views": what a `#` link names a heading by (CompMD gives headings no
 * IDs, so a link finds its heading by this) */
export function slug(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

/** The record `md.data.task` holds: the task's fields and what atm derives for it, a derived value replacing the
 * stored one of the same name (the effective priority). Only this task: never another task's or project's data. */
export function taskData(task: Task): Record<string, unknown> {
  const { derived, ...stored } = task;
  return { ...stored, ...(derived ?? {}) };
}

/** The frontmatter in front of every text: the same for each, never stored in tasks.json (§3b). Written as JSON,
 * which YAML reads as it is, so no value needs quoting rules. A task's text gets its task as `md.data.task`; a text
 * that isn't a task's (Help) gets none. `host:task` has no `schema`: in CompMD 0.3.0 a schema makes data a list of
 * records, and the task is one record; its values are typed when evaluated instead. Packs aren't here:
 * compmd.config.json makes the Mermaid pack ambient (§3c). */
export function frontmatter(withTask = true): string {
  const md = withTask ? { version: 1, data: [{ from: 'host:task', as: 'task' }] } : { version: 1 };
  return `---\n${JSON.stringify({ md })}\n---\n`;
}

/** How many lines the frontmatter takes: a diagnostic's line in the text is its line in the document less these */
export const FRONTMATTER_LINES = frontmatter().split('\n').length - 1;

export const DOCUMENT = 'task.comp.md';

/** @compmd/vite's request (`virtual:compmd`, from compmd.config.json): the packs, which are ambient, and their policy.
 * The module served from the asset folder gives it; tests give it from the same configuration. */
export type BaseRequest = Pick<Request, 'packs' | 'ambient' | 'policy'>;

/** What CompMD's processor is given for one task text (§3): @compmd/vite's request (the plugin's packs, each strict:
 * its components run in a strict frame with no network), and the dashboard's own parts: the text alone, behind the
 * frontmatter, with no files to import and no remote data, and the task's own data, if it is a task's, as the one
 * `host:` source */
export function compmdRequest(
  base: BaseRequest,
  text: string,
  task: Task | undefined,
  now = new Date(),
  timeZone = localTimeZone(),
) {
  return {
    ...base,
    path: DOCUMENT,
    files: { [DOCUMENT]: frontmatter(!!task) + text },
    now: now.toISOString(),
    tz: timeZone,
    allowedOrigins: [] as string[],
    host: task ? { task: taskData(task) } : {},
  };
}

/** What CompMD's processor is given for a file of the project in the document viewer (docs/compmd-viewer-design.md
 * §5): the document as atm wrapped it, under its own path, with its own frontmatter (none of the dashboard's), and the
 * rest as for task text: the plugin's packs, only this file (its imports of other files fail and show their
 * fallbacks), no remote data, no `host:` data */
export function documentRequest(
  base: BaseRequest,
  path: string,
  text: string,
  now = new Date(),
  timeZone = localTimeZone(),
) {
  return {
    ...base,
    path,
    files: { [path]: text },
    now: now.toISOString(),
    tz: timeZone,
    allowedOrigins: [] as string[],
    host: {},
    // Each node carries the source range it came from (spec §12.3): CompMD places each line of the text from it, for
    // the gutter's line numbers (the view's lines(), T-406)
    origins: true,
  };
}

/** A box, in CSS pixels from the top left of the page CompMD renders into */
export interface LineRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A line of a document where CompMD drew it (the view's `lines()`, CompMD 0.5.0): the boxes its text shows in, one
 * per visual line it covers. What draws nothing (a blank line, frontmatter, a fence) has none. */
export interface DrawnLine {
  line: number;
  rects: readonly LineRect[];
}

/** A row of a Markdown document's gutter: the source lines it numbers (one, or several that share a visual line:
 * a soft line break shows as a space), where it is, and whether CompMD drew them or they fill a gap */
export interface GutterRow {
  from: number;
  to: number;
  top: number;
  height: number;
  drawn: boolean;
}

/**
 * Every line of a document, 1 to `total`, in its gutter (T-406). A line CompMD drew is numbered at its first box,
 * as tall as it; lines that share a visual line (the same top) share a row. A line that draws nothing (a blank line,
 * frontmatter, a fence, a component's lines after its first) goes in the gap around it, the lines of a gap spread
 * evenly over it: from the end of the line drawn before (its last box; a box much taller than a line, a diagram, ends
 * a line below its top, so the component's own lines spread over it) to the next one's top, the page's `top` before the
 * first and its `bottom` after the last. CompMD says where each line is; nothing here matches the text.
 */
export function gutterRows(
  drawn: readonly DrawnLine[],
  total: number,
  { top = 0, bottom }: { top?: number; bottom: number },
): GutterRow[] {
  const placed = new Map<number, readonly LineRect[]>();
  for (const d of drawn) {
    const rects = d.rects.filter((r) => r.height > 0);
    if (d.line >= 1 && d.line <= total && rects.length && !placed.has(d.line)) placed.set(d.line, rects);
  }
  const heights = [...placed.values()].map((r) => r[0].height).sort((a, b) => a - b);
  const typical = heights.length ? heights[Math.floor(heights.length / 2)] : 20;
  const rows: GutterRow[] = [];
  let gap: number[] = [];
  let gapStart = top;
  const fill = (end: number) => {
    if (!gap.length) return;
    const step = Math.max(0, end - gapStart) / gap.length;
    gap.forEach((line, i) => rows.push({ from: line, to: line, top: gapStart + i * step, height: step, drawn: false }));
    gap = [];
  };
  let last: GutterRow | null = null;
  for (let line = 1; line <= total; line++) {
    const rects = placed.get(line);
    if (!rects) {
      gap.push(line);
      continue;
    }
    const [first] = rects;
    fill(first.y);
    const box = rects.length === 1 && first.height > 3 * typical;
    const height = box ? typical : first.height;
    if (last && last.to === line - 1 && Math.abs(last.top - first.y) < 1) last.to = line;
    else {
      last = { from: line, to: line, top: first.y, height, drawn: true };
      rows.push(last);
    }
    gapStart = box ? first.y + typical : Math.max(...rects.map((r) => r.y + r.height));
  }
  fill(Math.max(gapStart, bottom));
  return rows.sort((a, b) => a.from - b.from);
}

function localTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
  } catch {
    return 'UTC';
  }
}

/** Where atm serve serves this build's CompMD: dist/assets/compmd/<version>/ (embedded in atm), at /assets/compmd/<version>/ (§2):
 * the path vite.compmd.config.ts builds it for, from which @compmd/vite gives its frames' and modules' URLs */
export function compmdAssetUrl(file: string, origin = location.origin): string {
  return new URL(`/assets/compmd/${compmdVersion}/${file}`, origin).href;
}

/** What the served module gives (dashboard/compmd/entry.ts) */
export interface CompmdModule {
  init(): Promise<unknown>;
  OpenDocument: new (request: ReturnType<typeof compmdRequest> | ReturnType<typeof documentRequest>) => {
    readonly expanded: unknown;
    readonly components: readonly unknown[];
    free(): void;
  };
  request: BaseRequest;
  sandbox: { base: string; modules: unknown };
  createSandbox(options: { base: string; modules: unknown; page: 'host' }): {
    open(
      container: HTMLElement,
      options: { components: readonly unknown[] },
    ): {
      render(tree: unknown): void;
      scheme(): void;
      /** Each line of the text that draws something, with its boxes, from the container's top left (CompMD 0.5.0) */
      lines(from?: number): AsyncGenerator<DrawnLine>;
      /** `layout`: what the page shows may have moved, so its lines may have (CompMD 0.5.0) */
      on(type: 'rendered' | 'error' | 'layout', listener: (event: unknown) => void): void;
      destroy(): void;
    };
  };
}

/**
 * Only the packs' components: built modules, which the sandbox loads from the asset folder and a strict frame runs
 * only if they match the hash @compmd/vite took. A component the text defines itself (a `jsx` or `svelte` fence) has
 * no module: it is left out, so it shows its fallback and its code never runs (R5).
 */
export function packComponents<T>(components: readonly T[]): T[] {
  return components.filter((component) => !!(component as { module?: unknown }).module);
}

/** A text processed: its expanded tree, safe to render (safeTree), and the packs' components it can name */
export function processText(compmd: CompmdModule, text: string, task?: Task, path?: string, links: ProjectLinks = {}) {
  const request = path ? documentRequest(compmd.request, path, text) : compmdRequest(compmd.request, text, task);
  const document = new compmd.OpenDocument(request);
  try {
    return { tree: safeTree(document.expanded, links), components: packComponents(document.components) };
  } finally {
    document.free();
  }
}

let loading: Promise<CompmdModule> | null = null;

/**
 * CompMD, loaded once per page, the first time task text renders, and kept (§2). A dynamic import() of the served
 * module, not a fetch: the browser loads it as a script, which the page's CSP allows from the asset folder only, and
 * the module's own glue fetches its WebAssembly beside it. A failure is kept too: the page shows task text as written
 * until it is reloaded (R2).
 */
export function loadCompmd(importer: (url: string) => Promise<unknown> = (url) => import(/* @vite-ignore */ url)) {
  loading ??= importer(compmdAssetUrl('compmd.js')).then(async (module) => {
    const compmd = module as CompmdModule;
    await compmd.init();
    return compmd;
  });
  return loading;
}

/** Forget the loaded module (tests) */
export function resetCompmd() {
  loading = null;
}

/** How a project's text reaches the project's files (safeTree) */
export interface ProjectLinks {
  localImage?: (url: string) => string | null;
  docLink?: (url: string) => string | null;
}

/** A node of CompMD's expanded tree, as far as safeTree reads it (mdast's fields) */
interface TreeNode {
  type: string;
  url?: string;
  alt?: string | null;
  identifier?: string;
  value?: unknown;
  children?: TreeNode[];
}

const DATA_IMAGE = /^\s*data:image\//i;
const RAW_IMAGE = /<\s*img\b/i;

/**
 * The expanded tree with what task text mustn't do taken out before it renders (§3), so the browser never even asks:
 * an image that isn't a `data:` image (Markdown's, a reference's, or raw `<img>`) shows its text, since an image from
 * anywhere else would be a request outside the dashboard (the page's CSP refuses it too, noisily); a link atm
 * wouldn't follow becomes its text. secureLinks checks the rendered links again. For a project's text (task text, a
 * document in the viewer; docs/compmd-viewer-design.md): `localImage` gives a relative image's address on this server
 * (the files route), so it loads from there (V5); `docLink` gives a relative link's viewer link (`#doc=…`), which opens the
 * document viewer (V1a–V1c), so the browser never follows a relative URL itself. Null from either: as without it.
 */
export function safeTree<T>(tree: T, { localImage, docLink }: ProjectLinks = {}): T {
  const root = tree as TreeNode;
  const definitions = new Map<string, string>();
  const collect = (node: TreeNode) => {
    if (node.type === 'definition' && node.identifier) definitions.set(node.identifier, node.url ?? '');
    node.children?.forEach(collect);
  };
  collect(root);
  const asText = (value: unknown): TreeNode[] => (value ? [{ type: 'text', value: String(value) }] : []);
  const visit = (node: TreeNode): TreeNode[] => {
    switch (node.type) {
      case 'image': {
        if (DATA_IMAGE.test(node.url ?? '')) return [node];
        const local = localImage?.(node.url ?? '');
        return local ? [{ ...node, url: local }] : asText(node.alt);
      }
      case 'imageReference': {
        const url = definitions.get(node.identifier ?? '') ?? '';
        if (DATA_IMAGE.test(url)) return [node];
        const local = localImage?.(url);
        return local ? [{ type: 'image', url: local, alt: node.alt }] : asText(node.alt);
      }
      case 'link':
        if (!safeHref(node.url ?? '')) {
          const viewer = docLink?.(node.url ?? '');
          if (viewer) return [{ ...node, url: viewer, children: (node.children ?? []).flatMap(visit) }];
          return (node.children ?? []).flatMap(visit);
        }
        break;
      case 'html':
        if (RAW_IMAGE.test(String(node.value))) return asText(node.value);
        break;
    }
    return node.children ? [{ ...node, children: node.children.flatMap(visit) }] : [node];
  };
  return { ...root, children: (root.children ?? []).flatMap(visit) } as T;
}

/** The address of a web link to this dashboard itself (one of its pages on this server, T-429, or an old `#…` link to
 * it: a project, a task or a document linked by its URL), which the dashboard follows in place; null for any other
 * link */
export function dashboardAddress(href: string, page: string = location.href): string | null {
  let url: URL;
  let here: URL;
  try {
    url = new URL(href.trim());
    here = new URL(page);
  } catch {
    return null;
  }
  if (!/^https?:$/.test(url.protocol) || url.origin !== here.origin) return null;
  return isPagePath(url.pathname) ? addressOf(url) : null;
}

/**
 * Makes the links CompMD rendered in `root` safe and opens them as the dashboard always has (§3): only `https:`,
 * `http:`, `mailto:` and `#` stay links (CompMD also allows relative ones, which T-336 will define); any other becomes
 * its text. External links open in a new tab without access to this page; `#` links stay, for the panel to scroll.
 */
export function secureLinks(root: ParentNode) {
  for (const a of root.querySelectorAll('a[href]')) {
    const href = a.getAttribute('href') ?? '';
    if (!safeHref(href)) {
      a.replaceWith(...a.childNodes);
      continue;
    }
    if (href.trim().startsWith('#') || dashboardAddress(href) !== null) continue;
    a.setAttribute('target', '_blank');
    a.setAttribute('rel', 'noopener noreferrer');
  }
}
