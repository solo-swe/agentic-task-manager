import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
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
