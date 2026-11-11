// What each of the app's menu items does on the page (T-512): what the same control does now. Tabs come in T-514 and
// the sidebar in T-515: their items do nothing yet. Quit and New Window the app does itself, except in the menus the
// page draws (Windows, Linux, T-513).
import type { MenuAction, MenuState, SystemItem } from './api/menu';
import { currentAddress } from './navigation';
import { pluginInfo } from './plugin-info';
import type { DialogId } from './dialogs';
import type { ViewId } from './tabs';

/** CompMD's site, Help ▸ CompMD */
export const COMPMD_URL = 'https://compmd.hakanserce.com/';

/** What the menu's actions act on: the dashboard's state, and the page's own (in tests, stand-ins) */
export interface MenuTarget {
  readonly onProject: boolean;
  readonly tab: ViewId;
  readonly includeArchived: boolean;
  readonly workspace: string;
  settingsOpen: boolean;
  filtersOpen: boolean;
  openDialog(id: DialogId): void;
  openWhatsNew(): void;
  selectTab(tab: ViewId): void;
  setIncludeArchived(on: boolean): void;
  openProject(id: string | null): void;
  openWorkspace(id: string): void;
  checkAppUpdate(now?: number, force?: boolean): Promise<void>;
}

/** What the page itself does for some actions (the browser's, in the app's webview) */
export interface PageActions {
  back(): void;
  forward(): void;
  reload(): void;
  open(url: string): void;
  copy(text: string): Promise<void>;
  focusSearch(): void;
}

export const pageActions: PageActions = {
  back: () => history.back(),
  forward: () => history.forward(),
  reload: () => location.reload(),
  open: (url) => void window.open(url, '_blank', 'noopener,noreferrer'),
  copy: (text) => navigator.clipboard.writeText(text),
  focusSearch: () => document.querySelector<HTMLInputElement>('#task-search')?.focus(),
};

const VIEWS: Partial<Record<MenuAction['kind'], ViewId>> = {
  show_graph: 'graph',
  show_kanban: 'kanban',
  show_list: 'list',
  show_agents: 'agents',
  show_progress: 'progress',
  show_project: 'project',
};

/** The deep link to the page shown: `atm://` and its address */
export function pageLink(address: string = currentAddress()): string {
  return `atm:/${address}`;
}

/** Do what `action` asks, on `target` */
export function runMenuAction(action: MenuAction, target: MenuTarget, page: PageActions = pageActions): void {
  const view = VIEWS[action.kind];
  if (view) {
    if (target.onProject) target.selectTab(view);
    return;
  }
  switch (action.kind) {
    case 'about':
    case 'help':
      target.openDialog(action.kind);
      return;
    case 'connect_agents':
      target.openDialog('agents');
      return;
    case 'whats_new':
      target.openWhatsNew();
      return;
    case 'check_for_updates':
      void target.checkAppUpdate(Date.now(), true);
      return;
    case 'settings':
      target.settingsOpen = true;
      return;
    case 'toggle_filters':
      target.filtersOpen = !target.filtersOpen;
      return;
    case 'toggle_archived':
      target.setIncludeArchived(!target.includeArchived);
      return;
    case 'find':
      page.focusSearch();
      return;
    case 'copy_link':
      void page.copy(pageLink()).catch((e: unknown) => console.error('Copying the link failed:', e));
      return;
    case 'reload':
      page.reload();
      return;
    case 'back':
      page.back();
      return;
    case 'forward':
      page.forward();
      return;
    case 'overview':
      target.openProject(null);
      return;
    case 'project':
      target.openProject(action.id);
      return;
    case 'workspace':
      target.openWorkspace(action.id);
      return;
    case 'github':
      page.open(pluginInfo.repository);
      return;
    case 'compmd':
      page.open(COMPMD_URL);
      return;
    default:
      // Tabs (T-514), the sidebar (T-515); Quit and New Window are the app's
      return;
  }
}

/** The window commands the menus the page draws (Windows, Linux, T-513) need: there the app doesn't see the click */
export interface WindowActions {
  openWindow(address: string): Promise<unknown>;
  closeWindow(): Promise<unknown>;
  minimize(): Promise<unknown>;
  exec(command: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'selectAll'): void;
}

/** An item of a menu the page draws: what the menu bar's would do (Quit closes this window; with the last, the app) */
export function runDrawnAction(
  action: MenuAction,
  target: MenuTarget,
  window: WindowActions,
  page: PageActions = pageActions,
): void {
  if (action.kind === 'quit') void window.closeWindow();
  else if (action.kind === 'new_window') void window.openWindow('/');
  else runMenuAction(action, target, page);
}

/** One of the system's items in a menu the page draws; those only macOS has do nothing */
export function runSystemItem(item: SystemItem, window: WindowActions): void {
  const commands: Partial<Record<SystemItem, Parameters<WindowActions['exec']>[0]>> = {
    undo: 'undo',
    redo: 'redo',
    cut: 'cut',
    copy: 'copy',
    paste: 'paste',
    select_all: 'selectAll',
  };
  const command = commands[item];
  if (command) window.exec(command);
  else if (item === 'minimize') void window.minimize();
}

/** What the menu shows checked for `target` */
export function menuStateOf(target: MenuTarget): MenuState {
  return {
    view: target.onProject ? target.tab : null,
    sidebar: false,
    filters: target.filtersOpen,
    archived: target.includeArchived,
    workspace: target.workspace,
  };
}
