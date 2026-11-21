import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import ComponentTable from '../../src/lib/progress/ComponentTable.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import type { Component } from '../../src/lib/components';
import type { Task } from '../../src/lib/types';
import { loadIntoDashboard } from './components.helpers';
import { vocabulary, withKinds } from './views';

// The Progress tab's "By component" table (T-323): a row per component, 'No component' last; a row filters by it

/** A task as atm serves it: with its kind */
const task = (id: string, fields: Partial<Task>): Task =>
  withKinds([{ id, name: id, phase: 0, type: 'feature', description: '', status: 'todo', ...fields } as Task])[0];

const TASKS = [
  task('T-1', { components: ['api', 'ui'], status: 'done' }),
  task('T-2', { components: ['ui'], status: 'in_review' }),
  task('T-3', { components: ['ui'], status: 'blocked' }),
  task('T-4', { components: ['ui'], status: 'skipped' }),
  task('T-5', { components: ['api'], status: 'skipped' }),
  task('T-6', {}),
];

/** The project with `components`, as atm serves them with its vocabulary */
function project(
  components: Component[] = [
    { id: 'ui', label: 'Dashboard', paths: [] },
    { id: 'api', label: 'api', paths: [] },
  ],
) {
  loadIntoDashboard({ meta: {}, tasks: TASKS }, undefined, vocabulary({ components }));
}

function show(tasks: readonly Task[] = TASKS) {
  const { container } = render(ComponentTable, { tasks, scope: 'Sprint 4' });
  const row = (id: string) => container.querySelector<HTMLElement>(`tr[data-component="${id}"]`)!;
  return { container, row };
}

const cells = (row: HTMLElement) =>
  [...row.querySelectorAll('td')].map((td) => td.textContent?.replace(/\s+/g, ' ').trim());

beforeEach(() => project());

describe('ComponentTable', () => {
  it('is not shown for a project without components', () => {
    project([]);
    expect(show().container.querySelector('#component-progress')).toBeNull();
  });

  it('names the scope it counts', () => {
    expect(show().container.querySelector('.burndown-focus')).toHaveTextContent('Sprint 4');
  });

  it('a row per component in the project’s order, the tasks in none last', () => {
    const { container } = show();
    expect([...container.querySelectorAll('tbody tr')].map((tr) => tr.getAttribute('data-component'))).toEqual([
      'ui',
      'api',
      '',
    ]);
  });

  it('each row’s done of the not-skipped, the skipped, what remains and what is blocked or in review', () => {
    const { row } = show();
    const notes = (id: string) => [...row(id).querySelectorAll('.phase-blocked')].map((s) => s.textContent);
    expect(cells(row('ui')).slice(0, 2)).toEqual(['Dashboard', '1/3 · 1 skipped']);
    expect(row('ui').querySelector('.component-chip')).toHaveTextContent('Dashboard');
    expect(notes('ui')).toEqual(['1 blocked', '1 in review']);
    expect(cells(row('api'))).toEqual(['api', '1/1 · 1 skipped', '', '0']);
    expect(notes('api')).toEqual([]);
    expect(cells(row(''))).toEqual(['No component', '0/1', '', '1']);
    expect(row('').querySelector('.component-chip')).toBeNull();
  });

  it('the progress bar is done of the not-skipped; all skipped is complete', () => {
    const { row } = show([task('T-1', { components: ['ui'], status: 'done' }), task('T-2', { components: ['ui'] })]);
    expect(row('ui').querySelector<HTMLElement>('.progress-fill')!.style.width).toBe('50%');
    const allSkipped = show([task('T-5', { components: ['api'], status: 'skipped' })]);
    expect(allSkipped.row('api').querySelector<HTMLElement>('.progress-fill')!.style.width).toBe('100%');
    const none = show([task('T-6', { components: ['api'] })]);
    expect(none.row('api').querySelector<HTMLElement>('.progress-fill')!.style.width).toBe('0%');
  });

  it('with no tasks, only the header', () => {
    const { container } = show([]);
    expect(container.querySelector('#component-table')).not.toBeNull();
    expect(container.querySelectorAll('tbody tr')).toHaveLength(0);
  });

  it('a click on a component filters by it, and again undoes it', async () => {
    const { row } = show();
    expect(row('ui')).toHaveAttribute('aria-selected', 'false');
    await fireEvent.click(row('ui'));
    expect([...dashboard.filters.component]).toEqual(['ui']);
    expect(row('ui')).toHaveClass('selected');
    expect(row('ui')).toHaveAttribute('aria-selected', 'true');
    await fireEvent.click(row('ui'));
    expect([...dashboard.filters.component]).toEqual([]);
    expect(row('ui')).not.toHaveClass('selected');
  });

  it.each(['Enter', ' '])('the key %j filters by a component; other keys don’t', async (key) => {
    const { row } = show();
    await fireEvent.keyDown(row('api'), { key: 'a' });
    expect([...dashboard.filters.component]).toEqual([]);
    await fireEvent.keyDown(row('api'), { key });
    expect([...dashboard.filters.component]).toEqual(['api']);
  });

  it('the tasks in no component are no filter: not focusable, and a click does nothing', async () => {
    const { row } = show();
    expect(row('')).not.toHaveAttribute('tabindex');
    expect(row('')).not.toHaveAttribute('aria-selected');
    expect(row('')).not.toHaveClass('selectable');
    await fireEvent.click(row(''));
    await fireEvent.keyDown(row(''), { key: 'Enter' });
    expect([...dashboard.filters.component]).toEqual([]);
  });
});
