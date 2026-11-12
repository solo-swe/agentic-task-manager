// What the atm app's tabs do (T-514): the tab strip's clicks and the menu's items (File ▸ New Tab, Close Tab; Window ▸
// Next Tab, Previous Tab, Move Tab to New Window) both come here. Each changes the tab list (app-tabs.svelte.ts) and
// goes to the address it answers; closing the last tab closes the window.
import { appTabs, type AppTabs } from './app-tabs.svelte';
import { closeWindow, openWindow } from './api/shell';
import { navigate } from './navigation';

export interface TabActions {
  /** A new tab at `address` (the workspace's overview by default) */
  open(address?: string): void;
  select(id: string): void;
  /** Close tab `id`, the active one by default */
  close(id?: string): void;
  step(step: 1 | -1): void;
  move(id: string, to: number): void;
  /** The active tab into a window of its own */
  toWindow(): void;
}

/** What a window has to do for its tabs: in tests, stand-ins */
export interface TabWindow {
  go(address: string): void;
  closeWindow(): unknown;
  openWindow(address: string): Promise<boolean>;
}

const window_: TabWindow = { go: (address) => navigate(address), closeWindow, openWindow };

/** The actions on `tabs`, in `win` */
export function tabActions(tabs: AppTabs = appTabs, win: TabWindow = window_): TabActions {
  const go = (address: string | null | undefined) => {
    if (address === null) void win.closeWindow();
    else if (address !== undefined) win.go(address);
  };
  return {
    open: (address = '/') => go(tabs.open(address)),
    select: (id) => go(tabs.select(id) ?? undefined),
    close: (id = tabs.active) => go(tabs.close(id)),
    step: (step) => go(tabs.step(step) ?? undefined),
    move: (id, to) => tabs.move(id, to),
    toWindow: () => {
      const tab = tabs.current;
      if (!tab) return;
      void win.openWindow(tab.address).then((opened) => {
        if (opened) go(tabs.close(tab.id));
      });
    },
  };
}
