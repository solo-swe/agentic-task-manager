// lib/compmd.ts in the page: where CompMD is loaded from, loading it once, the frontmatter and the task's data, and
// the links it renders (docs/compmd-rendering-design.md §2, §3, §3b). The processor itself: compmd-processor.test.ts.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { version } from 'virtual:atm-compmd';
import {
  compmdAssetUrl,
  compmdRequest,
  frontmatter,
  FRONTMATTER_LINES,
  loadCompmd,
  resetCompmd,
  secureLinks,
  taskData,
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
  it('is the installed CompMD version’s, relative to the page', () => {
    expect(version).toMatch(/^\d+\.\d+\.\d+/);
    expect(compmdAssetUrl('compmd.js', 'http://localhost:8765/')).toBe(
      `http://localhost:8765/assets/compmd/${version}/compmd.js`,
    );
    expect(compmdAssetUrl('compmd.js', 'http://localhost:4173/svelte/tasks-dashboard.html')).toBe(
      `http://localhost:4173/svelte/assets/compmd/${version}/compmd.js`,
    );
  });
});

describe('loading CompMD', () => {
  it('imports the served module once and initialises it, for every text', async () => {
    const init = vi.fn(async () => undefined);
    const importer = vi.fn<(url: string) => Promise<unknown>>(async () => ({
      init,
      process: vi.fn(),
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

  it('the request: the text behind the frontmatter, the task as host data, no origins, no packs', () => {
    const request = compmdRequest('# Hello', task, new Date('2026-11-03T10:00:00Z'), 'Europe/Berlin');
    expect(request).toEqual({
      path: 'task.comp.md',
      files: { 'task.comp.md': frontmatter() + '# Hello' },
      now: '2026-11-03T10:00:00.000Z',
      tz: 'Europe/Berlin',
      allowedOrigins: [],
      host: { task: taskData(task) },
      policy: { imports: 'listed', packs: {} },
    });
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
