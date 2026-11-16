import { fireEvent } from '@testing-library/svelte';
import { emptyFilters } from '../../src/lib/filters';
import { parseTasksFile } from '../../src/lib/load';
import { dashboard } from '../../src/lib/state.svelte';
import type { TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';

/**
 * Put the shared dashboard state into a freshly loaded state for a component test. `archive` stands for a loaded
 * tasks-archive.json; without it there is none loaded.
 */
export function loadIntoDashboard(data: TasksFile, archive?: TasksFile) {
  const { tasks, taskMap, projectName, meta, bytes } = parseTasksFile(data, JSON.stringify(data).length);
  Object.assign(dashboard, {
    tasks,
    taskMap,
    projectName,
    meta,
    tasksBytes: bytes,
    vocab: resolveVocabulary(meta),
    loaded: true,
    loadError: null,
    filters: emptyFilters(),
    query: '',
    detailId: null,
    detailOpen: false,
    archive: archive?.tasks ?? [],
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
