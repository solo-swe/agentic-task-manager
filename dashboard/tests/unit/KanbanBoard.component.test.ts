import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import KanbanBoard from '../../src/lib/kanban/KanbanBoard.svelte';
import { ACTIVE } from '../../src/lib/filters';
import { dashboard } from '../../src/lib/state.svelte';
import type { TasksFile } from '../../src/lib/types';
import sprints from '../fixtures/sprints.json';
import { loadIntoDashboard } from './components.helpers';

// The completion ring and the options moved to the scope bar (T-323): the board keeps a toolbar only without sprints
describe('KanbanBoard toolbar', () => {
  it('a project without sprints: the board holds Show skipped, and no completion ring', () => {
    loadIntoDashboard({
      meta: { project: 'Plain' },
      tasks: [{ id: 'T-1', name: 'A', type: 'feature', status: 'todo' }],
    } as TasksFile);
    const { container } = render(KanbanBoard);
    expect(container.querySelector('#kanban-show-skipped')).not.toBeNull();
    expect(container.querySelector('#kanban-group-by-sprint')).toBeNull();
    expect(container.querySelector('#kanban-completion')).toBeNull();
  });

  it('with sprints: no toolbar, the scope bar holds the options', () => {
    loadIntoDashboard(sprints as TasksFile);
    const { container } = render(KanbanBoard);
    expect(container.querySelector('.kanban-toolbar')).toBeNull();
  });
});

describe('KanbanBoard grouping by sprint', () => {
  beforeEach(() => loadIntoDashboard(sprints as TasksFile));

  it('groups only where the board shows more than one sprint', async () => {
    dashboard.kanbanGroupBySprint = true;
    const { container } = render(KanbanBoard);
    expect(container.querySelectorAll('.kanban-lane').length).toBeGreaterThan(1);
    dashboard.setSprintScope([ACTIVE]);
    await tick();
    expect(container.querySelectorAll('.kanban-lane')).toHaveLength(0);
    dashboard.kanbanGroupBySprint = false;
  });

  it('a lane marks the active sprint ▶ and a closed one ✓', () => {
    dashboard.kanbanGroupBySprint = true;
    const { container } = render(KanbanBoard);
    const lane = (key: string) => container.querySelector(`.kanban-lane[data-sprint="${key}"]`)!;
    expect(lane('Ada Lovelace').querySelector('.sprint-marker.active')).not.toBeNull();
    expect(lane('Foundations').querySelector('.sprint-marker.closed')).not.toBeNull();
    dashboard.kanbanGroupBySprint = false;
  });
});
