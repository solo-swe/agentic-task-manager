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

  it('every project shows its progress as its card counts it: the active sprint’s, else its own; at 0% too (T-533)', () => {
    const sprint = (done: number, total: number) => ({
      id: 'Now',
      state: 'active' as const,
      due: null,
      release: null,
      done,
      total,
      percent: total ? Math.round((100 * done) / total) : 100,
      goal: null,
    });
    dashboard.projects = [
      project('s', 'Sprinting', { ...summary(80), active_sprint: sprint(1, 4) }),
      project('z', 'Just started', { ...summary(50), active_sprint: sprint(0, 6) }),
      project('n', 'No sprints', summary(30)),
      project('e', 'Empty sprint', { ...summary(60), active_sprint: sprint(0, 0) }),
      project('u', 'Not summed up', null),
    ];
    const { container } = render(Sidebar);
    const bar = (id: string) => container.querySelector<HTMLElement>(`[data-id="${id}"] .app-sidebar-bar`)!;
    expect(bar('s').dataset.percent).toBe('25');
    expect(bar('s').title).toBe('Now: 1 of 4 done (25%)');
    // 0%: the bar is there, its track drawn, and it says so
    expect(bar('z').dataset.percent).toBe('0');
    expect(bar('z').title).toBe('Now: 0 of 6 done (0%)');
    expect(bar('n').title).toBe('3 of 10 done (30%)');
    expect(bar('e').title).toBe('Now: no tasks yet');
    expect(bar('u').title).toBe('Progress not known yet');
    expect(bar('u').getAttribute('aria-label')).toBe('Progress not known yet');
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
