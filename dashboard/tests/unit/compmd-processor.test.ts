// @vitest-environment node
// The request lib/compmd.ts builds, run through the real CompMD processor (the WebAssembly the dashboard ships):
// what task text may and may not do (docs/compmd-rendering-design.md §3, §3b).
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, OpenDocument, process } from '@compmd/processor';
import { request as base } from 'virtual:compmd';
import { compmdRequest, documentRequest, FRONTMATTER_LINES, packComponents, safeTree } from '../../src/lib/compmd';
import type { Task } from '../../src/lib/types';

const WASM = new URL('../../node_modules/@compmd/processor/pkg/compmd_wasm_bg.wasm', import.meta.url);

beforeAll(() => {
  initSync({ module: readFileSync(WASM) });
});

const task: Task = {
  id: 'T-7',
  name: 'Sign-in page',
  type: 'feature',
  status: 'todo',
  description: '',
  phase: 0,
  dependencies: ['T-5', 'T-6'],
  labels: ['ux'],
  derived: { kind: 'todo', ready: false, waiting_for: ['T-6'], priority: 'P2', archived: false },
};

const NOW = new Date('2026-11-03T10:00:00Z');
// The request as the served runtime gives it: @compmd/vite's, from compmd.config.json
const requestFor = (text: string, t: Task = task) => compmdRequest(base, text, t, NOW, 'UTC');
const run = (text: string, t: Task = task) => process(requestFor(text, t));
/** The text of every node in the expanded tree, in order */
function textOf(node: unknown): string {
  if (!node || typeof node !== 'object') return '';
  const n = node as { value?: unknown; children?: unknown[] };
  return (typeof n.value === 'string' ? n.value : '') + (n.children ?? []).map(textOf).join('');
}
const codes = (r: ReturnType<typeof run>) => r.diagnostics.map((d) => d.code);

/** The first source line of each top-level block of a tree, where it has an origin */
const firstLines = (tree: unknown) =>
  ((tree as { children?: { origin?: { start?: { line?: number } } }[] }).children ?? []).map(
    (n) => n.origin?.start?.line,
  );

describe('a document’s source lines (T-406: CompMD places each line from its origins)', () => {
  it('a document is processed with origins, a frontmatter counted, and safeTree keeps them', () => {
    const text =
      '---\ntitle: X\n---\n# Title\n\nA paragraph\nover two lines.\n\n- one\n- two\n\n```ts\nconst a = 1;\n```\n';
    const tree = new OpenDocument(documentRequest(base, 'docs/x.md', text, NOW, 'UTC')).expanded;
    expect(firstLines(tree)).toEqual([4, 6, 9, 12]);
    expect(firstLines(safeTree(tree))).toEqual(firstLines(tree));
  });

  it('task text is processed without them', () => {
    expect(firstLines(run('# Title\n\nText.').expanded)).toEqual([undefined, undefined]);
  });
});

describe('task text through the real processor', () => {
  it('plain Markdown and GFM render with no diagnostics', () => {
    const r = run('# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] not yet\n\n~~old~~\n');
    expect(r.diagnostics).toEqual([]);
    expect(textOf(r.expanded)).toContain('Title');
  });

  it('md.data.task is the task, with atm’s derived values', () => {
    const r = run(
      'Task {{md.data.task.id}} is {{md.data.task.kind}} at {{md.data.task.priority}}, waiting for ' +
        ':value[its dependencies]{of=(md.list(md.data.task.waiting_for))}.',
    );
    expect(r.diagnostics).toEqual([]);
    expect(textOf(r.expanded)).toBe('Task T-7 is todo at P2, waiting for T-6.');
  });

  it('each runs over the task’s lists', () => {
    const r = run('::: each {in=(md.data.task.dependencies) as=d join=list}\n- {{d}}\n::: /each\n');
    expect(r.diagnostics).toEqual([]);
    expect(textOf(r.expanded)).toBe('T-5T-6');
  });

  it('a field the task doesn’t have is a diagnostic, not a crash', () => {
    const bare: Task = { id: 'T-1', name: 'x', type: 'feature', status: 'todo', description: '', phase: 0 };
    const r = run('{{md.data.task.name}} :value[none]{of=(md.list(md.data.task.waiting_for))}', bare);
    expect(textOf(r.expanded)).toContain('x');
    expect(r.diagnostics.every((d) => d.stage === 'evaluate')).toBe(true);
  });

  it('a frontmatter written in the task text is only text: it can’t import data, packs or files', () => {
    const r = run(
      '---\nmd:\n  data:\n    - from: https://example.com/x.json\n      as: x\n  import:\n    - from: "@kit/ui@1"\n      names: [card]\n---\n',
    );
    expect(codes(r)).not.toContain('F028');
    expect(textOf(r.expanded)).toContain('https://example.com/x.json');
  });

  it('only the task is host data, no origin is allowed, and only the listed packs, strict, ambient', () => {
    const request = requestFor('');
    expect(Object.keys(request.host)).toEqual(['task']);
    expect(request.allowedOrigins).toEqual([]);
    expect(request.policy).toMatchObject({
      imports: 'listed',
      packs: { '@comp-markdown/mermaid': { level: 'strict' } },
    });
    expect(Object.keys((request.policy as { packs: object }).packs)).toEqual(['@comp-markdown/mermaid']);
    expect(request.ambient).toEqual(['@comp-markdown/mermaid']);
  });

  it('a mermaid block is the ambient Mermaid pack’s component: a strict built module (T-285)', () => {
    const document = new OpenDocument(requestFor('```mermaid {alt="A then B"}\ngraph LR; A --> B\n```\n'));
    try {
      expect(document.diagnostics.filter((d) => d.severity === 'error')).toEqual([]);
      expect(packComponents(document.components)).toMatchObject([
        {
          name: 'mermaid',
          trust: { level: 'strict' },
          module: { pack: '@comp-markdown/mermaid', version: '0.1.1', path: 'dist/mermaid.js' },
        },
      ]);
    } finally {
      document.free();
    }
  });

  it('a pack the configuration doesn’t list can’t be imported, even from the text', () => {
    const r = run('---\nmd:\n  import:\n    - from: "@kit/ui@1"\n      names: [card]\n---\n:: card[a card]\n');
    expect(textOf(r.expanded)).toContain('a card');
  });

  it('a component the text defines itself is never given to the sandbox: its code never runs (R5)', () => {
    const text =
      '```jsx component=evil\nexport default function Evil() { return <b>x</b>; }\n```\n\n:: evil[the fallback]\n';
    const document = new OpenDocument(requestFor(text));
    try {
      expect(document.components.map((c) => c.name)).toContain('evil');
      expect(packComponents(document.components).map((c) => c.name)).toEqual(['mermaid']);
    } finally {
      document.free();
    }
  });

  it('a component that doesn’t exist shows its fallback, with a diagnostic', () => {
    const r = run(':: chart[Sales went up] {rows=(md.data.task.labels)}\n');
    expect(textOf(r.expanded)).toContain('Sales went up');
    expect(r.diagnostics.length).toBeGreaterThan(0);
  });

  it('safeTree: images from anywhere but data: show their text; links atm wouldn’t follow become text', () => {
    const tree = safeTree(
      run(
        '![a chart](https://example.com/c.png) ![ref][r] <img src="https://example.com/x.png"> ' +
          '![dot](data:image/png;base64,iVBORw0KGgo=) [ok](https://example.com) [bad](javascript:alert(1)) ' +
          '[rel](../api/projects)\n\n[r]: https://example.com/r.png\n',
      ).expanded,
    );
    const types: string[] = [];
    const urls: string[] = [];
    const walk = (node: { type: string; url?: string; children?: unknown[] }) => {
      types.push(node.type);
      if (node.url && node.type !== 'definition') urls.push(node.url);
      (node.children as (typeof node)[] | undefined)?.forEach(walk);
    };
    walk(tree as never);
    expect(types).not.toContain('imageReference');
    expect(urls).toEqual(['data:image/png;base64,iVBORw0KGgo=', 'https://example.com']);
    const text = textOf(tree);
    expect(text).toContain('a chart');
    expect(text).toContain('ref');
    expect(text).toContain('<img src="https://example.com/x.png">');
    expect(text).toContain('bad');
    expect(text).toContain('rel');
  });

  it('safeTree for a project’s text: a relative link opens the viewer, a relative image loads from this server', () => {
    const tree = safeTree(
      run(
        '[design](docs/x.md#L3) [out](../../etc) [site](https://example.com) ![flow](img/f.png) ![far](https://e.com/a.png)\n',
      ).expanded,
      {
        docLink: (url) => (url.startsWith('docs/') ? `#doc=${url.split('#')[0]}` : null),
        localImage: (url) => (url.startsWith('img/') ? `api/files?path=${url}` : null),
      },
    );
    const urls: string[] = [];
    const walk = (node: { type: string; url?: string; children?: unknown[] }) => {
      if (node.url) urls.push(`${node.type} ${node.url}`);
      (node.children as (typeof node)[] | undefined)?.forEach(walk);
    };
    walk(tree as never);
    expect(urls).toEqual(['link #doc=docs/x.md', 'link https://example.com', 'image api/files?path=img/f.png']);
    expect(textOf(tree)).toContain('out');
    expect(textOf(tree)).toContain('far');
  });

  it('diagnostics give lines in the document: the text’s line plus the frontmatter’s', () => {
    const r = run('fine\n\n{{md.nothing.here}}\n');
    const unknown = r.diagnostics.find((d) => d.code === 'F014');
    expect(unknown?.range.start.line).toBe(FRONTMATTER_LINES + 3);
  });
});
