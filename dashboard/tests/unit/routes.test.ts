import { describe, expect, it } from 'vitest';
import type { ProjectEntry } from '../../src/lib/load';
import {
  hasOverviewHash,
  isOverviewTab,
  parseOverviewTab,
  parseProjectHash,
  routeFor,
  withProjectHash,
} from '../../src/lib/routes';

const project = (id: string, missing = false): ProjectEntry => ({
  id,
  name: id,
  path: `/${id}`,
  missing,
  summary: null,
});

describe('the project in the URL', () => {
  it('is read from #project=, alongside other parameters', () => {
    expect(parseProjectHash('#project=abc&view=kanban')).toBe('abc');
    expect(parseProjectHash('#view=kanban')).toBeNull();
    expect(parseProjectHash('#project=')).toBeNull();
    expect(hasOverviewHash('#overview&about')).toBe(true);
    expect(hasOverviewHash('#project=abc')).toBe(false);
  });

  it('is set or replaced by the overview, keeping dialogs, and the view where the page has it', () => {
    expect(withProjectHash('#view=kanban', 'abc')).toBe('#view=kanban&project=abc');
    expect(withProjectHash('#project=abc&view=list&about', 'def')).toBe('#view=list&about&project=def');
    expect(withProjectHash('#overview', 'a b')).toBe('#project=a%20b');
    // A project's tab stays out of the overview, the overview's out of a project
    expect(withProjectHash('#project=abc&view=list&about', null)).toBe('#about&overview');
    expect(withProjectHash('#overview&view=board', 'abc')).toBe('#project=abc');
    expect(withProjectHash('#overview&view=projects', null)).toBe('#view=projects&overview');
    // "agents" is both (T-333): whose it is follows the page the hash names
    expect(withProjectHash('#project=abc&view=agents', null)).toBe('#overview');
    expect(withProjectHash('#overview&view=agents', 'abc')).toBe('#project=abc');
    expect(withProjectHash('#project=abc&view=agents', 'def')).toBe('#view=agents&project=def');
  });
});

describe('the overview tab in the URL', () => {
  it('is read from #view= when it is one of the overview’s', () => {
    expect(parseOverviewTab('#overview&view=board')).toBe('board');
    expect(parseOverviewTab('#overview&view=projects')).toBe('projects');
    expect(parseOverviewTab('#overview&view=kanban')).toBeNull();
    expect(parseOverviewTab('#overview')).toBeNull();
    expect(parseOverviewTab('#overview&view=agents')).toBe('agents');
    // A project's Agents tab is the project's (T-333)
    expect(parseOverviewTab('#project=abc&view=agents')).toBeNull();
    expect(isOverviewTab('board')).toBe(true);
    expect(isOverviewTab(3)).toBe(false);
  });
});

describe('routeFor', () => {
  const two = [project('a'), project('b')];

  it('shows a named project the server lists and has', () => {
    expect(routeFor(two, '#project=b&view=kanban')).toEqual({ page: 'project', id: 'b' });
  });

  it('a named project that is unknown or missing is its own page', () => {
    expect(routeFor(two, '#project=zz')).toEqual({ page: 'unknown', id: 'zz' });
    expect(routeFor([project('a', true)], '#project=a')).toEqual({ page: 'unknown', id: 'a' });
  });

  it('#overview is the overview, with any number of projects', () => {
    expect(routeFor([project('a')], '#overview')).toEqual({ page: 'overview' });
    expect(routeFor([], '#overview')).toEqual({ page: 'overview' });
  });

  it('lands on the overview with two or more projects, the project with one, how to add one with none', () => {
    expect(routeFor(two, '')).toEqual({ page: 'overview' });
    expect(routeFor([project('a'), project('gone', true)], '#view=list')).toEqual({ page: 'project', id: 'a' });
    expect(routeFor([project('gone', true)], '')).toEqual({ page: 'none' });
    expect(routeFor([], '')).toEqual({ page: 'none' });
  });
});
