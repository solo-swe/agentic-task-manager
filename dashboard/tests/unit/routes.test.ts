import { describe, expect, it } from 'vitest';
import type { ProjectEntry } from '../../src/lib/load';
import {
  namesOverview,
  isOverviewTab,
  overviewTabOf,
  projectOf,
  routeFor,
  taskOf,
  withProject,
  withTask,
} from '../../src/lib/routes';

const project = (id: string, missing = false): ProjectEntry => ({
  id,
  name: id,
  path: `/${id}`,
  missing,
  summary: null,
});

describe('the project in the address (T-429)', () => {
  it('is read from /projects/<id>, alongside other parameters', () => {
    expect(projectOf('/projects/abc/kanban?about')).toBe('abc');
    expect(projectOf('/projects/a%20b')).toBe('a b');
    expect(projectOf('/projects')).toBeNull();
    expect(projectOf('/?view=kanban')).toBeNull();
    expect(namesOverview('/projects?about')).toBe(true);
    expect(namesOverview('/board')).toBe(true);
    expect(namesOverview('/projects/abc')).toBe(false);
    expect(namesOverview('/')).toBe(false);
  });

  it('is set or replaced by the overview, keeping dialogs, and the view where the page has it', () => {
    expect(withProject('/?view=kanban', 'abc')).toBe('/projects/abc/kanban');
    expect(withProject('/projects/abc/list?about', 'def')).toBe('/projects/def/list?about');
    expect(withProject('/projects', 'a b')).toBe('/projects/a%20b');
    // A project's tab stays out of the overview, the overview's out of a project
    expect(withProject('/projects/abc/list?about', null)).toBe('/projects?about');
    expect(withProject('/board', 'abc')).toBe('/projects/abc');
    expect(withProject('/projects', null)).toBe('/projects');
    // The overview opens on the tab it was last on
    expect(withProject('/projects/abc/list', null, 'board')).toBe('/board');
    // "agents" is both (T-333): whose it is follows the page the address names
    expect(withProject('/projects/abc/agents', null)).toBe('/projects');
    expect(withProject('/agents', 'abc')).toBe('/projects/abc');
    expect(withProject('/projects/abc/agents', 'def')).toBe('/projects/def/agents');
  });

  it("a task's details are its project's: another project's page leaves them out", () => {
    expect(withProject('/projects/abc/list?task=T-1', 'def')).toBe('/projects/def/list');
    expect(withProject('/projects/abc?task=T-1', null)).toBe('/projects');
    expect(withProject('/projects/abc?task=T-1', 'abc')).toBe('/projects/abc?task=T-1');
  });
});

describe('the task in the address (T-429)', () => {
  it('is read from ?task=, and set or removed keeping the rest', () => {
    expect(taskOf('/projects/abc/list?task=T-12&about')).toBe('T-12');
    expect(taskOf('/projects/abc?task=')).toBeNull();
    expect(withTask('/projects/abc/list?sprint=active', 'T-3')).toBe('/projects/abc/list?sprint=active&task=T-3');
    expect(withTask('/projects/abc/list?task=T-3&about', 'T-4')).toBe('/projects/abc/list?about&task=T-4');
    expect(withTask('/projects/abc?task=T-3', null)).toBe('/projects/abc');
  });
});

describe('the overview tab in the address', () => {
  it('is its path, when it is one of the overview’s', () => {
    expect(overviewTabOf('/board')).toBe('board');
    expect(overviewTabOf('/projects')).toBe('projects');
    expect(overviewTabOf('/progress?about')).toBe('progress');
    expect(overviewTabOf('/agents')).toBe('agents');
    expect(overviewTabOf('/')).toBeNull();
    // A project's Agents tab is the project's (T-333)
    expect(overviewTabOf('/projects/abc/agents')).toBeNull();
    expect(isOverviewTab('board')).toBe(true);
    expect(isOverviewTab(3)).toBe(false);
  });
});

describe('routeFor', () => {
  const two = [project('a'), project('b')];

  it('shows a named project the server lists and has', () => {
    expect(routeFor(two, '/projects/b/kanban')).toEqual({ page: 'project', id: 'b' });
  });

  it('a named project that is unknown or missing is its own page', () => {
    expect(routeFor(two, '/projects/zz')).toEqual({ page: 'unknown', id: 'zz' });
    expect(routeFor([project('a', true)], '/projects/a')).toEqual({ page: 'unknown', id: 'a' });
  });

  it('/projects and the other overview tabs are the overview, with any number of projects', () => {
    expect(routeFor([project('a')], '/projects')).toEqual({ page: 'overview' });
    expect(routeFor([project('a')], '/board')).toEqual({ page: 'overview' });
    expect(routeFor([], '/projects')).toEqual({ page: 'overview' });
  });

  it('lands on the overview with two or more projects, the project with one, how to add one with none', () => {
    expect(routeFor(two, '/')).toEqual({ page: 'overview' });
    expect(routeFor([project('a'), project('gone', true)], '/?view=list')).toEqual({ page: 'project', id: 'a' });
    expect(routeFor([project('gone', true)], '/')).toEqual({ page: 'none' });
    expect(routeFor([], '/')).toEqual({ page: 'none' });
  });
});
