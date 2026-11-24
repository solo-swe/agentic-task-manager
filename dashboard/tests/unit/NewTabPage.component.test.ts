import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import NewTabPage from '../../src/lib/shell/NewTabPage.svelte';
import { setPref } from '../../src/lib/prefs';
import { dashboard } from '../../src/lib/state.svelte';

const navigate = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/navigation', async (actual) => ({ ...(await actual()), navigate }));

// The new tab's chooser (lib/shell/NewTabPage.svelte, ATM-755): the scopes and the chosen scope's items, an item a
// link to its page in this tab, the keys moving it

const project = (id: string, name: string, waiting = { review: 0, code_reviews: 0 }) => ({
  id,
  name,
  path: `/p/${id}`,
  missing: false,
  logo: null,
  worktrees: [],
  summary: {
    needs_you: { review: waiting.review, blocked_on_user: 0, together: 0, code_reviews: waiting.code_reviews },
  },
});

describe('NewTabPage', () => {
  beforeEach(() => {
    dashboard.projects = [
      project('atm', 'Agentic Task Manager', { review: 1, code_reviews: 2 }),
      project('compmd', 'CompMD'),
      { ...project('gone', 'Gone'), missing: true },
    ] as unknown as typeof dashboard.projects;
    setPref('sidebarScope', '');
    navigate.mockClear();
  });
  afterEach(() => {
    dashboard.projects = [];
    setPref('sidebarScope', '');
  });

  const scopes = (c: HTMLElement) => [...c.querySelectorAll<HTMLElement>('#new-tab-scopes [data-scope]')];
  const items = (c: HTMLElement) => [...c.querySelectorAll<HTMLAnchorElement>('#new-tab-items [data-item]')];

  it('lists All, then the projects there, with what needs you; All’s four items, each a link to its page', () => {
    const { container } = render(NewTabPage);
    expect(scopes(container).map((s) => s.dataset.scope)).toEqual(['', 'atm', 'compmd']);
    expect(scopes(container)[0]).toHaveAttribute('aria-pressed', 'true');
    // All sums its projects'; a project its own
    expect(scopes(container)[0].querySelector('.new-tab-badge')).toHaveTextContent('3');
    expect(scopes(container)[1].querySelector('.new-tab-badge')).toHaveTextContent('3');
    expect(scopes(container)[2].querySelector('.new-tab-badge')).toBeNull();
    expect(items(container).map((a) => [a.dataset.item, a.getAttribute('href')])).toEqual([
      ['overview', '/projects'],
      ['tasks', '/board'],
      ['reviews', '/reviews'],
      ['analysis', '/progress'],
    ]);
    // Code Reviews: the reviews waiting on you
    expect(items(container)[2].querySelector('.new-tab-badge')).toHaveTextContent('2');
  });

  it('opens on the sidebar’s scope; a scope clicked shows its six items', async () => {
    setPref('sidebarScope', 'compmd');
    const { container } = render(NewTabPage);
    expect(container.querySelector('[data-scope="compmd"]')).toHaveAttribute('aria-pressed', 'true');
    await fireEvent.click(container.querySelector('[data-scope="atm"]')!);
    expect(items(container).map((a) => a.getAttribute('href'))).toEqual([
      '/projects/atm/project',
      '/projects/atm/inbox',
      '/projects/atm/kanban',
      '/projects/atm/reviews',
      '/projects/atm/documents',
      '/projects/atm/progress',
    ]);
  });

  it('has the focus, so the keys move it at once; Enter opens the item in this tab', async () => {
    const { container } = render(NewTabPage);
    const root = container.querySelector<HTMLElement>('#new-tab')!;
    expect(document.activeElement).toBe(root);
    await fireEvent.keyDown(root, { key: 'ArrowDown' });
    await fireEvent.keyDown(root, { key: 'ArrowRight' });
    await fireEvent.keyDown(root, { key: 'ArrowDown' });
    await fireEvent.keyDown(root, { key: 'ArrowDown' });
    expect(container.querySelector('#new-tab-items .chosen')).toHaveAttribute('data-item', 'tasks');
    await fireEvent.keyDown(root, { key: 'Enter' });
    expect(navigate).toHaveBeenCalledWith('/projects/atm/kanban');
  });

  it('typing filters the projects, and says so; ⌘ with a key is left to the app', async () => {
    const { container } = render(NewTabPage);
    const root = container.querySelector<HTMLElement>('#new-tab')!;
    await fireEvent.keyDown(root, { key: 'c' });
    await fireEvent.keyDown(root, { key: 'o' });
    expect(scopes(container).map((s) => s.dataset.scope)).toEqual(['compmd']);
    expect(container.querySelector('#new-tab-filter')).toHaveTextContent('“co”');
    await fireEvent.keyDown(root, { key: 'z' });
    expect(container.querySelector('.new-tab-none')).toHaveTextContent('No project matches');
    await fireEvent.keyDown(root, { key: 'Escape' });
    expect(scopes(container)).toHaveLength(3);
    await fireEvent.keyDown(root, { key: 't', metaKey: true });
    expect(container.querySelector('#new-tab-filter')).toBeNull();
  });
});
