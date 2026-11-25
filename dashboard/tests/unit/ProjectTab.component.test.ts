import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ProjectTab from '../../src/lib/shell/ProjectTab.svelte';
import { sprintBoardAddress, statusListAddress } from '../../src/lib/sprint-param';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

const navigate = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/navigation', async (actual) => ({ ...(await actual()), navigate }));

// A project's Overview (the Project tab) links into the Board and the List (ATM-756, docs/sidebar-overhaul-design.md
// §2.4): each sprint and the backlog to the Board on it, the counts of to do, in review and blocked to the List

/** A link's address, as its parts: the path and each sprint it names */
const parts = (href: string | null) => {
  const url = new URL(href ?? '', 'http://x');
  return { path: url.pathname, sprint: url.searchParams.getAll('sprint') };
};

describe('ProjectTab’s links (ATM-756)', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    dashboard.projectId = 'fixture';
    history.replaceState(null, '', '/projects/fixture/project?task=T-001');
    navigate.mockClear();
  });
  afterEach(() => {
    dashboard.projectId = null;
    history.replaceState(null, '', '/');
  });

  it('each sprint listed and the backlog open the Board on it; the closed ones too, once shown', async () => {
    const { container } = render(ProjectTab);
    const sprints = () => [...container.querySelectorAll<HTMLAnchorElement>('#project-sprints a[data-sprint]')];
    expect(sprints().length).toBeGreaterThan(1);
    for (const link of sprints()) {
      const id = link.dataset.sprint!;
      expect(parts(link.getAttribute('href')), id).toEqual({
        path: '/projects/fixture/kanban',
        sprint: [id === 'backlog' ? 'backlog' : id],
      });
    }
    const toggle = container.querySelector<HTMLButtonElement>('#project-closed-toggle');
    if (toggle) {
      const before = sprints().length;
      await fireEvent.click(toggle);
      expect(sprints().length).toBeGreaterThan(before);
      for (const link of sprints().filter((l) => l.dataset.state === 'closed'))
        expect(parts(link.getAttribute('href')).sprint).toEqual([link.dataset.sprint]);
    }
  });

  it('the counts of to do, in review and blocked link to the List over every sprint; the others stay text', () => {
    const { container } = render(ProjectTab);
    const linked = [...container.querySelectorAll<HTMLAnchorElement>('#project-statuses a[data-status]')];
    const kinds = new Map(dashboard.vocab.statuses.map((s) => [s.id, s.kind]));
    expect(linked.map((l) => kinds.get(l.dataset.status!))).toEqual(
      dashboard.vocab.statuses.filter((s) => ['todo', 'review', 'blocked'].includes(s.kind)).map((s) => s.kind),
    );
    for (const link of linked)
      expect(parts(link.getAttribute('href'))).toEqual({ path: '/projects/fixture/list', sprint: ['all'] });
    const text = [...container.querySelectorAll('#project-statuses span[data-status]')].map((s) =>
      kinds.get((s as HTMLElement).dataset.status!),
    );
    expect(text).not.toContain('todo');
    expect(text).toContain('done');
  });

  it('a status count clicked: the List, with that status alone in the Filters and no search; ⌘-click left to the link', async () => {
    dashboard.query = 'something';
    const { container } = render(ProjectTab);
    const review = dashboard.vocab.statuses.find((s) => s.kind === 'review')!.id;
    const link = container.querySelector<HTMLAnchorElement>(`#project-statuses a[data-status="${review}"]`)!;
    await fireEvent.click(link, { metaKey: true });
    expect(navigate).not.toHaveBeenCalled();
    await fireEvent.click(link);
    expect([...dashboard.filters.status]).toEqual([review]);
    expect(dashboard.filters.sprint.size).toBe(0);
    expect(dashboard.query).toBe('');
    expect(navigate).toHaveBeenCalledWith('/projects/fixture/list?sprint=all');
  });

  it('the addresses keep the workspace and leave out what is open over the page', () => {
    expect(sprintBoardAddress('/w/work/projects/p1/project?task=T-1&doc=a.md', 'Phase 2')).toBe(
      '/w/work/projects/p1/kanban?sprint=Phase%202',
    );
    expect(statusListAddress('/w/work/projects/p1/project?task=T-1')).toBe('/w/work/projects/p1/list?sprint=all');
    expect(statusListAddress('/projects/p1')).toBe('/projects/p1/list?sprint=all');
  });
});
