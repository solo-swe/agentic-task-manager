import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appTabs } from '../../src/lib/app-tabs.svelte';
import type { ProjectEntry, ProjectSummary } from '../../src/lib/load';
import { setPref } from '../../src/lib/prefs';
import Sidebar from '../../src/lib/shell/Sidebar.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import { summary as fullSummary } from './summary.helpers';

// The app's sidebar (ATM-753, docs/sidebar-overhaul-design.md §2): a scope selector and the scope's items

const summary = (percent: number, review = 0, blocked = 0, codeReviews = 0): ProjectSummary =>
  fullSummary({
    total: 10,
    done: percent / 10,
    percent,
    needs_you: { review, blocked_on_user: blocked, together: 0, code_reviews: codeReviews },
  });
const project = (id: string, name: string, s: ProjectSummary | null, missing = false): ProjectEntry => ({
  id,
  name,
  path: `/work/${id}`,
  missing,
  summary: s,
});

const items = (container: HTMLElement) =>
  [...container.querySelectorAll<HTMLAnchorElement>('.app-sidebar-link')].map((a) => a.dataset.item);
const scopeButton = (container: HTMLElement) => container.querySelector<HTMLElement>('#app-sidebar-scope')!;

describe('Sidebar (ATM-753)', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    dashboard.projects = [
      project('a', 'Alpha', summary(40, 1, 0, 2)),
      project('b', 'Beta', summary(90, 0, 2, 1)),
      project('c', 'Gone', null, true),
    ];
    dashboard.projectsLoaded = true;
    dashboard.route = { page: 'project', id: 'a' };
    dashboard.tab = 'kanban';
    dashboard.sidebarOpen = true;
    appTabs.start('/projects/a/kanban');
  });
  afterEach(() => history.replaceState(null, '', '/'));

  it('the scope is the active tab’s project, with what needs you; its six items, the page shown marked', () => {
    const { container } = render(Sidebar);
    expect(scopeButton(container)).toHaveAttribute('data-scope', 'a');
    expect(scopeButton(container)).toHaveTextContent('Alpha');
    // What needs you: a review and two code reviews
    expect(scopeButton(container).querySelector('.app-sidebar-badge')).toHaveTextContent('3');
    expect(items(container)).toEqual(['overview', 'inbox', 'tasks', 'reviews', 'documents', 'analysis']);
    const tasks = container.querySelector('[data-item="tasks"]')!;
    expect(tasks).toHaveAttribute('aria-current', 'page');
    expect(tasks).toHaveAttribute('href', '/projects/a/kanban');
    expect(container.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
    // Code Reviews: the reviews waiting on you; no badge on the others
    expect(container.querySelector('[data-item="reviews"] .app-sidebar-badge')).toHaveTextContent('2');
    expect(container.querySelector('[data-item="inbox"] .app-sidebar-badge')).toBeNull();
    // The page leaves room for it
    expect(document.documentElement.style.getPropertyValue('--sidebar-offset')).toBe('240px');
  });

  it('Inbox: the items waiting in the project’s inbox, from its summary (ATM-762)', () => {
    dashboard.projects = [{ ...project('a', 'Alpha', { ...summary(40), inbox: { open: 4 } }) }];
    const { container } = render(Sidebar);
    expect(container.querySelector('[data-item="inbox"] .app-sidebar-badge')).toHaveTextContent('4');
  });

  it('on All projects’ page: All, its four items, the reviews waiting summed over the projects', async () => {
    dashboard.route = { page: 'overview' };
    dashboard.overviewTab = 'progress';
    const { container } = render(Sidebar);
    expect(scopeButton(container)).toHaveAttribute('data-scope', '');
    expect(scopeButton(container)).toHaveTextContent('All projects');
    expect(items(container)).toEqual(['overview', 'tasks', 'reviews', 'analysis']);
    expect(container.querySelector('[data-item="analysis"]')).toHaveAttribute('aria-current', 'page');
    expect(container.querySelector('[data-item="tasks"]')).toHaveAttribute('href', '/board');
    expect(container.querySelector('[data-item="reviews"] .app-sidebar-badge')).toHaveTextContent('3');
  });

  it('follows the active tab: another project’s page changes the scope', async () => {
    const { container } = render(Sidebar);
    dashboard.route = { page: 'project', id: 'b' };
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', 'b');
    expect(container.querySelector('[data-item="overview"]')).toHaveAttribute('href', '/projects/b/project');
  });

  it('a scope kept in the preferences holds on the first tab until it changes; another page follows again', async () => {
    setPref('sidebarScope', 'b');
    const { container } = render(Sidebar);
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', 'b');
    // Not the page's scope: no item marked
    expect(container.querySelectorAll('[aria-current="page"]')).toHaveLength(0);
    dashboard.route = { page: 'overview' };
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', '');
  });

  it('a switch of tab ends the choice by hand: back on its tab, the scope is the page’s again', async () => {
    setPref('sidebarScope', 'b');
    const { container } = render(Sidebar);
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', 'b');
    const first = appTabs.active!;
    appTabs.open('/projects/a/list');
    await Promise.resolve();
    appTabs.select(first);
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', 'a');
  });

  it('a missing project kept in the preferences is All projects', async () => {
    setPref('sidebarScope', 'c');
    const { container } = render(Sidebar);
    await Promise.resolve();
    expect(scopeButton(container)).toHaveAttribute('data-scope', '');
  });

  it('an item always opens a new tab, shown; ⌘-click in the background; never the tab shown', async () => {
    const { container } = render(Sidebar);
    const first = appTabs.active;
    // Even the page the tab already shows
    await fireEvent.click(container.querySelector('[data-item="tasks"]')!);
    expect(appTabs.tabs).toHaveLength(2);
    expect(appTabs.current?.address).toBe('/projects/a/kanban');
    expect(appTabs.active).not.toBe(first);
    const shown = appTabs.active;
    await fireEvent.click(container.querySelector('[data-item="analysis"]')!, { metaKey: true });
    expect(appTabs.tabs).toHaveLength(3);
    expect(appTabs.active).toBe(shown);
    expect(appTabs.tabs.map((t) => t.address)).toContain('/projects/a/progress');
  });

  it('the arrows on its edge resize it, kept within bounds; hidden, it takes no room', async () => {
    const { container } = render(Sidebar);
    const edge = container.querySelector<HTMLElement>('#app-sidebar-edge')!;
    await fireEvent.keyDown(edge, { key: 'ArrowRight' });
    expect(container.querySelector<HTMLElement>('#app-sidebar')!.style.width).toBe('256px');
    for (let i = 0; i < 20; i++) await fireEvent.keyDown(edge, { key: 'ArrowLeft' });
    expect(container.querySelector<HTMLElement>('#app-sidebar')!.style.width).toBe('180px');
    dashboard.sidebarOpen = false;
    await Promise.resolve();
    expect(container.querySelector('#app-sidebar')).toBeNull();
  });
});

describe('the hidden sidebar’s rail (ATM-771)', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    dashboard.projects = [project('a', 'Alpha', summary(40, 1, 0, 2)), project('b', 'Beta', summary(90))];
    dashboard.projectsLoaded = true;
    dashboard.route = { page: 'project', id: 'a' };
    dashboard.tab = 'kanban';
    dashboard.sidebarOpen = false;
    appTabs.start('/projects/a/kanban');
  });
  afterEach(() => {
    dashboard.sidebarOpen = true;
    history.replaceState(null, '', '/');
  });

  const rail = (container: HTMLElement) => container.querySelector<HTMLElement>('#app-sidebar-rail');
  const railItems = (container: HTMLElement) =>
    [...container.querySelectorAll<HTMLAnchorElement>('.app-sidebar-rail-item')].map((a) => a.dataset.item);
  const zone = (container: HTMLElement) => container.querySelector<HTMLElement>('.app-sidebar-zone')!;

  it('hidden: the rail, the scope’s icon and each item’s icon and badge, no labels; the page leaves its room', () => {
    const { container } = render(Sidebar);
    expect(container.querySelector('#app-sidebar')).toBeNull();
    expect(rail(container)).not.toBeNull();
    expect(container.querySelector('#app-sidebar-rail-scope')).toHaveAttribute('data-scope', 'a');
    expect(container.querySelector('#app-sidebar-rail-scope')).toHaveAttribute('title', 'Alpha');
    expect(railItems(container)).toEqual(['overview', 'inbox', 'tasks', 'reviews', 'documents', 'analysis']);
    // Icons only: the names are their labels and titles, never text
    expect(rail(container)!.textContent?.replace(/\d/g, '').trim()).toBe('');
    const reviews = container.querySelector('.app-sidebar-rail-item[data-item="reviews"]')!;
    expect(reviews).toHaveAttribute('aria-label', 'Code Reviews (2 waiting for you)');
    expect(reviews.querySelector('.app-sidebar-rail-badge')).toHaveTextContent('2');
    expect(container.querySelector('.app-sidebar-rail-item[data-item="tasks"]')).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(document.documentElement.style.getPropertyValue('--sidebar-offset')).toBe('0px');
    expect(document.documentElement.style.getPropertyValue('--sidebar-rail')).toBe('44px');
  });

  it('All projects: its icon, and All’s items', async () => {
    dashboard.route = { page: 'overview' };
    setPref('sidebarScope', '');
    const { container } = render(Sidebar);
    await Promise.resolve();
    expect(container.querySelector('#app-sidebar-rail-scope')).toHaveAttribute('title', 'All projects');
    expect(railItems(container)).not.toContain('inbox');
  });

  it('the pointer resting on it shows the whole sidebar over the page, which keeps its room; leaving hides it, unsaved', async () => {
    vi.useFakeTimers();
    const { container } = render(Sidebar);
    await fireEvent.mouseEnter(zone(container));
    // Not at once: a quick click on an icon is the icon's
    await vi.advanceTimersByTimeAsync(200);
    expect(container.querySelector('#app-sidebar')).toBeNull();
    await vi.advanceTimersByTimeAsync(100);
    vi.useRealTimers();
    const shown = container.querySelector('#app-sidebar')!;
    expect(shown).not.toBeNull();
    expect(shown).toHaveClass('peeking');
    expect(document.documentElement.style.getPropertyValue('--sidebar-offset')).toBe('0px');
    expect(dashboard.sidebarOpen).toBe(false);
    await fireEvent.mouseLeave(zone(container));
    expect(container.querySelector('#app-sidebar')).toBeNull();
    expect(dashboard.sidebarOpen).toBe(false);
  });

  it('the keyboard’s focus in it shows it; the focus leaving, or Escape, hides it; a click’s focus doesn’t hold it', async () => {
    const { container } = render(Sidebar);
    const tasks = container.querySelector<HTMLElement>('.app-sidebar-rail-item[data-item="tasks"]')!;
    await fireEvent.focusIn(tasks);
    expect(container.querySelector('#app-sidebar')).not.toBeNull();
    await fireEvent.focusOut(tasks, { relatedTarget: document.body });
    expect(container.querySelector('#app-sidebar')).toBeNull();
    await fireEvent.focusIn(tasks);
    await fireEvent.keyDown(zone(container), { key: 'Escape' });
    expect(container.querySelector('#app-sidebar')).toBeNull();
    // A click's focus doesn't show it, nor hold it once the pointer leaves
    await fireEvent.mouseEnter(zone(container));
    await fireEvent.pointerDown(tasks);
    await fireEvent.focusIn(tasks);
    expect(container.querySelector('#app-sidebar')).toBeNull();
    await fireEvent.mouseLeave(zone(container));
    expect(container.querySelector('#app-sidebar')).toBeNull();
  });

  it('a quick click on an icon: its item, and no sidebar shown over the rail', async () => {
    vi.useFakeTimers();
    const { container } = render(Sidebar);
    await fireEvent.mouseEnter(zone(container));
    const reviews = container.querySelector<HTMLElement>('.app-sidebar-rail-item[data-item="reviews"]')!;
    await fireEvent.pointerDown(reviews);
    await fireEvent.click(reviews);
    await vi.advanceTimersByTimeAsync(1000);
    vi.useRealTimers();
    expect(appTabs.current?.address).toBe('/projects/a/reviews');
    expect(container.querySelector('#app-sidebar')).toBeNull();
  });

  it('a rail icon opens its item as the sidebar’s does: a new tab, ⌘-click in the background', async () => {
    const { container } = render(Sidebar);
    const first = appTabs.active;
    await fireEvent.click(container.querySelector('.app-sidebar-rail-item[data-item="reviews"]')!);
    expect(appTabs.tabs).toHaveLength(2);
    expect(appTabs.current?.address).toBe('/projects/a/reviews');
    expect(appTabs.active).not.toBe(first);
    const shown = appTabs.active;
    await fireEvent.click(container.querySelector('.app-sidebar-rail-item[data-item="analysis"]')!, { metaKey: true });
    expect(appTabs.tabs).toHaveLength(3);
    expect(appTabs.active).toBe(shown);
  });

  it('shown again (Show Sidebar), no rail; the sidebar takes its room', async () => {
    const { container } = render(Sidebar);
    dashboard.sidebarOpen = true;
    await Promise.resolve();
    expect(rail(container)).toBeNull();
    expect(container.querySelector('#app-sidebar')).not.toHaveClass('peeking');
    expect(document.documentElement.style.getPropertyValue('--sidebar-rail')).toBe('0px');
    expect(document.documentElement.style.getPropertyValue('--sidebar-offset')).toBe('240px');
  });
});
