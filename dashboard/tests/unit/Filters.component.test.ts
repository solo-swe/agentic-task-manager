import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import Filters from '../../src/components/Filters.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

const button = (container: HTMLElement, group: string, value: string) =>
  container.querySelector(`.filter-btn[data-filter="${group}"][data-value="${value}"]`) as HTMLElement;

async function openPanel(container: HTMLElement) {
  await fireEvent.click(container.querySelector('#filters-toggle')!);
}

describe('Filters', () => {
  beforeEach(() => loadIntoDashboard(standard));

  it('renders nothing until tasks are loaded', () => {
    dashboard.loaded = false;
    const { container } = render(Filters);
    expect(container.querySelector('#filters')).toBeEmptyDOMElement();
  });

  it('a search box and a Filters button; the popup opens on it (T-323)', async () => {
    const { container } = render(Filters);
    const toggle = container.querySelector('#filters-toggle')!;
    expect(container.querySelector('#filter-panel')).not.toBeVisible();
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(container.querySelector('#active-filters')).toHaveTextContent('No filters');
    expect(container.querySelector('#match-count')).toHaveTextContent('12 of 12 tasks');

    await openPanel(container);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect([...container.querySelectorAll('.filter-group')].map((g) => (g as HTMLElement).dataset.group)).toEqual([
      'sprint',
      'type',
      'status',
      'priority',
    ]);
  });

  it('renders a button per option for each group, with counts; no "All" until something is selected', async () => {
    const { container } = render(Filters);
    await openPanel(container);
    const values = (group: string) =>
      [...container.querySelectorAll(`.filter-btn[data-filter="${group}"]`)].map(
        (b) => (b as HTMLElement).dataset.value,
      );
    // Phases read as sprints: the open ones, then Phase 0 (all done: closed)
    expect(values('sprint')).toEqual(['Phase 1', 'Phase 2', 'Phase 0']);
    expect(values('status')).toEqual(['todo', 'in_progress', 'in_review', 'blocked', 'done', 'skipped']);
    // No task is in review: offered with a count of 0, and can be selected
    expect(button(container, 'status', 'in_review')).toHaveAttribute('aria-label', 'In Review, 0 tasks');
    expect(button(container, 'status', 'in_review')).toBeEnabled();
    expect(container.querySelectorAll('.filter-btn.active')).toHaveLength(0);
    expect(button(container, 'type', 'feature')).toHaveAttribute('aria-label', 'Feature, 3 tasks');
    // standard.json has no priorities: every task has the default
    expect(values('priority')).toEqual(['P2']);
    expect(button(container, 'priority', 'P2')).toHaveAttribute('aria-label', 'P2 Medium, 12 tasks');
  });

  it('toggles filters, shows them as chips, and clears them from the chip', async () => {
    const { container } = render(Filters);
    await openPanel(container);
    await fireEvent.click(button(container, 'type', 'feature'));
    expect([...dashboard.filters.type]).toEqual(['feature']);
    expect(button(container, 'type', 'feature')).toHaveClass('active');
    expect(button(container, 'type', 'all')).toHaveTextContent('clear');
    expect(dashboard.filtered.map((t) => t.id)).toEqual(['T-004', 'T-005', 'T-011']);
    expect(container.querySelector('.filter-chip[data-filter="type"]')).toHaveTextContent('Type: Feature');
    expect(container.querySelector('#match-count')).toHaveTextContent('3 of 12 tasks');

    await fireEvent.click(container.querySelector('.filter-chip[data-filter="type"] .chip-clear')!);
    expect(dashboard.filters.type.size).toBe(0);
    expect(container.querySelector('.filter-chip')).toBeNull();
  });

  it('"clear" clears its group, and goes', async () => {
    const { container } = render(Filters);
    await openPanel(container);
    await fireEvent.click(button(container, 'type', 'feature'));
    expect(button(container, 'type', 'all')).toHaveAttribute('aria-label', 'All types, 12 tasks');
    await fireEvent.click(button(container, 'type', 'all'));
    expect(dashboard.filters.type.size).toBe(0);
    expect(button(container, 'type', 'all')).toBeNull();
  });

  it('a long group shows its most used values first and the rest behind "+N"; labels can be found by name', async () => {
    const labels = Array.from({ length: 14 }, (_, i) => `l${String(i).padStart(2, '0')}`);
    const tasks = standard.tasks.map((t, i) => ({ ...t, labels: i === 0 ? labels : [labels[13]] }));
    loadIntoDashboard({ ...standard, tasks });
    const { container } = render(Filters);
    await openPanel(container);
    const shown = () =>
      [...container.querySelectorAll('.filter-btn[data-filter="label"]')].map((b) => (b as HTMLElement).dataset.value);
    expect(shown()).toHaveLength(10);
    expect(shown()[0]).toBe('l13'); // on every task: the most used
    const more = container.querySelector('.filter-more[data-more="label"]')!;
    expect(more).toHaveTextContent('+4');
    await fireEvent.click(more);
    expect(shown()).toHaveLength(14);
    await fireEvent.input(container.querySelector('#filter-find-label')!, { target: { value: 'l0' } });
    expect(shown()).toEqual(['l00', 'l01', 'l02', 'l03', 'l04', 'l05', 'l06', 'l07', 'l08', 'l09']);
  });

  it('shows a Labels group, with counts, once some task has a label', async () => {
    const labelled = standard.tasks.map((t, i) => (i < 3 ? { ...t, labels: i === 0 ? ['ux', 'api'] : ['ux'] } : t));
    loadIntoDashboard({ ...standard, tasks: labelled });
    const { container } = render(Filters);
    await openPanel(container);
    expect(container.querySelector('.filter-group[data-group="label"] .filter-group-label')).toHaveTextContent(
      'Labels',
    );
    expect(button(container, 'label', 'ux')).toHaveAttribute('aria-label', 'ux, 3 tasks');
    await fireEvent.click(button(container, 'label', 'api'));
    expect(dashboard.filtered.map((t) => t.id)).toEqual(['T-001']);
    expect(container.querySelector('.filter-chip[data-filter="label"]')).toHaveTextContent('Labels: api');
  });

  it('shows a Started by group only when more than one actor started tasks', async () => {
    const one = standard.tasks.map((t, i) => (i < 2 ? { ...t, started_by: 'claude:aaaa' } : t));
    loadIntoDashboard({ ...standard, tasks: one });
    const first = render(Filters);
    await openPanel(first.container);
    expect(first.container.querySelector('.filter-group[data-group="agent"]')).toBeNull();
    first.unmount();

    const two = one.map((t, i) => (i === 2 ? { ...t, started_by: 'Hakan' } : t));
    loadIntoDashboard({ ...standard, tasks: two });
    const { container } = render(Filters);
    await openPanel(container);
    expect(container.querySelector('.filter-group[data-group="agent"]')).toHaveTextContent('Started by');
    expect(button(container, 'agent', 'claude:aaaa')).toHaveAttribute('aria-label', 'claude:aaaa, 2 tasks');
  });

  it('the search narrows the tasks and the filter counts, and can be cleared', async () => {
    const { container } = render(Filters);
    const input = container.querySelector('#task-search') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: 'T-004' } });
    expect(dashboard.query).toBe('T-004');
    expect(dashboard.filtered.map((t) => t.id)).toEqual(['T-004']);
    expect(container.querySelector('#match-count')).toHaveTextContent('1 of 12 tasks');
    await openPanel(container);
    expect(button(container, 'type', 'feature')).toHaveAttribute('aria-label', 'Feature, 1 task');

    await fireEvent.click(container.querySelector('.search-clear')!);
    expect(dashboard.query).toBe('');
    expect(container.querySelector('.search-clear')).toBeNull();
  });
});
