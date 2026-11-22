import { fireEvent } from '@testing-library/svelte';
import { emptyFilters } from '../../src/lib/filters';
import { parseTasksFile } from '../../src/lib/load';
import { dashboard } from '../../src/lib/state.svelte';
import type { Release } from '../../src/lib/releases';
import type { SprintModel } from '../../src/lib/sprints';
import type { TasksFile } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { sprintModel, withKinds, withLinks } from './views';

/** What atm serves beside the tasks, as a test gives it */
export interface Served {
  sprints?: SprintModel;
  releases?: Release[];
}

/**
 * Put the shared dashboard state into a freshly loaded state for a component test, as atm serves the project: its
 * vocabulary (`vocab`, the defaults unless given), each task's kind (ATM-718), and its sprints and releases (ATM-720:
 * `served`'s, else the fake's for the fixture's meta, views.ts). `archive` stands for a loaded tasks-archive.json;
 * without it there is none loaded.
 */
export function loadIntoDashboard(
  data: TasksFile,
  archive?: TasksFile,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  served: Served = {},
) {
  // atm's answers on each task (its kind, then the links and places it works out over the project, ATM-719)
  const withTheirKinds = { ...data, tasks: withLinks(withKinds(data.tasks ?? [], vocab), data.meta) };
  const { tasks, taskMap, projectName, meta, bytes } = parseTasksFile(withTheirKinds, JSON.stringify(data).length);
  const archived = withKinds(archive?.tasks ?? [], vocab);
  const releases = (meta as { releases?: { id: string }[] } | undefined)?.releases ?? [];
  Object.assign(dashboard, {
    tasks,
    taskMap,
    projectName,
    meta,
    tasksBytes: bytes,
    vocab,
    sprints: served.sprints ?? sprintModel(meta, [...tasks, ...archived], vocab),
    releases:
      served.releases ??
      releases.map((r) => ({
        state: 'planned',
        sprints: [],
        total: 0,
        done: 0,
        in_pr: 0,
        merged: 0,
        ...r,
        notes: { groups: [], open: [] },
      })),
    loaded: true,
    loadError: null,
    filters: emptyFilters(),
    query: '',
    detailId: null,
    detailOpen: false,
    archive: archived,
    archiveLoaded: !!archive,
    includeArchived: false,
    // Keep component tests free of background reloads (the real default is on)
    autoRefresh: false,
  });
}

/** Let Bits UI's layers arm (T-431): a dialog or popup listens for Escape and outside clicks a moment after it opens */
export const settled = () => new Promise((resolve) => setTimeout(resolve, 20));

/** A click on `el` as a pointer makes it, which is how Bits UI tells a click outside a dialog or popup (it acts on
 * the pointerdown a few milliseconds later) */
export async function press(el: Element) {
  // Off the popup's box, which is jsdom's empty one at 0, 0: Bits UI checks that the click is truly outside it
  const at = { button: 0, clientX: 1, clientY: 1 };
  await fireEvent.pointerDown(el, at);
  await fireEvent.pointerUp(el, at);
  await fireEvent.click(el, at);
  await settled();
}
