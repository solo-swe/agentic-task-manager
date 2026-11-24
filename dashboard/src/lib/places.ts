// A place's name, from its address (ATM-686): what a back button says it goes back to ("← Board", "← ATM-12",
// "← All reviews"). What is open over a page names it first (a task's details, a document), then a review, then the
// page itself.
import { addressParams, type Item } from './address';

/** A project's tabs, as their buttons name them (the Kanban is the Board and the graph Dependencies, ATM-751) */
const PROJECT_TABS: Record<string, string> = {
  kanban: 'Board',
  list: 'List',
  graph: 'Dependencies',
  progress: 'Progress',
  agents: 'Agents',
  project: 'Overview',
  reviews: 'Reviews',
  inbox: 'Inbox',
  documents: 'Documents',
};

/** The overview's pages (address.ts OVERVIEW_PATHS), as their buttons name them */
const OVERVIEW_TABS: Record<string, string> = {
  projects: 'Projects',
  board: 'Board',
  list: 'List',
  progress: 'Progress',
  agents: 'Agents',
  reviews: 'All reviews',
};

/** The sidebar's items, as the sidebar and the tabs' titles name them (docs/sidebar-overhaul-design.md §1) */
export const ITEM_NAMES: Record<Item, string> = {
  overview: 'Overview',
  inbox: 'Inbox',
  tasks: 'Tasks',
  reviews: 'Code Reviews',
  documents: 'Documents',
  analysis: 'Analysis',
};

/** What `address` shows, in a word or two */
export function placeName(address: string): string {
  const params = addressParams(address);
  const task = params.get('task');
  if (task) return task;
  if (params.has('doc')) return 'Document';
  const review = params.get('review');
  if (review) return review;
  const view = params.get('view') ?? '';
  if (params.has('overview')) return OVERVIEW_TABS[view] ?? 'Projects';
  // A project without a tab (or one it doesn't know) opens on its Overview, the Project tab (ATM-751)
  if (params.has('project')) return PROJECT_TABS[view] ?? PROJECT_TABS.project;
  return 'Projects';
}
