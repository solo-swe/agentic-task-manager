import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appTabs } from '../../src/lib/app-tabs.svelte';
import type { ProjectEntry, ProjectSummary } from '../../src/lib/load';
import Sidebar from '../../src/lib/shell/Sidebar.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import { summary as fullSummary } from './summary.helpers';

const summary = (percent: number, review = 0, blocked = 0): ProjectSummary =>
  fullSummary({ total: 10, done: percent / 10, percent, needs_you: { review, blocked_on_user: blocked, together: 0 } });
const project = (id: string, name: string, s: ProjectSummary | null, missing = false): ProjectEntry => ({
  id,
  name,
  path: `/work/${id}`,
  missing,
  summary: s,
});

describe('Sidebar (T-515)', () => {
  beforeEach(() => {
    dashboard.projects = [
      project('a', 'Alpha', summary(40, 1)),
      project('b', 'Beta', summary(90, 0, 2)),
      project('c', 'Gone', null, true),
    ];
    dashboard.route = { page: 'project', id: 'a' };
    dashboard.sidebarOpen = true;
    appTabs.start('/projects/a');
  });
  afterEach(() => history.replaceState(null, '', '/'));

  it('lists the overview and the projects: the open one marked, what needs you badged, a missing one dimmed', () => {
    const { container } = render(Sidebar);
    const items = [...container.querySelectorAll<HTMLButtonElement>('.app-sidebar-item')];
    expect(items.map((i) => i.dataset.id)).toEqual(['overview', 'a', 'b', 'c']);
    expect(items[1]).toHaveAttribute('aria-current', 'page');
    expect(items[1].querySelector('.app-sidebar-badge')).toHaveTextContent('1');
    expect(items[2].querySelector('.app-sidebar-badge')).toHaveTextContent('2');
    expect(items[3]).toBeDisabled();
    expect(items[3].querySelector('.app-sidebar-bar')).toBeNull();
    // The page leaves room for it
    expect(document.documentElement.style.getPropertyValue('--sidebar-offset')).toBe('240px');
  });

  it('a click opens a project here; ⌘-click in a new tab', async () => {
    const opened = vi.spyOn(dashboard, 'openProject').mockImplementation(() => {});
    const { container } = render(Sidebar);
    await fireEvent.click(container.querySelector('[data-id="b"]')!);
    expect(opened).toHaveBeenCalledWith('b');
    await fireEvent.click(container.querySelector('[data-id="b"]')!, { metaKey: true });
    expect(appTabs.tabs).toHaveLength(2);
    expect(appTabs.current?.address).toMatch(/\/projects\/b/);
    opened.mockRestore();
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
