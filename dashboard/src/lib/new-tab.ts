// The app's new tab (`/new`, ATM-755, docs/sidebar-overhaul-design.md §3): a chooser of scope and item, as a browser's
// new tab page is. Two columns, the scopes (All, then the workspace's projects) and the chosen scope's items; the keys
// move in a column and between them, typing filters the projects, Enter opens. Its state and the keys' rules, apart
// from the component.
import type { Item } from './address';
import { itemsOf, type Scope } from './sidebar';

/** What the chooser shows: the scope and the item chosen, the column the keys move in, and the projects' filter */
export interface Chooser {
  scope: Scope;
  item: number;
  column: 0 | 1;
  filter: string;
}

/** A project as the chooser lists it */
export interface ChooserProject {
  id: string;
  name: string;
}

/** The scopes listed: All projects, then the projects the filter keeps (by name or ID, any case); All is left out
 * while a filter is typed that it doesn't match */
export function scopesOf(projects: readonly ChooserProject[], filter: string): Scope[] {
  const words = filter.trim().toLowerCase();
  if (!words) return [null, ...projects.map((p) => p.id)];
  const kept = projects
    .filter((p) => p.name.toLowerCase().includes(words) || p.id.toLowerCase().includes(words))
    .map((p) => p.id);
  return 'all projects'.includes(words) ? [null, ...kept] : kept;
}

/** The chooser after the filter is `filter`: its scope the first listed when the one chosen is gone, its item the
 * first, the keys in the scopes' column */
export function filtered(state: Chooser, projects: readonly ChooserProject[], filter: string): Chooser {
  const scopes = scopesOf(projects, filter);
  const scope = scopes.includes(state.scope) ? state.scope : (scopes[0] ?? state.scope);
  return { scope, item: scope === state.scope ? state.item : 0, column: 0, filter };
}

/** What a key does (nothing for a key it doesn't take): the chooser after it, or the item to open */
export function keyed(
  state: Chooser,
  projects: readonly ChooserProject[],
  key: string,
): { state: Chooser } | { open: Item } | null {
  const scopes = scopesOf(projects, state.filter);
  const items = itemsOf(state.scope);
  const step = key === 'ArrowDown' ? 1 : key === 'ArrowUp' ? -1 : 0;
  if (step && state.column === 0) {
    if (!scopes.length) return null;
    const at = Math.max(0, scopes.indexOf(state.scope));
    const scope = scopes[Math.min(scopes.length - 1, Math.max(0, at + step))];
    const item = Math.min(state.item, itemsOf(scope).length - 1);
    return { state: { ...state, scope, item } };
  }
  if (step) return { state: { ...state, item: Math.min(items.length - 1, Math.max(0, state.item + step)) } };
  if (key === 'ArrowRight') return { state: { ...state, column: 1 } };
  if (key === 'ArrowLeft') return { state: { ...state, column: 0 } };
  if (key === 'Enter') return scopes.includes(state.scope) && items[state.item] ? { open: items[state.item] } : null;
  if (key === 'Backspace' && state.filter) return { state: filtered(state, projects, state.filter.slice(0, -1)) };
  if (key === 'Escape' && state.filter) return { state: filtered(state, projects, '') };
  // A character typed: the projects' filter
  if (key.length === 1 && key !== ' ') return { state: filtered(state, projects, state.filter + key) };
  return null;
}
