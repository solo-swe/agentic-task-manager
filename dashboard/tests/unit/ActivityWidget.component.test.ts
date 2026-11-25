import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ActivityWidget from '../../src/lib/progress/ActivityWidget.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';
import { loadIntoDashboard } from './components.helpers';
import { withKinds } from './views';

// The Activity pill (lib/progress/ActivityWidget.svelte, ATM-739): it charts the task file's completions alone, so
// it is the same chart whether a view has loaded the archive or not; the deep dive counts the archive (D-5.2)

const NOW = Date.parse('2026-04-01T12:00:00Z');
const done = (id: string, at: string): Task => ({
  id,
  name: id,
  phase: 1,
  description: '',
  type: 'feature',
  status: 'done',
  started_at: at,
  completed_at: at.replace(':00:00', ':30:00'),
});
const live = [done('T-1', '2026-04-01T08:00:00Z'), done('T-2', '2026-04-01T10:00:00Z')];
const archived = [done('A-1', '2026-03-20T09:00:00Z'), done('A-2', '2026-03-21T09:00:00Z')];

/** The pill as drawn: its label, its tooltip, each bar and the line */
function pill(container: HTMLElement) {
  const card = container.querySelector('#activity-card')!;
  return {
    label: card.getAttribute('aria-label'),
    title: card.getAttribute('title'),
    marks: [...card.querySelectorAll('rect, path')].map((m) =>
      ['x', 'y', 'width', 'height', 'd'].map((a) => m.getAttribute(a)).join(' '),
    ),
  };
}

describe('the Activity pill', () => {
  it('a project’s: the same with the archive loaded or not; it says the archive is left out', () => {
    loadIntoDashboard({ meta: { archived: 2 }, tasks: live });
    dashboard.loadedAt = NOW;
    const without = render(ActivityWidget);
    const shown = pill(without.container);
    without.unmount();
    expect(shown.label).toBe('Activity: 2 tasks completed, archived tasks left out');
    expect(shown.title).toMatch(/all time: 2 \(archived tasks left out\)/);

    loadIntoDashboard({ meta: { archived: 2 }, tasks: live }, { tasks: archived });
    dashboard.loadedAt = NOW;
    expect(dashboard.history).toHaveLength(4);
    const withArchive = render(ActivityWidget);
    expect(pill(withArchive.container)).toEqual(shown);
    withArchive.unmount();
  });

  it('a project without an archive says nothing of one', () => {
    loadIntoDashboard({ tasks: live });
    dashboard.loadedAt = NOW;
    const { container } = render(ActivityWidget);
    expect(pill(container).label).toBe('Activity: 2 tasks completed');
    expect(pill(container).title).not.toContain('archived');
  });

  it('the overview’s: its task files’ tasks by project, not the archives in its charts', () => {
    loadIntoDashboard({ tasks: [] });
    dashboard.loadedAt = NOW;
    // As atm serves them, with their kinds
    const byProject = (tasks: Task[]) => withKinds(tasks, DEFAULT_VOCABULARY).map((t) => ({ ...t, type: 'One' }));
    const source = (archivedToo: boolean) => ({
      tasks: archivedToo ? [...live, ...archived] : live,
      claims: new Map(),
      byProject: byProject(archivedToo ? [...live, ...archived] : live),
      liveByProject: byProject(live),
      archived: true,
      projects: [{ name: 'One', color: 'red' }],
      of: (w: string) => w,
    });
    const one = render(ActivityWidget, { source: source(false) });
    const shown = pill(one.container);
    one.unmount();
    expect(shown.label).toBe('Activity: 2 tasks completed, archived tasks left out');
    const two = render(ActivityWidget, { source: source(true) });
    expect(pill(two.container)).toEqual(shown);
  });
});
