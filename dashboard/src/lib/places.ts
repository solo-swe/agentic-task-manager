// A place's name, from its address (ATM-686): what a back button says it goes back to ("← Board", "← ATM-12",
// "← All reviews"). What is open over a page names it first (a task's details, a document), then a review, then the
// page itself.
import { addressParams, itemOf, namesNewTab, reviewOf, type Item } from './address';
import { ITEM_ICONS } from './tabs';

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

/** How a tab names its project: its key, else its name (api/projects lists both) */
export interface ProjectNaming {
  key?: string | null;
  name: string;
}

/** A scope's name cut to this many characters, when it has no key */
const SCOPE_MAX = 16;

/** A tab's scope (docs/sidebar-overhaul-design.md §3, ATM-754): the project's key (`ATM`), else its name cut to 16
 * characters (its ID while the projects aren't listed); All projects is `All` */
export function scopeName(id: string | null, project: ProjectNaming | undefined): string {
  if (id === null) return 'All';
  const key = project?.key?.trim();
  if (key) return key;
  const name = project?.name.trim() || id;
  return name.length > SCOPE_MAX ? `${name.slice(0, SCOPE_MAX - 1)}…` : name;
}

/** A tab's title and icon (ATM-754) */
export interface TabTitle {
  title: string;
  icon: (typeof ITEM_ICONS)[Item] | 'plus';
}

/**
 * The title of a tab showing `address` (docs/sidebar-overhaul-design.md §3, ATM-754): `<scope> - <item>` with the
 * item's icon (`ATM - Tasks`, `All - Code Reviews`). A review or a document open in the tab names itself instead
 * (`ATM - CR-134`, `ATM - sidebar-overhaul-design.md`), keeping the item's icon; `/new` is *New Tab*. The landing
 * page (`/`) is All's Overview. `project` names a project by its ID.
 */
export function tabTitle(address: string, project: (id: string) => ProjectNaming | undefined): TabTitle {
  if (namesNewTab(address)) return { title: 'New Tab', icon: 'plus' };
  const params = addressParams(address);
  const id = params.get('project');
  const scope = scopeName(id, id === null ? undefined : project(id));
  const item = itemOf(address) ?? 'overview';
  const icon = ITEM_ICONS[item];
  const doc = params.get('doc');

  // The file's name, without a link's lines or text (`docs/x.md#L4-L9`)
  const file = doc?.split('#')[0].split('/').filter(Boolean).at(-1);
  if (file) return { title: `${scope} - ${file}`, icon };
  const review = reviewOf(address).review;
  if (review) return { title: `${scope} - ${review}`, icon };
  return { title: `${scope} - ${ITEM_NAMES[item]}`, icon };
}

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
