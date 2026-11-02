import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import Header from '../../src/components/Header.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import type { TasksFile } from '../../src/lib/types';
import archive from '../fixtures/archive.json';
import sprints from '../fixtures/sprints.json';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

describe('Header', () => {
  beforeEach(() => loadIntoDashboard(standard));

  it('has no attention count when nothing needs the user', () => {
    const { container } = render(Header);
    expect(container.querySelector('#attention-count')).toBeNull();
  });

  it('shows how many tasks are archived, from meta.archived, without the archive loaded', () => {
    loadIntoDashboard({ ...standard, meta: { ...standard.meta, archived: 600 } });
    const { container } = render(Header);
    // Its phases read as sprints (not migrated)
    expect(container.querySelector('#subtitle')).toHaveTextContent('12 tasks across 3 sprints · 600 archived —');
    expect(dashboard.archiveLoaded).toBe(false);
  });

  it('the archived count is a toggle for Include archived (T-354)', async () => {
    loadIntoDashboard({ ...standard, meta: { ...standard.meta, archived: 3 } }, archive as TasksFile);
    const { container } = render(Header);
    const toggle = container.querySelector<HTMLButtonElement>('#archived-count')!;
    expect(toggle.tagName).toBe('BUTTON');
    expect(toggle).toHaveTextContent('3 archived');
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(toggle);
    expect(dashboard.includeArchived).toBe(true);
    expect(toggle).toHaveTextContent('showing 3 archived');
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    expect(toggle.title).toBe('Hide the archived tasks again');
    await fireEvent.click(toggle);
    expect(dashboard.includeArchived).toBe(false);
    expect(toggle).toHaveTextContent(/^3 archived$/);
  });

  it('without an archive, no toggle: the subtitle is one text, as before the archive existed', () => {
    const { container } = render(Header);
    expect(container.querySelector('#archived-count')).toBeNull();
    expect(container.querySelector('#subtitle')).toHaveTextContent(
      '12 tasks across 3 sprints — loaded from tasks.json',
    );
  });

  it('counts the sprints and the backlog in a migrated project (SR-7.6)', () => {
    loadIntoDashboard(sprints as TasksFile);
    const { container } = render(Header);
    expect(container.querySelector('#subtitle')).toHaveTextContent('5 tasks across 2 sprints · 1 in the backlog');
  });

  it('counts tasks in review and tasks blocked on the user, and lists them', async () => {
    const tasks = standard.tasks.map((t) =>
      t.id === 'T-005'
        ? { ...t, status: 'in_review' }
        : t.id === 'T-008'
          ? { ...t, status: 'blocked', waiting_on: 'user', questions: ['Which DB?'] }
          : t,
    );
    loadIntoDashboard({ ...standard, tasks });
    const { container } = render(Header);
    const chip = container.querySelector('#attention-count')!;
    expect(chip).toHaveTextContent('2 need you');
    await fireEvent.click(chip);
    expect([...container.querySelectorAll('.needs-you-row')].map((r) => r.getAttribute('data-id'))).toEqual([
      'T-005',
      'T-008',
    ]);
    expect(dashboard.detailOpen).toBe(false);
  });
});
