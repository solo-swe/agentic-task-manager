// What the app's ‹ › arrows list on a right-click or a long press (ATM-695, docs/tabs-and-navigation-design.md §4.5):
// the active tab's entries before the one shown (‹) or after it (›), nearest first, as a browser's Back and Forward
// buttons list theirs, each named by its place, to jump to.
import { addressParams } from './address';
import type { HistoryEntry } from './navigation';
import { placeName } from './places';

/** An entry to jump to: its address, its name, and how many steps back it is (negative: forward), as `back` takes */
export interface HistoryItem {
  address: string;
  name: string;
  steps: number;
}

/** The entries of `history` before (`-1`) or after (`1`) the one shown, nearest first. `projectName` names a project
 * by its ID (none: the ID) */
export function historyItems(
  history: { entries: HistoryEntry[]; at: number } | undefined,
  direction: -1 | 1,
  projectName: (id: string) => string | undefined = () => undefined,
): HistoryItem[] {
  if (!history) return [];
  const items: HistoryItem[] = [];
  for (let i = history.at + direction; i >= 0 && i < history.entries.length; i += direction) {
    const { address } = history.entries[i];
    items.push({ address, name: entryName(address, projectName), steps: history.at - i });
  }
  return items;
}

/** An entry's name: its place, and the project it is in ("Kanban — Website", "ATM-12 — Website"); a place of the
 * Projects Overview, alone */
export function entryName(address: string, projectName: (id: string) => string | undefined = () => undefined): string {
  const place = placeName(address);
  const project = addressParams(address).get('project');
  return project ? `${place} — ${projectName(project) ?? project}` : place;
}
