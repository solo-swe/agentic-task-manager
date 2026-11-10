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
