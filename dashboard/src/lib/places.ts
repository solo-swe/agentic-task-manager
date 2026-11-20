// A place's name, from its address (ATM-686): what a back button says it goes back to ("← Kanban", "← ATM-12",
// "← All reviews"). What is open over a page names it first (a task's details, a document), then a review, then the
// page itself.
import { addressParams } from './address';

/** A project's tabs, as their buttons name them */
const PROJECT_TABS: Record<string, string> = {
  kanban: 'Kanban',
  list: 'List',
  graph: 'Graph',
  progress: 'Progress',
  agents: 'Agents',
  project: 'Project',
  reviews: 'Reviews',
};

/** The overview's tabs (address.ts OVERVIEW_TABS), as their buttons name them */
const OVERVIEW_TABS: Record<string, string> = {
  projects: 'Projects',
  board: 'Omniboard',
  progress: 'Progress',
  agents: 'Agents',
  reviews: 'All reviews',
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
  if (params.has('project')) return PROJECT_TABS[view] ?? 'Kanban';
  return 'Projects';
}
