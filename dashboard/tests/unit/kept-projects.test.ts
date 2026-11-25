import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { KEPT_PROJECTS_KEY, keepProjects, keptProjects } from '../../src/lib/kept-projects';

/** A project as atm's api/projects lists it */
const listed = (id: string, extra: Record<string, unknown> = {}) => ({
  id,
  name: `Project ${id}`,
  path: `/work/${id}`,
  missing: false,
  summary: { key: id.toUpperCase() },
  worktrees: [],
  logo: `p/${id}/logo`,
  ...extra,
});

describe('kept projects (ATM-776)', () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  it('nothing kept: none', () => {
    expect(keptProjects('default')).toBeNull();
  });

  it('kept: read back through the fresh answer’s rule, with its tag', () => {
    keepProjects('default', [listed('a'), listed('b')], { etag: '"t1"' });
    const kept = keptProjects('default')!;
    expect(kept.validators).toEqual({ etag: '"t1"' });
    expect(kept.projects.map((p) => [p.id, p.name, p.logo, p.summary?.key])).toEqual([
      ['a', 'Project a', '/p/a/logo', 'A'],
      ['b', 'Project b', '/p/b/logo', 'B'],
    ]);
  });

  it('an empty list is kept as one: a workspace with no projects', () => {
    keepProjects('default', [], {});
    expect(keptProjects('default')).toEqual({ projects: [], validators: {} });
  });

  it('each workspace its own; keeping one leaves the others', () => {
    keepProjects('default', [listed('a')], { etag: '"d"' });
    keepProjects('work', [listed('w')], { etag: '"w"' });
    keepProjects('default', [listed('a2')], { etag: '"d2"' });
    expect(keptProjects('default')!.projects.map((p) => p.id)).toEqual(['a2']);
    expect(keptProjects('work')!.projects.map((p) => p.id)).toEqual(['w']);
    expect(keptProjects('other')).toBeNull();
  });

  it('what a fresh answer would refuse, refused when read back: no ID, another site’s logo', () => {
    keepProjects('default', [listed(''), { name: 'no id' }, listed('a', { logo: 'https://elsewhere/logo' })], {});
    expect(keptProjects('default')!.projects.map((p) => [p.id, p.logo])).toEqual([['a', null]]);
  });

  it.each([
    ['not JSON', '{ not json'],
    ['not an object', '[1, 2]'],
    ['the workspace not an object', JSON.stringify({ default: 'x' })],
    ['its list not a list', JSON.stringify({ default: { listed: { id: 'a' } } })],
  ])('stored %s: none', (_, raw) => {
    localStorage.setItem(KEPT_PROJECTS_KEY, raw);
    expect(keptProjects('default')).toBeNull();
  });

  it('a tag that isn’t text is no tag', () => {
    localStorage.setItem(KEPT_PROJECTS_KEY, JSON.stringify({ default: { listed: [], etag: 7, lastModified: '' } }));
    expect(keptProjects('default')!.validators).toEqual({});
  });

  it('storage that throws: nothing kept, nothing thrown', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('full');
    });
    expect(() => keepProjects('default', [listed('a')], {})).not.toThrow();
    expect(keptProjects('default')).toBeNull();
  });
});
