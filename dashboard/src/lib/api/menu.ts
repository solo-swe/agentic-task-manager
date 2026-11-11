// The atm app's menus (T-512, docs/app-desktop-ui-design.md §6): defined once in the app (app/src-tauri/src/menu.rs);
// an item sends the window in front an action on its channel, and the page does what the same control does
// (lib/menu-actions.ts). The page reports what the menu shows checked. Each command acts on the calling window.
import { Channel, invoke } from '@tauri-apps/api/core';

/** What a menu item asks for (the app's `MenuAction`) */
export type MenuAction =
  | {
      kind:
        | 'about'
        | 'check_for_updates'
        | 'settings'
        | 'connect_agents'
        | 'new_tab'
        | 'new_window'
        | 'close_tab'
        | 'copy_link'
        | 'find'
        | 'show_graph'
        | 'show_kanban'
        | 'show_list'
        | 'show_agents'
        | 'show_progress'
        | 'show_project'
        | 'toggle_sidebar'
        | 'toggle_filters'
        | 'toggle_archived'
        | 'reload'
        | 'back'
        | 'forward'
        | 'overview'
        | 'next_tab'
        | 'previous_tab'
        | 'move_tab_to_new_window'
        | 'help'
        | 'whats_new'
        | 'github'
        | 'compmd'
        | 'quit';
    }
  | { kind: 'project' | 'workspace'; id: string };

/** What the menu shows checked for this window (the app's `MenuState`) */
export interface MenuState {
  view: string | null;
  sidebar: boolean;
  filters: boolean;
  archived: boolean;
  workspace: string | null;
}

/** Listen for the menu's actions, `run` each one */
export async function listenForMenu(run: (action: MenuAction) => void): Promise<void> {
  const channel = new Channel<MenuAction>();
  channel.onmessage = (action) => {
    if (action && typeof action === 'object' && typeof action.kind === 'string') run(action);
  };
  await invoke('subscribe_menu', { channel });
}

/** Tell the app what this window's menu shows checked */
export async function reportMenuState(shown: MenuState): Promise<void> {
  try {
    await invoke('menu_state', { shown });
  } catch (e) {
    console.error('Telling the app what the menu shows failed:', e);
  }
}
