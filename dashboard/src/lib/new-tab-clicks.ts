// In the atm app, ⌘-click, Ctrl-click or a middle click opens a task or a document in a new tab (T-515, left from
// T-514), as a browser opens a link. One listener on the page decides (routes/+layout.svelte); this says where to.
import { docOf, withDoc } from './documents';
import { withTask } from './routes';

/** The address a click on `target` opens in a new tab, from the page at `address`; null for anything else */
export function newTabAddress(target: Element | null, address: string): string | null {
  const task = target?.closest<HTMLElement>('.kanban-card[data-id], .task-card[data-id]')?.dataset.id;
  if (task) return withTask(address, task);
  const href = target?.closest('a[href]')?.getAttribute('href');
  const doc = href ? docOf(href) : null;
  return doc ? withDoc(address, doc) : null;
}
