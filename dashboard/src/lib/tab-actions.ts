// What the atm app's tabs do (T-514): the tab strip's clicks and the menu's items (File ▸ New Tab, Close Tab; Window ▸
// Next Tab, Previous Tab, Move Tab to New Window) both come here. Each changes the tab list (app-tabs.svelte.ts) and
// goes to the address it answers; closing the last tab closes the window.
import { appTabs, type AppTabs } from './app-tabs.svelte';
import { closeWindow, openWindow } from './api/shell';
import { rememberScroll, showTab } from './navigation';

/** A click that asks for a new tab: ⌘-click, Ctrl-click or a middle click, as in a browser */
export function wantsNewTab(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.button === 1;
}

/** Whether such a click keeps the new tab in the background, as browsers do: all but ⇧⌘ (⇧Ctrl) brings it forward */
export function inBackground(e: MouseEvent): boolean {
  return !e.shiftKey;
}

export interface TabActions {
  /** A new tab at `address` (the workspace's overview by default), shown; in the `background`, right of this one,
   * which stays shown (ATM-698: ⌘-click and a middle click; ⇧⌘-click brings it forward) */
  open(address?: string, background?: boolean): void;
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
  /** Show the active tab, now at `address` */
  go(address: string): void;
  /** The tab shown is about to be left: it keeps where its page is scrolled */
  leave?(): void;
  closeWindow(): unknown;
  openWindow(address: string): Promise<boolean>;
}

/** A tab shown as it was left, its history as it was (ATM-694): a switch adds to no history */
const window_: TabWindow = { go: () => showTab(), leave: rememberScroll, closeWindow, openWindow };

/** The actions on `tabs`, in `win` */
export function tabActions(tabs: AppTabs = appTabs, win: TabWindow = window_): TabActions {
  const go = (address: string | null | undefined) => {
    if (address === null) void win.closeWindow();
    else if (address !== undefined) win.go(address);
  };
  /** The tab shown is left (a switch, a new tab): before the tabs change, so its own entry keeps its scroll */
  const leaving = <T>(change: () => T): T => {
    win.leave?.();
    return change();
  };
  return {
    open: (address = '/', background = false) =>
      background ? void tabs.open(address, true) : go(leaving(() => tabs.open(address))),
    select: (id) => go(id === tabs.active ? undefined : (leaving(() => tabs.select(id)) ?? undefined)),
    close: (id = tabs.active) => go(tabs.close(id)),
    step: (step) => go(leaving(() => tabs.step(step)) ?? undefined),
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
