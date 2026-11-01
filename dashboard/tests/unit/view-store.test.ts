import { beforeEach, describe, expect, it } from 'vitest';
import { PREFS_KEY, setPref } from '../../src/lib/prefs';
import { sprintModel } from '../../src/lib/sprints';
import type { Task } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import {
  clearView,
  defaultView,
  projectKey,
  readView,
  sanitizeView,
  writeView,
  type SavedView,
} from '../../src/lib/view-store';

const task = (phase: number, type: string, status: string) =>
  ({ id: `T-${phase}${type}`, phase, type, status }) as Task;

describe('projectKey', () => {
  it('uses meta.id, else the project name, else a shared default', () => {
    expect(projectKey({ id: 'a1b2c3d4', project: 'Shop' })).toBe('atm-dashboard:view:id:a1b2c3d4');
    expect(projectKey({ project: 'Shop' })).toBe('atm-dashboard:view:project:Shop');
    expect(projectKey({ id: '', project: '' })).toBe('atm-dashboard:view:default');
    expect(projectKey({ id: 7 })).toBe('atm-dashboard:view:default');
    expect(projectKey(undefined)).toBe('atm-dashboard:view:default');
  });
});

describe('reading and writing', () => {
  beforeEach(() => localStorage.clear());
  const key = projectKey({ id: 'p1' });

  it('round-trips a view', () => {
    const view: SavedView = {
      filters: {
        sprint: ['Phase 1', ':backlog'],
        release: ['1.0'],
        component: ['api'],
        type: ['feature', 'test'],
        status: [],
        priority: ['P0'],
        stage: ['in_pr'],
        label: ['ux'],
        agent: ['claude:a'],
      },
      version: 2,
      query: 'login',
      tab: 'agents',
      kanban: { groupBySprint: true, showSkipped: true, showAllDone: true },
      listSort: 'priority',
      listGroupBy: 'component',
      listShowClosed: true,
      progressSprint: 'Phase 2',
      filterClosedSprints: true,
      progressRange: '14d',
      graphFiltered: 'dim',
      graphDimDone: true,
      graph: { k: 1.5, x: -120, y: 40 },
      agents: { start: 1000, end: 61_000, followEnd: true },
      agentsLive: true,
      activity: { preset: 'custom', from: 1000, to: 5000 },
    };
    writeView(key, view);
    expect(readView(key)).toEqual(view);
  });

  it('an old view\'s "Active phases only" is dropped quietly; the burndown range defaults to the sprint (T-240, T-243)', () => {
    localStorage.setItem(key, JSON.stringify({ progressActive: { only: false }, progressRange: 'decade' }));
    const view = readView(key);
    expect(view).not.toHaveProperty('progressActive');
    expect(view.progressRange).toBe('sprint');
  });

  it('a view saved before sprints: its phase filter and focused phase are dropped, "finished phases" means closed sprints, the phase range is the sprint range (SR-7.1, SR-7.4)', () => {
    localStorage.setItem(
      key,
      JSON.stringify({
        filters: { phase: ['1'], type: ['test'] },
        filterFinishedPhases: true,
        progressPhase: 2,
        progressRange: 'phase',
      }),
    );
    const view = readView(key);
    expect(view.filters).not.toHaveProperty('phase');
    expect(view.filters.type).toEqual(['test']);
    expect(view.filterClosedSprints).toBe(true);
    expect(view).not.toHaveProperty('progressPhase');
    expect(view.progressSprint).toBeNull();
    expect(view.progressRange).toBe('sprint');
  });

  it('opens on the active sprint: by default, and for a view saved before version 2 without a Sprint filter (T-323)', () => {
    expect(defaultView().filters.sprint).toEqual([':active']);
    localStorage.setItem(key, JSON.stringify({ filters: { sprint: [], type: ['test'] } }));
    expect(readView(key).filters.sprint).toEqual([':active']);
    localStorage.setItem(key, JSON.stringify({ filters: { sprint: ['Phase 1'] } }));
    expect(readView(key).filters.sprint).toEqual(['Phase 1']);
    // Since version 2, no Sprint filter is a choice: all tasks
    localStorage.setItem(key, JSON.stringify({ version: 2, filters: { sprint: [] } }));
    expect(readView(key).filters.sprint).toEqual([]);
  });

  it('a view saved before sprints keeps "group by phase" as "group by sprint" (SR-7.3)', () => {
    localStorage.setItem(key, JSON.stringify({ kanban: { groupByPhase: true } }));
    expect(readView(key).kanban.groupBySprint).toBe(true);
  });

  it('projects are kept apart', () => {
    writeView(key, { ...defaultView(), tab: 'progress' });
    expect(readView(projectKey({ id: 'p2' })).tab).toBe('kanban');
  });

  it('nothing stored, or junk, gives the defaults', () => {
    expect(readView(key)).toEqual(defaultView());
    localStorage.setItem(key, '{not json');
    expect(readView(key)).toEqual(defaultView());
    localStorage.setItem(
      key,
      JSON.stringify({ tab: 'nope', filters: { sprint: [1, '2'], type: 'x' }, kanban: 3, progressSprint: 1 }),
    );
    expect(readView(key)).toEqual({
      ...defaultView(),
      filters: { ...defaultView().filters, sprint: ['2'] },
    });
  });

  it('a malformed graph position is ignored', () => {
    for (const graph of [{ k: 0, x: 0, y: 0 }, { k: 1, x: 'a', y: 0 }, { k: 1, x: 0 }, 'x']) {
      localStorage.setItem(key, JSON.stringify({ graph }));
      expect(readView(key).graph).toBeNull();
    }
  });

  it('a malformed agents time range is ignored', () => {
    for (const agents of [{ start: 5, end: 5 }, { start: 'a', end: 9 }, { start: 1 }, 'x']) {
      localStorage.setItem(key, JSON.stringify({ agents }));
      expect(readView(key).agents).toBeNull();
    }
  });

  it('clearView forgets the project', () => {
    writeView(key, { ...defaultView(), tab: 'graph' });
    clearView(key);
    expect(localStorage.getItem(key)).toBeNull();
    expect(readView(key).tab).toBe('kanban');
  });

  it('the old per-browser "group by phase" choice seeds projects that have no saved view yet', () => {
    setPref('kanbanGroupByPhase', true);
    expect(readView(key).kanban.groupBySprint).toBe(true);
    writeView(key, { ...defaultView(), kanban: { groupBySprint: false, showSkipped: false, showAllDone: false } });
    expect(readView(key).kanban.groupBySprint).toBe(false);
    expect(JSON.parse(localStorage.getItem(PREFS_KEY)!).kanbanGroupByPhase).toBe(true);
  });
});

describe('sanitizeView', () => {
  const tasks = [
    { ...task(0, 'feature', 'done'), labels: ['ux'], started_by: 'claude:a', release: '1.0', components: ['api'] },
    task(1, 'test', 'todo'),
  ];
  const vocab = resolveVocabulary({});
  const model = sprintModel({}, tasks, vocab);

  it('drops filter values and a focused sprint the tasks no longer have', () => {
    const view: SavedView = {
      ...defaultView(),
      filters: {
        sprint: ['Phase 0', 'Phase 5'],
        release: ['1.0', '0.9'],
        component: ['api', 'gone'],
        type: ['feature', 'docs'],
        status: ['todo', 'blocked'],
        priority: ['P2', 'P0'],
        stage: ['done', 'merged'],
        label: ['ux', 'gone'],
        agent: ['claude:a', 'claude:gone'],
      },
      progressSprint: 'Phase 5',
    };
    // Neither task has a priority: both are the default, P2
    expect(sanitizeView(view, tasks, vocab, model)).toEqual({
      ...view,
      filters: {
        sprint: ['Phase 0'],
        release: ['1.0'],
        component: ['api'],
        type: ['feature'],
        status: ['todo'],
        priority: ['P2'],
        stage: ['done'],
        label: ['ux'],
        agent: ['claude:a'],
      },
      progressSprint: null,
    });
  });

  it('keeps everything that still exists', () => {
    const view: SavedView = {
      ...defaultView(),
      filters: { ...defaultView().filters, sprint: ['Phase 1'] },
      progressSprint: 'Phase 0',
    };
    expect(sanitizeView(view, tasks, vocab, model)).toEqual(view);
  });

  it('keeps "the active sprint" while none is active: it applies again once one starts (T-323)', () => {
    expect(sanitizeView(defaultView(), tasks, vocab, model).filters.sprint).toEqual([':active']);
  });
});
