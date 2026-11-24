import { describe, expect, it } from 'vitest';
import { placeName, scopeName, tabTitle, type ProjectNaming } from '../../src/lib/places';

// A tab's title (ATM-754, docs/sidebar-overhaul-design.md §3): `<scope> - <item>` with the item's icon
describe('tabTitle', () => {
  const projects: Record<string, ProjectNaming> = {
    atm: { key: 'ATM', name: 'Agentic Task Manager' },
    blog: { key: null, name: 'Blog' },
    long: { name: 'A project with a long name' },
  };
  const of = (id: string) => projects[id];
  const title = (address: string) => tabTitle(address, of);

  it('names each item of a project by its key, with the item’s icon', () => {
    expect(title('/projects/atm')).toEqual({ title: 'ATM - Overview', icon: 'layout-dashboard' });
    expect(title('/projects/atm/project')).toEqual({ title: 'ATM - Overview', icon: 'layout-dashboard' });
    expect(title('/projects/atm/inbox')).toEqual({ title: 'ATM - Inbox', icon: 'inbox' });
    for (const view of ['kanban', 'list', 'graph'])
      expect(title(`/projects/atm/${view}`), view).toEqual({ title: 'ATM - Tasks', icon: 'square-kanban' });
    expect(title('/projects/atm/reviews')).toEqual({ title: 'ATM - Code Reviews', icon: 'file-diff' });
    expect(title('/projects/atm/documents')).toEqual({ title: 'ATM - Documents', icon: 'file-text' });
    for (const view of ['progress', 'agents'])
      expect(title(`/projects/atm/${view}`), view).toEqual({ title: 'ATM - Analysis', icon: 'trending-up' });
    // A task open over the page doesn't change it
    expect(title('/projects/atm/kanban?task=ATM-1').title).toBe('ATM - Tasks');
  });

  it('All projects is All, on each of its items; the landing page is All’s Overview', () => {
    expect(title('/projects')).toEqual({ title: 'All - Overview', icon: 'layout-dashboard' });
    expect(title('/')).toEqual({ title: 'All - Overview', icon: 'layout-dashboard' });
    for (const tab of ['board', 'list']) expect(title(`/${tab}`).title, tab).toBe('All - Tasks');
    expect(title('/reviews')).toEqual({ title: 'All - Code Reviews', icon: 'file-diff' });
    for (const tab of ['progress', 'agents']) expect(title(`/${tab}`).title, tab).toBe('All - Analysis');
    expect(title('/w/work/reviews').title).toBe('All - Code Reviews');
  });

  it('a review or a document names itself, keeping the item’s icon', () => {
    expect(title('/projects/atm/reviews/CR-134')).toEqual({ title: 'ATM - CR-134', icon: 'file-diff' });
    expect(title('/projects/atm/reviews/CR-134/diffs?round=2').title).toBe('ATM - CR-134');
    expect(title('/projects/atm/kanban?doc=docs/sidebar-overhaul-design.md')).toEqual({
      title: 'ATM - sidebar-overhaul-design.md',
      icon: 'square-kanban',
    });
    expect(title('/projects/atm/documents?doc=README.md').title).toBe('ATM - README.md');
    expect(title(`/projects/atm/kanban?doc=${encodeURIComponent('docs/x.md#L4-L9')}`).title).toBe('ATM - x.md');
    // A document over All's page
    expect(title('/board?doc=docs/a.md').title).toBe('All - a.md');
  });

  it('a project without a key by its name, cut to 16 characters; one not listed by its ID', () => {
    expect(title('/projects/blog/kanban').title).toBe('Blog - Tasks');
    expect(title('/projects/long/kanban').title).toBe('A project with … - Tasks');
    expect(scopeName('long', projects.long)).toHaveLength(16);
    expect(title('/projects/gone/kanban').title).toBe('gone - Tasks');
    expect(scopeName('x', { key: '  ', name: 'Named' })).toBe('Named');
    expect(scopeName('x', { name: '' })).toBe('x');
    expect(scopeName(null, undefined)).toBe('All');
  });

  it('the new tab is New Tab', () => {
    expect(title('/new')).toEqual({ title: 'New Tab', icon: 'plus' });
    expect(title('/w/work/new').title).toBe('New Tab');
  });
});

// A place's name from its address (lib/places.ts, ATM-686): what a review's back button says it goes back to

describe('placeName', () => {
  it.each([
    // A project without a tab is on its Overview (ATM-751, ATM-752: the Project tab is the Overview item); the Kanban
    // is the Board, the graph Dependencies, and All's board the Board too
    ['/projects/abc', 'Overview'],
    ['/projects/abc/kanban', 'Board'],
    ['/projects/abc/list', 'List'],
    ['/projects/abc/graph', 'Dependencies'],
    ['/projects/abc/progress', 'Progress'],
    ['/projects/abc/agents', 'Agents'],
    ['/projects/abc/project', 'Overview'],
    ['/projects/abc/reviews', 'Reviews'],
    ['/projects/abc/inbox', 'Inbox'],
    ['/projects/abc/documents', 'Documents'],
    ['/projects', 'Projects'],
    ['/board', 'Board'],
    ['/list', 'List'],
    ['/reviews', 'All reviews'],
    ['/progress', 'Progress'],
    ['/agents', 'Agents'],
    ['/w/work/board', 'Board'],
    ['/', 'Projects'],
  ])('a page: %s is %s', (address, name) => {
    expect(placeName(address)).toBe(name);
  });

  it('what is open over a page names it: a task’s details, a document', () => {
    expect(placeName('/projects/abc/kanban?task=ATM-12')).toBe('ATM-12');
    expect(placeName('/board?task=LFB-3')).toBe('LFB-3');
    expect(placeName('/projects/abc/list?doc=docs/x.md')).toBe('Document');
  });

  it('another review is named by its reference', () => {
    expect(placeName('/projects/abc/reviews/CR-7?round=2')).toBe('CR-7');
  });
});
