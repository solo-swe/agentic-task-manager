import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  addressOf,
  addressParams,
  formatAddress,
  isPagePath,
  legacyParams,
  OVERVIEW_TABS,
  withWorkspace,
  workspaceOf,
} from '../../src/lib/address';
import { VIEWS } from '../../src/lib/tabs';
import { OVERVIEW_PATHS, PROJECT_TABS } from '../../src/params';

const params = (address: string) => [...addressParams(address)];
const at = (href: string) => {
  const url = new URL(href, 'http://localhost:7000');
  return { pathname: url.pathname, search: url.search, hash: url.hash };
};

describe('the address (T-429)', () => {
  it('reads the page from the path and the rest from the query', () => {
    expect(params('/')).toEqual([]);
    expect(params('/projects')).toEqual([
      ['overview', ''],
      ['view', 'projects'],
    ]);
    expect(params('/board')).toEqual([
      ['overview', ''],
      ['view', 'board'],
    ]);
    expect(params('/projects/a%20b')).toEqual([['project', 'a b']]);
    expect(params('/projects/abc/list?sprint=a,b&doc=docs/x.md:L3&about')).toEqual([
      ['project', 'abc'],
      ['view', 'list'],
      ['sprint', 'a,b'],
      ['doc', 'docs/x.md:L3'],
      ['about', ''],
    ]);
    // The page is the path's: the query can't name another one
    expect(params('/projects/abc?project=def&overview&view=list')).toEqual([['project', 'abc']]);
    // The landing page may name a tab (an old link's), another page only in its path
    expect(params('/?view=list')).toEqual([['view', 'list']]);
    // A path the dashboard has no page for is the landing page
    expect(params('/tasks-dashboard.html?about')).toEqual([['about', '']]);
    expect(params('/projects/abc/list/more')).toEqual([]);
    expect(params('/projects/%E0%A4%A')).toEqual([]);
  });

  it('writes the page as a path, the rest as the query; flags without "=", paths and lists readable', () => {
    const write = (entries: [string, string][]) => formatAddress(new URLSearchParams(entries));
    expect(write([])).toBe('/');
    expect(write([['overview', '']])).toBe('/projects');
    expect(
      write([
        ['overview', ''],
        ['view', 'agents'],
      ]),
    ).toBe('/agents');
    expect(
      write([
        ['project', 'a/b c'],
        ['view', 'graph'],
        ['help', ''],
      ]),
    ).toBe('/projects/a%2Fb%20c/graph?help');
    expect(
      write([
        ['view', 'list'],
        ['sprint', 'Ada Lovelace,backlog'],
        ['doc', 'docs/x.md:L3#a&b'],
        ['project', 'p1'],
      ]),
    ).toBe('/projects/p1/list?sprint=Ada%20Lovelace,backlog&doc=docs/x.md:L3%23a%26b');
    expect(write([['view', 'list']])).toBe('/?view=list');
    // Written and read back, the same
    const address = '/projects/p1/list?sprint=active&task=T-1&doc=docs/a%20b.md#:~:text=x';
    expect(formatAddress(addressParams(formatAddress(addressParams(address))))).toBe(
      formatAddress(addressParams(address)),
    );
  });

  it('an old #… link is read as its parameters, and becomes its new address', () => {
    expect(legacyParams('#project=abc&view=list&about')).not.toBeNull();
    expect(legacyParams('#overview')).not.toBeNull();
    expect(legacyParams('#routes')).toBeNull();
    expect(legacyParams('')).toBeNull();
    expect(addressOf(at('/#project=abc&view=list&doc=docs/x.md:L3&about'))).toBe(
      '/projects/abc/list?doc=docs/x.md:L3&about',
    );
    expect(addressOf(at('/#overview&view=board'))).toBe('/board');
    expect(addressOf(at('/#overview'))).toBe('/projects');
    expect(addressOf(at('/#view=list&sprint=all'))).toBe('/?view=list&sprint=all');
    expect(addressOf(at('/#help'))).toBe('/?help');
    // A heading's anchor isn't one: the path and query are the address
    expect(addressOf(at('/projects/abc?about#routes'))).toBe('/projects/abc?about');
    expect(addressOf(at('/projects/abc/'))).toBe('/projects/abc');
  });

  it("knows the dashboard's page paths, which src/routes/ has a page for", () => {
    for (const path of ['/', '/projects', '/projects/abc', '/projects/abc/list', '/board', '/progress', '/agents'])
      expect(isPagePath(path), path).toBe(true);
    for (const path of ['/api/projects', '/p/abc/claims.json', '/assets/x.js', '/projects/a/b/c', '/boards'])
      expect(isPagePath(path), path).toBe(false);
    // The route matchers' lists are the dashboard's own
    expect(PROJECT_TABS).toEqual([...VIEWS]);
    expect(OVERVIEW_PATHS).toEqual(OVERVIEW_TABS.filter((tab) => tab !== 'projects'));
    const routes = join(dirname(fileURLToPath(import.meta.url)), '../../src/routes');
    for (const page of [
      '+page.svelte',
      'projects/+page.svelte',
      'projects/[id]/+page.svelte',
      'projects/[id]/[tab=view]/+page.svelte',
      '[tab=overview]/+page.svelte',
    ])
      expect(existsSync(join(routes, page)), page).toBe(true);
  });
});

describe('the workspace in the address (ADR-003 §5)', () => {
  it('reads /w/<w> as the workspace, `default` as none, and keeps it through every page', () => {
    expect(params('/w/work')).toEqual([['workspace', 'work']]);
    expect(params('/w/work/projects/abc/list?task=T-1')).toEqual([
      ['workspace', 'work'],
      ['project', 'abc'],
      ['view', 'list'],
      ['task', 'T-1'],
    ]);
    expect(params('/w/default/board')).toEqual([
      ['overview', ''],
      ['view', 'board'],
    ]);
    // The workspace is the path's: a query can't set it
    expect(params('/projects?workspace=x')).toEqual([
      ['overview', ''],
      ['view', 'projects'],
    ]);
    for (const address of ['/w/work', '/w/work/projects', '/w/work/board', '/w/work/projects/abc/list?about'])
      expect(formatAddress(addressParams(address)), address).toBe(address);
    expect(formatAddress(addressParams('/w/default/projects/abc'))).toBe('/projects/abc');
    expect(addressOf(at('/w/a%20b/projects'))).toBe('/w/a%20b/projects');
  });

  it('knows which workspace an address is in, and moves to another without the first one’s project', () => {
    expect(workspaceOf('/')).toBe('default');
    expect(workspaceOf('/w/work/board')).toBe('work');
    expect(withWorkspace('/projects/abc/list?task=T-1&about', 'work')).toBe('/w/work?about');
    expect(withWorkspace('/w/work/board?sprint=a', 'default')).toBe('/board');
    expect(withWorkspace('/w/work/projects/abc', 'work')).toBe('/w/work/projects/abc');
  });

  it("has a page under /w/<w> for each of the dashboard's pages", () => {
    for (const path of ['/w/work', '/w/work/', '/w/work/projects', '/w/work/projects/abc/list', '/w/work/agents'])
      expect(isPagePath(path), path).toBe(true);
    for (const path of ['/w', '/w/', '/w/work/x', '/w/work/projects/a/b/c']) expect(isPagePath(path), path).toBe(false);
    const routes = join(dirname(fileURLToPath(import.meta.url)), '../../src/routes/w/[workspace]');
    for (const page of [
      '+page.svelte',
      'projects/+page.svelte',
      'projects/[id]/+page.svelte',
      'projects/[id]/[tab=view]/+page.svelte',
      '[tab=overview]/+page.svelte',
    ])
      expect(existsSync(join(routes, page)), page).toBe(true);
  });
});
