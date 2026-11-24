import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  ALL_ITEMS,
  addressOf,
  addressParams,
  formatAddress,
  isPagePath,
  itemOf,
  ITEMS,
  legacyParams,
  namesNewTab,
  OVERVIEW_PATHS as ADDRESS_OVERVIEW_PATHS,
  REVIEW_SECTIONS,
  reviewListOf,
  reviewOf,
  reviewProjectsOf,
  withReview,
  withReviewList,
  withReviewProjects,
  withWorkspace,
  workspaceOf,
} from '../../src/lib/address';

// All reviews' project filter in its address (ATM-757, docs/sidebar-overhaul-design.md §2.5)
describe('the All reviews project filter in the address', () => {
  const known = ['atm', 'compmd', 'blog'];

  it('reads the projects named, in the workspace’s order; unknown and empty ones are left out', () => {
    expect(reviewProjectsOf('/reviews', known)).toEqual([]);
    expect(reviewProjectsOf('/reviews?projects=', known)).toEqual([]);
    expect(reviewProjectsOf('/reviews?projects=atm', known)).toEqual(['atm']);
    expect(reviewProjectsOf('/reviews?projects=blog,atm', known)).toEqual(['atm', 'blog']);
    expect(reviewProjectsOf('/reviews?projects=atm,gone,,%20compmd%20', known)).toEqual(['atm', 'compmd']);
    // Only unknown ones: every project
    expect(reviewProjectsOf('/reviews?projects=gone', known)).toEqual([]);
    expect(reviewProjectsOf('/w/work/reviews?projects=compmd&q=x', known)).toEqual(['compmd']);
  });

  it('writes them as one parameter, none for every project, keeping the rest of the address', () => {
    expect(withReviewProjects('/reviews', ['atm', 'compmd'])).toBe('/reviews?projects=atm,compmd');
    expect(withReviewProjects('/reviews?projects=atm&task=T-1', [])).toBe('/reviews?task=T-1');
    expect(withReviewProjects('/w/work/reviews', ['blog'])).toBe('/w/work/reviews?projects=blog');
    // Read back as written
    const written = withReviewProjects('/reviews', ['compmd', 'blog']);
    expect(reviewProjectsOf(written, known)).toEqual(['compmd', 'blog']);
  });
});
import { VIEWS } from '../../src/lib/tabs';
import { OVERVIEW_PATHS, PROJECT_TABS, REVIEW_SECTION_PATHS } from '../../src/params';

const params = (address: string) => [...addressParams(address)];
const at = (href: string) => {
  const url = new URL(href, 'http://localhost:7000');
  return { pathname: url.pathname, search: url.search, hash: url.hash };
};

describe('. and .. are no IDs (ATM-676, audit D7)', () => {
  it('a path with a . or .. segment, written or escaped, names no page', () => {
    for (const path of [
      '/projects/../reviews',
      '/projects/%2E%2E',
      '/projects/%2e/list',
      '/projects/p1/reviews/..',
      '/projects/p1/reviews/%2E%2E',
      '/w/../projects/p1',
      '/w/%2E%2E/projects/p1',
    ]) {
      const params = addressParams(path);
      expect(
        ['workspace', 'project', 'review'].map((k) => params.get(k)),
        path,
      ).toEqual([null, null, null]);
    }
    // A dot inside an ID is an ID
    expect(addressParams('/projects/a..b/reviews/CR-1.2').get('review')).toBe('CR-1.2');
  });

  it('an old link naming project . or .. names none', () => {
    expect(legacyParams('#project=..&view=list')?.get('project')).toBeNull();
    expect(legacyParams('#project=.&view=list')?.get('project')).toBeNull();
    expect(legacyParams('#project=p1')?.get('project')).toBe('p1');
  });
});

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
    expect(OVERVIEW_PATHS).toEqual(ADDRESS_OVERVIEW_PATHS.filter((tab) => tab !== 'projects'));
    const routes = join(dirname(fileURLToPath(import.meta.url)), '../../src/routes');
    for (const page of [
      '+page.svelte',
      'projects/+page.svelte',
      'projects/[id]/+page.svelte',
      'projects/[id]/[tab=view]/+page.svelte',
      '[tab=overview]/+page.svelte',
      'projects/[id]/reviews/[review]/+page.svelte',
      'new/+page.svelte',
    ])
      expect(existsSync(join(routes, page)), page).toBe(true);
  });
});

// The sidebar overhaul's addresses (docs/sidebar-overhaul-design.md §4, ATM-751)
describe('the sidebar’s items in the address', () => {
  it.each([
    // A project's pages: its Overview (also without a tab, or with one it doesn't know), Tasks' three, Code Reviews
    // (a review's page too), Inbox, Documents, Analysis' two
    ['/projects/abc', 'overview'],
    ['/projects/abc/project', 'overview'],
    ['/projects/abc/nope', 'overview'],
    ['/projects/abc/kanban', 'tasks'],
    ['/projects/abc/list?sprint=a', 'tasks'],
    ['/projects/abc/graph', 'tasks'],
    ['/projects/abc/reviews', 'reviews'],
    ['/projects/abc/reviews/CR-7/diffs?file=a.md', 'reviews'],
    ['/projects/abc/inbox', 'inbox'],
    ['/projects/abc/documents?doc=docs/x.md', 'documents'],
    ['/projects/abc/progress', 'analysis'],
    ['/projects/abc/agents', 'analysis'],
    // All projects': the cards, the board and its List, every review, progress and agents
    ['/projects', 'overview'],
    ['/board', 'tasks'],
    ['/list', 'tasks'],
    ['/reviews', 'reviews'],
    ['/progress', 'analysis'],
    ['/w/work/agents', 'analysis'],
    ['/w/work/projects/abc/kanban', 'tasks'],
  ])('%s is the %s item’s', (address, item) => {
    expect(itemOf(address)).toBe(item);
  });

  it('the landing page and the new tab are no item’s; what is open over a page doesn’t change its item', () => {
    expect(itemOf('/')).toBeNull();
    expect(itemOf('/w/work')).toBeNull();
    expect(itemOf('/new')).toBeNull();
    expect(itemOf('/projects/abc/kanban?task=ATM-1&doc=docs/x.md')).toBe('tasks');
  });

  it('six items for a project, four for All projects (no Inbox or Documents, CR-135)', () => {
    expect(ITEMS).toEqual(['overview', 'inbox', 'tasks', 'reviews', 'documents', 'analysis']);
    expect(ALL_ITEMS).toEqual(['overview', 'tasks', 'reviews', 'analysis']);
  });

  it('reads and writes All’s List, a project’s Inbox and Documents, and the new tab', () => {
    expect(params('/list?sprint=a')).toEqual([
      ['overview', ''],
      ['view', 'list'],
      ['sprint', 'a'],
    ]);
    expect(formatAddress(addressParams('/w/work/list'))).toBe('/w/work/list');
    expect(params('/projects/abc/inbox')).toEqual([
      ['project', 'abc'],
      ['view', 'inbox'],
    ]);
    expect(formatAddress(addressParams('/projects/abc/documents?doc=docs/x.md'))).toBe(
      '/projects/abc/documents?doc=docs/x.md',
    );
    expect(params('/new')).toEqual([['new', '']]);
    expect(formatAddress(addressParams('/new'))).toBe('/new');
    expect(formatAddress(addressParams('/w/work/new'))).toBe('/w/work/new');
    expect(namesNewTab('/new')).toBe(true);
    expect(namesNewTab('/w/work/new/')).toBe(true);
    expect(namesNewTab('/projects')).toBe(false);
    // Only the path names it: `?new` is no page, and nothing under it is one
    expect(namesNewTab('/?new')).toBe(false);
    expect(params('/new/x')).toEqual([]);
    for (const path of ['/list', '/new', '/w/work/new', '/projects/abc/inbox', '/projects/abc/documents'])
      expect(isPagePath(path), path).toBe(true);
    for (const path of ['/new/x', '/news', '/list/x']) expect(isPagePath(path), path).toBe(false);
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

  it('moving to another workspace keeps an overview tab and the dialogs, and drops the document and Sprint filter', () => {
    expect(withWorkspace('/board?sprint=a&doc=x.md&help', 'work')).toBe('/w/work/board?help');
    expect(withWorkspace('/w/a/agents', 'b')).toBe('/w/b/agents');
    expect(withWorkspace('/?view=list&about', 'work')).toBe('/w/work?about');
    expect(withWorkspace('/', 'default')).toBe('/');
  });

  it('a /w/ without a name is no workspace, and names no page', () => {
    expect(params('/w/')).toEqual([]);
    expect(params('/w')).toEqual([]);
    expect(workspaceOf('/w/')).toBe('default');
  });

  it('an overview tab it doesn’t know is the project cards', () => {
    const write = (entries: [string, string][]) => formatAddress(new URLSearchParams(entries));
    // (All's List is one since ATM-751; the graph, a project's only, isn't)
    expect(
      write([
        ['overview', ''],
        ['view', 'graph'],
      ]),
    ).toBe('/projects');
    expect(params('/kanban')).toEqual([]);
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
      'projects/[id]/reviews/[review]/+page.svelte',
      'new/+page.svelte',
    ])
      expect(existsSync(join(routes, page)), page).toBe(true);
  });
});

describe('a review in the address (ATM-574)', () => {
  it('reads a review’s page from the path, and its view from the query', () => {
    expect(params('/projects/abc/reviews/CR-7?round=3&against=1&file=src/a.rs')).toEqual([
      ['project', 'abc'],
      ['view', 'reviews'],
      ['review', 'CR-7'],
      ['round', '3'],
      ['against', '1'],
      ['file', 'src/a.rs'],
    ]);
    expect(params('/w/work/projects/abc/reviews/CR-8.2')).toEqual([
      ['workspace', 'work'],
      ['project', 'abc'],
      ['view', 'reviews'],
      ['review', 'CR-8.2'],
    ]);
    // The reviews page is a tab like any other; a review under another tab, or deeper, is no page
    expect(params('/projects/abc/reviews')).toEqual([
      ['project', 'abc'],
      ['view', 'reviews'],
    ]);
    expect(params('/projects/abc/list/CR-7')).toEqual([]);
    expect(params('/projects/abc/reviews/CR-7/files')).toEqual([]);
    expect(params('/projects/abc/reviews/CR-7/overview')).toEqual([]);
    expect(params('/projects/abc/reviews/CR-7/diffs/x')).toEqual([]);
    // The review is the path's: the query can't name one
    expect(params('/projects/abc/reviews?review=CR-1')).toEqual([
      ['project', 'abc'],
      ['view', 'reviews'],
    ]);
  });

  it('writes a review in the path, on the reviews tab only', () => {
    const write = (entries: [string, string][]) => formatAddress(new URLSearchParams(entries));
    expect(
      write([
        ['project', 'abc'],
        ['view', 'reviews'],
        ['review', 'CR 7/x'],
        ['file', 'a b.md'],
      ]),
    ).toBe('/projects/abc/reviews/CR%207%2Fx?file=a%20b.md');
    // On another tab a review is dropped, never a query parameter
    expect(
      write([
        ['project', 'abc'],
        ['view', 'list'],
        ['review', 'CR-7'],
      ]),
    ).toBe('/projects/abc/list');
    const address = '/w/work/projects/abc/reviews/CR-7?round=2&file=x.rs';
    expect(formatAddress(addressParams(address))).toBe(address);
  });

  it('a review’s page is one of the dashboard’s pages, its reference letters, digits, - and .', () => {
    for (const path of [
      '/projects/abc/reviews',
      '/projects/abc/reviews/CR-7',
      '/projects/abc/reviews/CR-8.2/',
      '/w/work/projects/abc/reviews/0193c0de-77aa',
      '/projects/abc/reviews/CR-7/diffs',
      '/w/work/projects/abc/reviews/CR-7/comments/',
    ])
      expect(isPagePath(path), path).toBe(true);
    for (const path of [
      '/projects/abc/reviews/CR-7/x',
      '/projects/abc/reviews/CR-7/overview',
      '/projects/abc/reviews/CR-7/diffs/x',
      '/projects/abc/reviews/CR_7',
      '/projects/abc/reviews/CR%207',
      `/projects/abc/reviews/${'x'.repeat(65)}`,
      '/projects/abc/list/CR-7',
    ])
      expect(isPagePath(path), path).toBe(false);
  });

  it('reads the review, round, round against and file a page shows', () => {
    expect(reviewOf('/projects/abc/reviews/CR-7?round=3&against=1&file=a.md')).toEqual({
      review: 'CR-7',
      section: 'overview',
      round: 3,
      against: 1,
      file: 'a.md',
      comment: null,
    });
    expect(reviewOf('/projects/abc/reviews/CR-7')).toEqual({
      review: 'CR-7',
      section: 'overview',
      round: null,
      against: null,
      file: null,
      comment: null,
    });
    // A round that isn't a round's number is none
    for (const round of ['0', '-1', 'x', '1.5', ''])
      expect(reviewOf(`/projects/abc/reviews/CR-7?round=${round}`).round, round).toBeNull();
    // No review: nothing of one, whatever the query says
    expect(reviewOf('/projects/abc/reviews?round=3&file=a.md&comment=c1')).toEqual({
      review: null,
      section: 'overview',
      round: null,
      against: null,
      file: null,
      comment: null,
    });
    expect(reviewOf('/projects/abc/list').review).toBeNull();
  });

  it('moves to a review, a round, a file, or back to the list, keeping the rest', () => {
    const from = '/projects/abc/list?sprint=s1&task=T-1';
    expect(withReview(from, { review: 'CR-7' })).toBe('/projects/abc/reviews/CR-7?sprint=s1&task=T-1');
    expect(withReview('/projects/abc/reviews/CR-7?file=a.md', { review: 'CR-7', round: 2, against: 1 })).toBe(
      '/projects/abc/reviews/CR-7?round=2&against=1',
    );
    expect(withReview('/projects/abc/reviews/CR-7?round=2', { review: 'CR-7', round: 2, file: 'x/y.rs' })).toBe(
      '/projects/abc/reviews/CR-7?round=2&file=x/y.rs',
    );
    // Back to the list: the review's view goes with it
    expect(withReview('/w/work/projects/abc/reviews/CR-7?round=2&against=1&file=a.md', { review: null })).toBe(
      '/w/work/projects/abc/reviews',
    );
  });
});

describe('a review’s sub-tab, a comment in view, the Reviews list’s filter (ATM-697)', () => {
  it('reads and writes the sub-tab in the path, the Overview at the review’s own', () => {
    expect(params('/projects/abc/reviews/CR-7/diffs?file=a.md')).toEqual([
      ['project', 'abc'],
      ['view', 'reviews'],
      ['review', 'CR-7'],
      ['section', 'diffs'],
      ['file', 'a.md'],
    ]);
    expect(reviewOf('/w/work/projects/abc/reviews/CR-7/comments').section).toBe('comments');
    // The path's alone: a query naming one names none
    expect(reviewOf('/projects/abc/reviews/CR-7?section=diffs').section).toBe('overview');
    for (const address of ['/projects/abc/reviews/CR-7/diffs?file=a.md', '/w/w/projects/abc/reviews/CR-7/comments'])
      expect(formatAddress(addressParams(address))).toBe(address);
    expect(withReview('/projects/abc/reviews/CR-7', { review: 'CR-7', section: 'diffs', file: 'a.md' })).toBe(
      '/projects/abc/reviews/CR-7/diffs?file=a.md',
    );
    expect(withReview('/projects/abc/reviews/CR-7/diffs', { review: 'CR-7', section: 'overview' })).toBe(
      '/projects/abc/reviews/CR-7',
    );
  });

  it('reads and writes the comment in view in the query, never as an anchor', () => {
    const address = '/projects/abc/reviews/CR-7/diffs?file=docs/x.md&comment=c12';
    expect(reviewOf(address)).toMatchObject({ section: 'diffs', file: 'docs/x.md', comment: 'c12' });
    expect(withReview(address, { review: 'CR-7', section: 'diffs', file: 'docs/x.md', comment: 'c12' })).toBe(address);
    expect(reviewOf('/projects/abc/reviews/CR-7?comment=').comment).toBeNull();
    // Another review, or the list: the comment goes
    expect(withReview(address, { review: 'CR-8' })).toBe('/projects/abc/reviews/CR-8');
    expect(withReview(address, { review: null })).toBe('/projects/abc/reviews');
  });

  it('reads and writes the list’s filter and search, each field it names', () => {
    const query = {
      q: 'tabs nav',
      status: ['waiting', 'live'],
      author: 'ana',
      reviewer: '',
      task: 'ATM-7',
      branch: '',
    };
    const address = withReviewList('/projects/abc/reviews', query);
    expect(address).toBe('/projects/abc/reviews?q=tabs%20nav&status=waiting,live&author=ana&tasks=ATM-7');
    expect(reviewListOf(address)).toEqual(query);
    // Naming none: none (the list's kept one applies); a review's page or another tab: none
    expect(reviewListOf('/projects/abc/reviews')).toBeNull();
    expect(reviewListOf('/projects/abc/reviews?sprint=s1')).toBeNull();
    expect(reviewListOf('/projects/abc/reviews/CR-7?q=x')).toBeNull();
    expect(reviewListOf('/projects/abc/list?q=x')).toBeNull();
    // An empty filter clears it; the rest of the address stays
    expect(withReviewList(`${address}&sprint=s1`, { ...query, q: '', status: [], author: '', task: '' })).toBe(
      '/projects/abc/reviews?sprint=s1',
    );
    // Opening a review leaves the list's filter behind; the list's own task details stay
    expect(withReview(`${address}&task=T-1`, { review: 'CR-7' })).toBe('/projects/abc/reviews/CR-7?task=T-1');
  });

  it('the sub-tab route’s values are the address’s', () => {
    expect(REVIEW_SECTION_PATHS).toEqual(REVIEW_SECTIONS.filter((s) => s !== 'overview'));
  });
});
