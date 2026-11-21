// In the atm app, ⌘-click, Ctrl-click or a middle click opens a task or a document in a new tab (T-515, left from
// T-514), as a browser opens a link. One listener on the page decides (routes/+layout.svelte); this says where to.
import { dashboardAddress, hrefOf, linkAt } from './compmd';
import { docOf, withDoc } from './documents';
import { withTask } from './routes';

/** The address a click on `target` opens in a new tab, from the page at `address`; null for anything else: a task's
 * card or row, a document's link, or any link to one of the dashboard's own pages (a review row, a reference; ATM-698:
 * every card and row is a real link, and this is the one place a click on one opens a tab) */
export function newTabAddress(target: Element | null, address: string): string | null {
  const link = linkAt(target);
  const href = link ? hrefOf(link) : null;
  const doc = href ? docOf(href) : null;
  if (doc) return withDoc(address, doc);
  // `#…` links are the text's own (a task's key): the text opens them. Another link to one of the dashboard's pages,
  // resolved from the page shown
  const own = href && !href.startsWith('#') ? dashboardAddress(new URL(href, location.origin + address).href) : null;
  if (own) return own;
  const task = target?.closest<HTMLElement>('.kanban-card[data-id], .task-card[data-id]')?.dataset.id;
  return task ? withTask(address, task) : null;
}

/** A click that opens in place: the main button, no ⌘, Ctrl, ⇧ or ⌥ (any other is a new tab's, or the browser's) */
export function plainClick(e: MouseEvent): boolean {
  return e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;
}

/** A card's or row's link clicked (ATM-698): a plain click opens it in place with `open` (its details), and goes no
 * further; any other is left to the browser (a new tab), or in the app to the page's one handler, which saw it first */
export function followInPlace(e: MouseEvent, open: () => void) {
  if (!plainClick(e)) return;
  e.preventDefault();
  e.stopPropagation();
  open();
}
