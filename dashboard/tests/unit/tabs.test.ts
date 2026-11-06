import { describe, expect, it } from 'vitest';
import { adjacentView, DEFAULT_VIEW, namesView, viewOf, VIEWS, withView } from '../../src/lib/tabs';

describe('viewOf', () => {
  it("reads the project's tab from its address (T-429)", () => {
    expect(viewOf('/projects/p1/kanban')).toBe('kanban');
    expect(viewOf('/projects/p1/progress?about')).toBe('progress');
    expect(viewOf('/?view=list')).toBe('list');
    expect(namesView('/projects/p1/list')).toBe(true);
    expect(namesView('/projects/p1')).toBe(false);
  });

  it('falls back to the Kanban (T-251)', () => {
    expect(DEFAULT_VIEW).toBe('kanban');
    expect(viewOf('')).toBe('kanban');
    expect(viewOf('/')).toBe('kanban');
    expect(viewOf('/projects/p1')).toBe('kanban');
    expect(viewOf('/projects/p1/nope')).toBe('kanban');
    expect(viewOf('/projects/p1/graph')).toBe('graph');
  });
});

describe('withView', () => {
  it('sets the view and keeps other parameters', () => {
    expect(withView('/projects/p1', 'kanban')).toBe('/projects/p1/kanban');
    expect(withView('/projects/p1/graph', 'progress')).toBe('/projects/p1/progress');
    expect(withView('/projects/p1?about', 'kanban')).toBe('/projects/p1/kanban?about');
  });

  it('round-trips through viewOf', () => {
    for (const v of VIEWS) expect(viewOf(withView('/projects/p1?x=y', v))).toBe(v);
  });
});

describe('adjacentView', () => {
  it('moves left/right and wraps, in the tab order: Kanban, List, Graph, Progress, Agents, Project', () => {
    expect(VIEWS).toEqual(['kanban', 'list', 'graph', 'progress', 'agents', 'project']);
    expect(adjacentView('graph', 1)).toBe('progress');
    expect(adjacentView('project', 1)).toBe('kanban');
    expect(adjacentView('kanban', -1)).toBe('project');
  });
});
