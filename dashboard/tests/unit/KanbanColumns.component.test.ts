import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import KanbanColumns from '../../src/lib/kanban/KanbanColumns.svelte';
import { buildBoard } from '../../src/lib/kanban';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task } from '../../src/lib/types';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// standard.json's done tasks: T-001, T-002, T-003, T-004, T-012
const staged = (workflow?: string) => {
  const tasks = (standard.tasks as Task[]).map((t) =>
    t.id === 'T-001'
      ? { ...t, pr: { number: 4, url: 'https://example.com/pr/4' }, merged_at: '2026-04-02T00:00:00Z' }
      : t.id === 'T-004' || t.id === 'T-012'
        ? { ...t, pr: { number: 7, url: 'https://example.com/pr/7' } }
        : t,
  );
  loadIntoDashboard({ ...standard, meta: workflow ? { workflow: { type: workflow } } : {}, tasks });
  const board = buildBoard(dashboard.tasks, dashboard.tasks, { showSkipped: false, statusFilter: new Set() });
  return render(KanbanColumns, { columns: board, showAllDone: false, onshowall: () => {} }).container;
};

describe('KanbanColumns: a closed sprint’s board (T-323)', () => {
  it('foldEmpty folds the empty columns, not Done, and widens Done', () => {
    loadIntoDashboard({ ...standard, meta: {}, tasks: (standard.tasks as Task[]).filter((t) => t.status === 'done') });
    const board = buildBoard(dashboard.tasks, dashboard.tasks, { showSkipped: false, statusFilter: new Set() });
    const { container } = render(KanbanColumns, {
      columns: board,
      showAllDone: false,
      foldEmpty: true,
      onshowall: () => {},
    });
    const folded = [...container.querySelectorAll('.kanban-column.folded')].map(
      (c) => (c as HTMLElement).dataset.status,
    );
    expect(folded).toEqual(['todo', 'in_progress', 'in_review', 'blocked']);
    expect(container.querySelector('.kanban-column.folded .kanban-empty')).toBeNull();
    expect(container.querySelector('.kanban-column[data-status="done"]')).toHaveClass('wide');
  });
});

describe('KanbanColumns: stages of Done', () => {
  it('in a pull-request project, Done has sections In PR (per PR), Done and Merged, with counts', () => {
    const container = staged('pull-request');
    const done = container.querySelector('.kanban-column[data-status="done"]')!;
    const sections = [...done.querySelectorAll('.kanban-section')].map((s) => [
      s.querySelector('.kanban-section-header > :first-child')!.textContent,
      s.querySelector('.kanban-section-count')!.textContent,
      [...s.querySelectorAll('.kanban-card')].map((c) => (c as HTMLElement).dataset.id).sort(),
    ]);
    expect(sections).toEqual([
      ['In PR #7', '2', ['T-004', 'T-012']],
      ['Done', '2', ['T-002', 'T-003']],
      ['Merged', '1', ['T-001']],
    ]);
    expect(done.querySelector('.kanban-section-header a')).toHaveAttribute('href', 'https://example.com/pr/7');
  });

  it('cards show their stage whatever the workflow; without pull-request the column is not split', () => {
    const container = staged();
    expect(container.querySelector('.kanban-section')).toBeNull();
    const chip = container.querySelector('.kanban-card[data-id="T-004"] .stage-chip')!;
    expect(chip).toHaveTextContent('PR #7');
    expect(chip).toHaveAttribute('href', 'https://example.com/pr/7');
    expect(container.querySelector('.kanban-card[data-id="T-001"] .stage-chip')).toHaveTextContent('Merged');
    expect(container.querySelector('.kanban-card[data-id="T-002"] .stage-chip')).toBeNull();
  });
});
