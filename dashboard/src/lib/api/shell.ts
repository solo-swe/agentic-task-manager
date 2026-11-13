// The atm app's window (T-511, docs/app-desktop-ui-design.md §1a): what it has of the platform's own, its title, and
// a new window at one of the dashboard's pages. Each command acts on the window that calls it
// (app/src-tauri/src/windows.rs). In the browser none of this applies.
import { invoke } from '@tauri-apps/api/core';
import { linkTarget } from './links';

/** What the window has of the platform's own: the page draws only the rest */
export interface AppShell {
  /** The system's menu bar has the app's menus (macOS): the page draws none */
  system_menu_bar: boolean;
  /** The system draws the window's buttons over the page (macOS): the page leaves room for them */
  system_window_buttons: boolean;
}

/** In the browser, and when the app can't say: the page's own shell, as now */
export const NO_SHELL: AppShell = { system_menu_bar: false, system_window_buttons: false };

let asked: Promise<AppShell> | null = null;

/** The window's shell, asked once */
export function appShell(): Promise<AppShell> {
  asked ??= invoke<AppShell>('shell').catch((e: unknown) => {
    console.error('Asking the app for its window failed:', e);
    return NO_SHELL;
  });
  return asked;
}

/** Forget the shell asked (tests) */
export function resetAppShell(): void {
  asked = null;
}

/** Give the window `title` (the window switcher's, the taskbar's), as the page sets `document.title` */
export async function setWindowTitle(title: string): Promise<void> {
  try {
    await invoke('set_title', { title });
  } catch (e) {
    console.error('Setting the window’s title failed:', e);
  }
}

/** One of the window commands (T-513): on the calling window; a failure is only logged */
async function windowCommand(command: 'minimize' | 'toggle_maximize' | 'close_window' | 'start_dragging') {
  try {
    await invoke(command);
  } catch (e) {
    console.error(`The window's ${command} failed:`, e);
  }
}

/** The page's window buttons (Windows, Linux) and its bars (a drag, a double click) */
export const minimizeWindow = () => windowCommand('minimize');
export const toggleMaximizeWindow = () => windowCommand('toggle_maximize');
export const closeWindow = () => windowCommand('close_window');
export const startDragging = () => windowCommand('start_dragging');

/** What a mouse-down on a bar does: a drag, or a double click maximizing, unless it is on one of the bar's controls */
export function onBarMouseDown(e: MouseEvent): void {
  if (e.button !== 0) return;
  const target = e.target as Element | null;
  const controls =
    'button, a, input, select, textarea, [role="menuitem"], [role="menubar"], [role="tab"], [data-no-drag]';
  if (target?.closest(controls)) return;
  e.preventDefault();
  void (e.detail === 2 ? toggleMaximizeWindow() : startDragging());
}

/** A window's tabs as the app keeps them for the next start (T-518): their addresses, in order, and the active one */
export interface KeptTabs {
  addresses: string[];
  active: number;
}

/** The tabs the window comes back with when the app starts (once; none for a new window or a reload) */
export async function restoredTabs(): Promise<KeptTabs | null> {
  try {
    const kept = await invoke<KeptTabs | null>('restored_tabs');
    if (!kept) return null;
    const addresses = kept.addresses.map(linkTarget).filter((a): a is string => a !== null);
    return addresses.length ? { addresses, active: Math.min(kept.active, addresses.length - 1) } : null;
  } catch (e) {
    console.error('Asking the app for the window’s tabs failed:', e);
    return null;
  }
}

/** The window's tabs, for the app to bring back next time (T-518) */
export async function saveTabs(tabs: KeptTabs): Promise<void> {
  try {
    await invoke('save_tabs', { tabs });
  } catch (e) {
    console.error('Keeping the window’s tabs failed:', e);
  }
}

/** What needs the user (T-518, T-520): the page shows `workspace`, whose projects changed; the app reads how many
 * tasks need the user in them, shows the total on its icon (`badge`) and notifies a project whose count went up
 * (`notify`). The counts and the notifications' text are the app's own */
export async function reportNeedsYou(workspace: string, badge: boolean, notify: boolean): Promise<void> {
  try {
    await invoke('needs_you', { workspace, badge, notify });
  } catch (e) {
    console.error('Telling the app what needs you failed:', e);
  }
}

/** Open a new window at `address`, one of the dashboard's pages (the app checks it again); whether it opened */
export async function openWindow(address: string): Promise<boolean> {
  const target = linkTarget(address);
  if (!target) return false;
  try {
    await invoke('open_window', { address: target });
    return true;
  } catch (e) {
    console.error('Opening a window failed:', e);
    return false;
  }
}
