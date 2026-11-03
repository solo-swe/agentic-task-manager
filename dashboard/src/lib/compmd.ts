// CompMD renders all task text in the details panel (docs/compmd-rendering-design.md). Task text is untrusted: anyone
// who can change tasks.json wrote it. This module builds what CompMD is given (the frontmatter the dashboard puts in
// front of every text, and the request, §3 and §3b), loads CompMD from atm serve's asset folder (§2), and makes the
// rendered links safe (§3).
// Types only: the processor itself is in the module served from the asset folder, never in the dashboard's bundle
import type { Request } from '@compmd/processor';
import { version as compmdVersion } from 'virtual:atm-compmd';
import { safeHref } from './markdown';
import type { Task } from './types';

/** The record `md.data.task` holds: the task's fields and what atm derives for it, a derived value replacing the
 * stored one of the same name (the effective priority). Only this task: never another task's or project's data. */
export function taskData(task: Task): Record<string, unknown> {
  const { derived, ...stored } = task;
  return { ...stored, ...(derived ?? {}) };
}

/** The frontmatter in front of every task text: the same for each, never stored in tasks.json (§3b). Written as JSON,
 * which YAML reads as it is, so no value needs quoting rules. `host:task` has no `schema`: in CompMD 0.3.0 a schema
 * makes data a list of records, and the task is one record; its values are typed when evaluated instead. Packs aren't
 * here: compmd.config.json makes the Mermaid pack ambient (§3c). */
export function frontmatter(): string {
  const md = { version: 1, data: [{ from: 'host:task', as: 'task' }] };
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
 * frontmatter, with no files to import and no remote data, and the task's own data as the one `host:` source */
export function compmdRequest(
  base: BaseRequest,
  text: string,
  task: Task,
  now = new Date(),
  timeZone = localTimeZone(),
) {
  return {
    ...base,
    path: DOCUMENT,
    files: { [DOCUMENT]: frontmatter() + text },
    now: now.toISOString(),
    tz: timeZone,
    allowedOrigins: [] as string[],
    host: { task: taskData(task) },
  };
}

function localTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
  } catch {
    return 'UTC';
  }
}

/** Where atm serve serves this build's CompMD: plugin/assets/compmd/<version>/, at /assets/compmd/<version>/ (§2):
 * the path vite.compmd.config.ts builds it for, from which @compmd/vite gives its frames' and modules' URLs */
export function compmdAssetUrl(file: string, origin = location.origin): string {
  return new URL(`/assets/compmd/${compmdVersion}/${file}`, origin).href;
}

/** What the served module gives (dashboard/compmd/entry.ts) */
export interface CompmdModule {
  init(): Promise<unknown>;
  OpenDocument: new (request: ReturnType<typeof compmdRequest>) => {
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
      on(type: 'rendered' | 'error', listener: (event: unknown) => void): void;
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
export function processText(compmd: CompmdModule, text: string, task: Task) {
  const document = new compmd.OpenDocument(compmdRequest(compmd.request, text, task));
  try {
    return { tree: safeTree(document.expanded), components: packComponents(document.components) };
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
 * wouldn't follow becomes its text. secureLinks checks the rendered links again.
 */
export function safeTree<T>(tree: T): T {
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
      case 'image':
        return DATA_IMAGE.test(node.url ?? '') ? [node] : asText(node.alt);
      case 'imageReference':
        return DATA_IMAGE.test(definitions.get(node.identifier ?? '') ?? '') ? [node] : asText(node.alt);
      case 'link':
        if (!safeHref(node.url ?? '')) return (node.children ?? []).flatMap(visit);
        break;
      case 'html':
        if (RAW_IMAGE.test(String(node.value))) return asText(node.value);
        break;
    }
    return node.children ? [{ ...node, children: node.children.flatMap(visit) }] : [node];
  };
  return { ...root, children: (root.children ?? []).flatMap(visit) } as T;
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
    if (href.trim().startsWith('#')) continue;
    a.setAttribute('target', '_blank');
    a.setAttribute('rel', 'noopener noreferrer');
  }
}
