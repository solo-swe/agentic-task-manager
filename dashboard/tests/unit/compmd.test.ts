// lib/compmd.ts in the page: where CompMD is loaded from, loading it once, the frontmatter and the task's data, and
// the links it renders (docs/compmd-rendering-design.md §2, §3, §3b). The processor itself: compmd-processor.test.ts.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { version } from 'virtual:atm-compmd';
import { request as base, sandbox } from 'virtual:compmd';
import {
  compmdAssetUrl,
  compmdRequest,
  dashboardAddress,
  frontmatter,
  FRONTMATTER_LINES,
  gutterRows,
  loadCompmd,
  resetCompmd,
  safeHref,
  secureLinks,
  slug,
  taskData,
  type GutterRow,
} from '../../src/lib/compmd';
import type { Task } from '../../src/lib/types';

const task: Task = {
  id: 'T-7',
  name: 'Sign-in page',
  type: 'feature',
  status: 'todo',
  description: 'the text',
  phase: 0,
  priority: 'P1',
  derived: { kind: 'todo', ready: true, waiting_for: [], priority: 'P1', archived: false },
};

afterEach(() => resetCompmd());

describe('the asset folder', () => {
  it('is the installed CompMD version’s, at /assets/compmd/<version>/ on the page’s server', () => {
    expect(version).toMatch(/^\d+\.\d+\.\d+/);
    expect(compmdAssetUrl('compmd.js', 'http://localhost:8765')).toBe(
      `http://localhost:8765/assets/compmd/${version}/compmd.js`,
    );
  });

  it('the Mermaid pack’s module is pinned by the hash of the file it ships (@compmd/vite, T-285)', () => {
    const found = (sandbox.modules as (place: object) => { url: string; integrity: string })({
      pack: '@comp-markdown/mermaid',
      version: '0.1.1',
      path: 'dist/mermaid.js',
    });
    expect(found.url).toMatch(/\/packs\/@comp-markdown\/mermaid@0\.1\.1\/dist\/mermaid\.js$/);
    const shipped = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), '../../node_modules/@comp-markdown/mermaid/dist/mermaid.js'),
    );
    const hashes = found.integrity.split(/\s+/);
    expect(hashes).toContain(`sha384-${createHash('sha384').update(shipped).digest('base64')}`);
  });
});

describe('loading CompMD', () => {
  it('imports the served module once and initialises it, for every text', async () => {
    const init = vi.fn(async () => undefined);
    const importer = vi.fn<(url: string) => Promise<unknown>>(async () => ({
      init,
      OpenDocument: vi.fn(),
      createSandbox: vi.fn(),
    }));
    const [a, b] = await Promise.all([loadCompmd(importer), loadCompmd(importer)]);
    expect(a).toBe(b);
    expect(importer).toHaveBeenCalledTimes(1);
    expect(importer.mock.calls[0][0]).toMatch(new RegExp(`/assets/compmd/${version}/compmd\\.js$`));
    expect(init).toHaveBeenCalledTimes(1);
  });

  it('a failure is kept: the page shows texts as written until it is reloaded', async () => {
    const importer = vi.fn<(url: string) => Promise<unknown>>(async () => {
      throw new Error('404');
    });
    await expect(loadCompmd(importer)).rejects.toThrow('404');
    await expect(loadCompmd(importer)).rejects.toThrow('404');
    expect(importer).toHaveBeenCalledTimes(1);
  });
});

describe('what CompMD is given', () => {
  it('the task’s data is its fields and atm’s derived values, the effective priority winning', () => {
    const data = taskData({ ...task, priority: undefined });
    expect(data).toMatchObject({ id: 'T-7', name: 'Sign-in page', kind: 'todo', ready: true, priority: 'P1' });
    expect(data).not.toHaveProperty('derived');
  });

  it('the frontmatter declares the task as the one host source, and nothing else', () => {
    const text = frontmatter();
    expect(text.startsWith('---\n') && text.endsWith('\n---\n')).toBe(true);
    expect(JSON.parse(text.slice(4, -5))).toEqual({ md: { version: 1, data: [{ from: 'host:task', as: 'task' }] } });
    expect(FRONTMATTER_LINES).toBe(3);
  });

  it('the request: @compmd/vite’s (packs, ambient, policy), the text behind the frontmatter, the task, no origins', () => {
    const request = compmdRequest(base, '# Hello', task, new Date('2026-11-03T10:00:00Z'), 'Europe/Berlin');
    expect(request).toEqual({
      ...base,
      path: 'task.comp.md',
      files: { 'task.comp.md': frontmatter() + '# Hello' },
      now: '2026-11-03T10:00:00.000Z',
      tz: 'Europe/Berlin',
      allowedOrigins: [],
      host: { task: taskData(task) },
    });
    // The dashboard's parts win over anything a configuration might hold
    const overridden = compmdRequest({ ...base, allowedOrigins: ['https://x'], host: { y: 1 } } as never, '', task);
    expect(overridden.allowedOrigins).toEqual([]);
    expect(Object.keys(overridden.host)).toEqual(['task']);
  });

  it('a text that is no task’s (Help, What’s new) gets no task: no host source, no host data', () => {
    const text = frontmatter(false);
    expect(JSON.parse(text.slice(4, -5))).toEqual({ md: { version: 1 } });
    expect(text.split('\n').length - 1).toBe(FRONTMATTER_LINES);
    const request = compmdRequest(base, '# Help', undefined);
    expect(request.files['task.comp.md']).toBe(text + '# Help');
    expect(request.host).toEqual({});
  });
});

describe('a Markdown document’s gutter: every line (T-406)', () => {
  const box = (y: number, height = 20, x = 0, width = 300) => ({ x, y, width, height });
  const brief = (rows: GutterRow[]) =>
    rows.map((r) => [r.from === r.to ? r.from : `${r.from}-${r.to}`, r.top, r.height, r.drawn]);

  it('a drawn line at its first box, as tall; the lines that draw nothing spread over the gaps, before and after', () => {
    // 1–3 frontmatter, 4 a heading, 5 blank, 6–7 a paragraph over two lines (7 wraps), 8 blank, 9 text, 10 blank
    const rows = gutterRows(
      [
        { line: 4, rects: [box(30, 30)] },
        { line: 6, rects: [box(80)] },
        { line: 7, rects: [box(100), box(120)] },
        { line: 9, rects: [box(160)] },
      ],
      10,
      { top: 0, bottom: 200 },
    );
    expect(brief(rows)).toEqual([
      [1, 0, 10, false],
      [2, 10, 10, false],
      [3, 20, 10, false],
      [4, 30, 30, true],
      [5, 60, 20, false],
      [6, 80, 20, true],
      [7, 100, 20, true],
      // after 7's last box (its wrapped second line), up to 9
      [8, 140, 20, false],
      [9, 160, 20, true],
      [10, 180, 20, false],
    ]);
  });

  it('lines that share a visual line share a row; a later line on its own box', () => {
    // 2 starts on 1's visual line (a soft break is a space); 3 starts where 2's second box is
    const rows = gutterRows(
      [
        { line: 1, rects: [box(0, 20, 0, 100)] },
        { line: 2, rects: [box(0, 20, 110, 190), box(20, 20, 0, 50)] },
        { line: 3, rects: [box(20, 20, 60, 100)] },
        { line: 4, rects: [box(40)] },
      ],
      4,
      { bottom: 60 },
    );
    expect(brief(rows)).toEqual([
      ['1-2', 0, 20, true],
      [3, 20, 20, true],
      [4, 40, 20, true],
    ]);
  });

  it('a box much taller than a line (a diagram) is numbered at its top, its own lines spread over the rest', () => {
    // 1 text, 2 blank, 3–7 a Mermaid block: its fence line draws the diagram, 4–7 nothing of their own, 8 text
    const rows = gutterRows(
      [
        { line: 1, rects: [box(0)] },
        { line: 3, rects: [box(40, 200)] },
        { line: 8, rects: [box(260)] },
      ],
      8,
      { bottom: 300 },
    );
    expect(brief(rows).slice(1, 4)).toEqual([
      [2, 20, 20, false],
      [3, 40, 20, true],
      // 4–7 over the diagram below its first line's height, to 8: 200 px for four lines
      [4, 60, 50, false],
    ]);
    expect(rows.at(-1)).toMatchObject({ from: 8, top: 260, drawn: true });
  });

  it('a line past the end, a repeat or an empty box is ignored; nothing drawn: every line spread over the page', () => {
    const rows = gutterRows(
      [
        { line: 2, rects: [box(10, 0)] },
        { line: 9, rects: [box(10)] },
      ],
      2,
      { bottom: 40 },
    );
    expect(brief(rows)).toEqual([
      [1, 0, 20, false],
      [2, 20, 20, false],
    ]);
    expect(gutterRows([], 0, { bottom: 10 })).toEqual([]);
  });
});

describe('links', () => {
  it('only web and mail addresses and anchors in the page are followed', () => {
    expect(['https://a', 'http://a', 'mailto:a@b', '#x', ' #x'].map(safeHref)).toEqual([true, true, true, true, true]);
    expect(['data:text/html,x', 'javascript:alert(1)', 'file:///etc', 'docs/a.md'].map(safeHref)).toEqual([
      false,
      false,
      false,
      false,
    ]);
  });

  it('a `#` link names a heading by its slug', () => {
    expect(slug('Statuses and kinds!')).toBe('statuses-and-kinds');
    expect(slug('  Running agents in parallel ')).toBe('running-agents-in-parallel');
  });
});

describe('the links CompMD renders', () => {
  function rendered(html: string) {
    const root = document.createElement('div');
    root.innerHTML = html;
    secureLinks(root);
    return root;
  }

  it('web and mail links open in a new tab, without access to the page', () => {
    const root = rendered('<a href="https://example.com">site</a> <a href="mailto:a@b.c">mail</a>');
    for (const a of root.querySelectorAll('a')) {
      expect(a.getAttribute('target')).toBe('_blank');
      expect(a.getAttribute('rel')).toBe('noopener noreferrer');
    }
  });

  it('a link to the dashboard itself opens in place: its address; any other one opens in a new tab', () => {
    const page = 'http://localhost:8765/projects/p2/list';
    expect(dashboardAddress('http://localhost:8765/projects/p1?doc=docs/x.md:L3', page)).toBe(
      '/projects/p1?doc=docs/x.md:L3',
    );
    expect(dashboardAddress('http://localhost:8765/board', page)).toBe('/board');
    // An old link (T-429): its new address
    expect(dashboardAddress('http://localhost:8765/#project=p1&doc=docs/x.md:L3', page)).toBe(
      '/projects/p1?doc=docs/x.md:L3',
    );
    expect(dashboardAddress('http://localhost:9999/projects/p1', page)).toBeNull(); // another server
    expect(dashboardAddress('http://localhost:8765/other#doc=x.md', page)).toBeNull(); // not a page of it
    expect(dashboardAddress('http://localhost:8765/api/projects', page)).toBeNull();
    expect(dashboardAddress('https://example.com/#doc=x.md', page)).toBeNull();
    expect(dashboardAddress('mailto:a@b.c', page)).toBeNull();
    expect(dashboardAddress('not a url', page)).toBeNull();
    const here = `${location.origin}${location.pathname}#doc=docs/x.md`;
    const a = rendered(`<a href="${here}">the design</a>`).querySelector('a');
    expect(a?.getAttribute('target')).toBeNull();
  });

  it('a # link stays, for the panel to scroll', () => {
    const a = rendered('<a href="#setup">setup</a>').querySelector('a');
    expect(a?.getAttribute('href')).toBe('#setup');
    expect(a?.hasAttribute('target')).toBe(false);
  });

  it('any other link becomes its text: javascript:, data:, relative paths', () => {
    const root = rendered(
      '<a href="javascript:alert(1)">one</a> <a href="data:text/html,x">two</a> <a href="../api/projects">three</a>',
    );
    expect(root.querySelectorAll('a')).toHaveLength(0);
    expect(root.textContent).toBe('one two three');
  });
});
