import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import TaskGrid from '../../src/lib/tasks/TaskGrid.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import type { TasksFile } from '../../src/lib/types';
import archive from '../fixtures/archive.json';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// The List's end: the archive one click away (T-354)
describe('TaskGrid: the archived tasks hidden', () => {
  const withArchived = (n: number) => ({ ...standard, meta: { ...standard.meta, archived: n } });
  beforeEach(() => loadIntoDashboard(standard));

  it('says how many archived tasks the List leaves out, and a click shows them', async () => {
    loadIntoDashboard(withArchived(3), archive as TasksFile);
    const { container } = render(TaskGrid);
    const note = container.querySelector('#list-archived-toggle')!;
    expect(note).toHaveTextContent('3 archived tasks hidden · show');
    await fireEvent.click(note);
    expect(dashboard.includeArchived).toBe(true);
    // Shown: nothing hidden, so no note
    expect(container.querySelector('#list-archived-toggle')).toBeNull();
  });

  it('one archived task is "task"', () => {
    loadIntoDashboard(withArchived(1), archive as TasksFile);
    const { container } = render(TaskGrid);
    expect(container.querySelector('#list-archived-toggle')).toHaveTextContent('1 archived task hidden · show');
  });

  it('no note without an archive, nor while Include archived is on', () => {
    expect(render(TaskGrid).container.querySelector('#list-archived-toggle')).toBeNull();
    loadIntoDashboard(withArchived(3), archive as TasksFile);
    dashboard.includeArchived = true;
    expect(render(TaskGrid).container.querySelector('#list-archived-toggle')).toBeNull();
  });
});
