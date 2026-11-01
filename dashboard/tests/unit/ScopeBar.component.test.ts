import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import ScopeBar from '../../src/components/ScopeBar.svelte';
import { ACTIVE, BACKLOG } from '../../src/lib/filters';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task, TasksFile } from '../../src/lib/types';
import sprintsFile from '../fixtures/sprints.json';
import { loadIntoDashboard } from './components.helpers';

// fixtures/sprints.json: "Ada Lovelace" is active (due 2026-04-12, T-002–T-004), "Foundations" closed (T-001),
// "Alan Turing" planned (no tasks); T-005 is in the backlog
const NOW = Date.parse('2026-04-09T12:00:00Z');
const file = sprintsFile as TasksFile;

function load(edit: (t: Task) => Partial<Task> = () => ({}), meta = file.meta) {
  loadIntoDashboard({ meta, tasks: (file.tasks as Task[]).map((t) => ({ ...t, ...edit(t) })) } as TasksFile);
  dashboard.loadedAt = NOW;
  dashboard.tab = 'kanban';
}

const text = (c: HTMLElement, selector: string) => c.querySelector(selector)?.textContent?.replace(/\s+/g, ' ').trim();

describe('ScopeBar (T-323)', () => {
  beforeEach(() => load());

  it('the active sprint: its goal, due date, release and progress, marked ▶', async () => {
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(container.querySelector('#scope-bar')?.getAttribute('data-state')).toBe('active');
    expect(text(container, '#scope-trigger')).toBe('Ada Lovelace');
    expect(container.querySelector('#scope-trigger .sprint-marker.active')).not.toBeNull();
    expect(text(container, '.sprint-goal')).toBe('Sign-in, end to end');
    expect(text(container, '.sprint-due')).toBe('due in 3 days');
    expect(text(container, '.sprint-release')).toBe('release 1.0');
    expect(text(container, '.sprint-progress')).toBe('0 of 3 done');
    expect(text(container, '.sprint-added')).toBe('1 added mid-sprint');
    expect(text(container, '.sprint-carried')).toBe('1 carried in');
    expect(container.querySelector('#scope-filtered')).toBeNull();
  });

  it('narrowed by other filters or the search: "showing N, filtered"; ✕ clears them and keeps the sprint', async () => {
    dashboard.setSprintScope([ACTIVE]);
    dashboard.toggleFilter('status', 'todo');
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '.sprint-progress')).toBe('0 of 3 done');
    expect(text(container, '#scope-filtered')).toBe('showing 2, filtered');
    await fireEvent.click(container.querySelector('#scope-filtered button')!);
    expect(dashboard.filters.status.size).toBe(0);
    expect([...dashboard.filters.sprint]).toEqual([ACTIVE]);
    expect(container.querySelector('#scope-filtered')).toBeNull();
  });

  it('the active sprint all done: close it, the release, more work from the backlog', async () => {
    load((t) => (t.sprint?.toLowerCase() === 'ada lovelace' ? { status: 'done' } : {}));
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-done')).toBe('All 3 tasks done');
    expect(text(container, '#scope-hint')).toBe('Close it with /atm:close-sprint · release 1.0 is ready to ship');
    expect(container.querySelector('.scope-facts')?.textContent).toContain('Pull from the backlog (1 task)');
    expect(container.querySelector('.sprint-progress')).toBeNull();
  });

  it('the active sprint with only reviews or blocks left: settle them before closing', async () => {
    load((t) =>
      t.id === 'T-002' ? { status: 'in_review' } : t.sprint?.toLowerCase() === 'ada lovelace' ? { status: 'done' } : {},
    );
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-hint')).toBe('1 in review: settle it before closing');
    expect(text(container, '.sprint-progress')).toBe('2 of 3 done');
  });

  it('an active sprint with no tasks: pull some from the backlog', async () => {
    load((t) => (t.sprint?.toLowerCase() === 'ada lovelace' ? { sprint: undefined } : {}));
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-hint')).toBe(
      'No tasks yet · pull some from the backlog (4 tasks) with /atm:plan-sprint',
    );
  });

  it('a planned sprint: its size, and how to start it', async () => {
    dashboard.setSprintScope(['Alan Turing']);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '.sprint-state')).toBe('planned');
    expect(text(container, '.sprint-size')).toBe('0 tasks');
    expect(text(container, '#scope-hint')).toBe('start it with /atm:plan-sprint');
  });

  it('a closed sprint: marked ✓, its dates, progress and where its unfinished tasks went', async () => {
    dashboard.setSprintScope(['Foundations']);
    const { container } = render(ScopeBar);
    await tick();
    expect(container.querySelector('#scope-trigger .sprint-marker.closed')).not.toBeNull();
    expect(text(container, '.sprint-state')).toBe('closed');
    expect(text(container, '.sprint-dates')).toMatch(/^Apr 1 – Apr 5$/);
    expect(text(container, '.sprint-progress')).toBe('1 of 1 done');
    expect(text(container, '.sprint-carried')).toBe('1 carried to Ada Lovelace');
    expect(container.querySelector('.sprint-due')).toBeNull();
  });

  it('the backlog, and several sprints', async () => {
    dashboard.setSprintScope([BACKLOG]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-trigger')).toBe('Backlog');
    expect(text(container, '.sprint-size')).toBe('1 task in no sprint');
    dashboard.setSprintScope([ACTIVE, 'Foundations']);
    await tick();
    expect(text(container, '#scope-trigger')).toBe('Filtered: 2 selected');
    expect(text(container, '.sprint-goal')).toBe('Foundations, Ada Lovelace');
    expect(text(container, '.sprint-progress')).toBe('1 of 4 done');
    expect(container.querySelector('#kanban-group-by-sprint')).not.toBeNull();
  });

  it('all tasks while a sprint is active: a link back to it', async () => {
    dashboard.setSprintScope([]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-trigger')).toBe('All tasks');
    expect(text(container, '#scope-hint')).toBe('Ada Lovelace is active');
    await fireEvent.click(container.querySelector('#scope-show-active')!);
    expect([...dashboard.filters.sprint]).toEqual([ACTIVE]);
  });

  it('no active sprint: the last closed and the next planned, and how to start it; ACTIVE selects nothing', async () => {
    const meta = {
      ...file.meta,
      sprints: (file.meta!.sprints as { id: string; state: string }[]).map((s) =>
        s.id === 'Ada Lovelace' ? { ...s, state: 'closed' } : s,
      ),
    };
    load(undefined, meta);
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(text(container, '#scope-trigger')).toBe('All tasks');
    expect(text(container, '#scope-hint')).toMatch(
      /^No active sprint · last: Foundations, closed .+ · next planned: Alan Turing \(0 tasks\) · start it with \/atm:plan-sprint$/,
    );
  });

  it('the menu sets the Sprint filter to one choice', async () => {
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    await fireEvent.click(container.querySelector('#scope-trigger')!);
    const choices = [...container.querySelectorAll('.scope-choice')].map((c) => c.getAttribute('data-scope'));
    expect(choices).toEqual([ACTIVE, 'Alan Turing', BACKLOG, 'all', 'Foundations']);
    expect(container.querySelector(`.scope-choice[aria-checked="true"]`)?.getAttribute('data-scope')).toBe(ACTIVE);
    await fireEvent.click(container.querySelector(`.scope-choice[data-scope="${BACKLOG}"]`)!);
    expect([...dashboard.filters.sprint]).toEqual([BACKLOG]);
    expect(container.querySelector('#scope-menu')).toBeNull();
  });

  it('the Kanban options show on the Kanban only; Group by sprint only with more than one sprint', async () => {
    dashboard.setSprintScope([ACTIVE]);
    const { container } = render(ScopeBar);
    await tick();
    expect(container.querySelector('#kanban-show-skipped')).not.toBeNull();
    expect(container.querySelector('#kanban-group-by-sprint')).toBeNull();
    dashboard.tab = 'list';
    await tick();
    expect(container.querySelector('#kanban-show-skipped')).toBeNull();
  });

  it('a project without sprints has no scope bar', () => {
    loadIntoDashboard({ meta: { project: 'Plain' }, tasks: [] } as unknown as TasksFile);
    const { container } = render(ScopeBar);
    expect(container.querySelector('#scope-bar')).toBeNull();
  });
});
