// atm's own right-click menus in the app (ATM-699, docs/tabs-and-navigation-design.md §4.5): on a link, card or row,
// on a title-bar tab, on selected text, and elsewhere. WebKit's own menu is turned off in the app (its Reload would
// load the page's route, not the app's page, ATM-584), except in a field you type in, where the system's Cut, Copy and
// Paste stay. Selected text gets atm's menu with Copy (WebKit's would bring its Reload back). The browser keeps its own
// menus. This module says what a right-click is on and which items its menu has; ui/AppContextMenu.svelte shows them.
import type { AppTab } from './app-tabs.svelte';
import { pageLink } from './menu-actions';
import { newTabAddress } from './new-tab-clicks';

/** What a right-click is on */
export type MenuTarget =
  /** A field you type in: the system's menu (Cut, Copy, Paste), not atm's */
  | { kind: 'text' }
  /** Text selected on the page, outside a field: atm's menu, Copy first */
  | { kind: 'selection'; text: string }
  /** A link, card or row to one of the dashboard's pages, by the address it opens */
  | { kind: 'link'; address: string }
  /** A title-bar tab, by its ID */
  | { kind: 'tab'; id: string }
  /** Anywhere else */
  | { kind: 'page' };

/** The prefix of a title-bar tab's element ID (AppTitleBar's `idPrefix`) */
export const TAB_ID_PREFIX = 'app-tab-';

/** What a right-click on `target` is on, from the page at `address`; `selected`, the text selected now */
export function menuTargetAt(target: Element | null, address: string, selected = ''): MenuTarget {
  if (target?.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]'))
    return { kind: 'text' };
  if (selected.trim()) return { kind: 'selection', text: selected };
  const tab = target?.closest<HTMLElement>(`[role="tab"][id^="${TAB_ID_PREFIX}"]`);
  if (tab) return { kind: 'tab', id: tab.id.slice(TAB_ID_PREFIX.length) };
  const link = newTabAddress(target, address);
  if (link) return { kind: 'link', address: link };
  return { kind: 'page' };
}

/** One item of a menu: what it says, and what it does; a disabled one does nothing */
export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  /** A line before it, between the menu's groups */
  separated?: boolean;
  run(): void;
}

/** What the menus act on: the window's tabs and the page (in tests, stand-ins) */
export interface MenuDeps {
  /** The window's tabs, in order */
  tabs(): readonly AppTab[];
  /** Open `address` here, in the tab shown */
  go(address: string): void;
  /** A new tab at `address` (the overview's by default); in the `background`, right of this one */
  openTab(address?: string, background?: boolean): void;
  closeTab(id: string): void;
  /** A new window at `address`; whether it opened */
  openWindow(address: string): Promise<boolean>;
  copy(text: string): Promise<void>;
  canGo(delta: -1 | 1): boolean;
  back(): void;
  forward(): void;
  reload(): void;
}

const copyLink = (deps: MenuDeps, address: string) => () =>
  void deps.copy(pageLink(address)).catch((e: unknown) => console.error('Copying the link failed:', e));

/** Back, Forward and Reload, for this tab */
const pageItems = (deps: MenuDeps, separated = false): MenuItem[] => [
  { id: 'back', label: 'Back', disabled: !deps.canGo(-1), separated, run: deps.back },
  { id: 'forward', label: 'Forward', disabled: !deps.canGo(1), run: deps.forward },
  { id: 'reload', label: 'Reload', run: deps.reload },
];

/** The items of the menu for `target`; none in a field (the system's menu shows) */
export function menuItems(target: MenuTarget, deps: MenuDeps): MenuItem[] {
  switch (target.kind) {
    case 'text':
      return [];
    case 'selection': {
      const { text } = target;
      const copy = () => void deps.copy(text).catch((e: unknown) => console.error('Copying the text failed:', e));
      return [{ id: 'copy', label: 'Copy', run: copy }, ...pageItems(deps, true)];
    }
    case 'link': {
      const { address } = target;
      return [
        { id: 'open', label: 'Open', run: () => deps.go(address) },
        { id: 'open-tab', label: 'Open in New Tab', run: () => deps.openTab(address, true) },
        { id: 'open-window', label: 'Open in New Window', run: () => void deps.openWindow(address) },
        { id: 'copy-link', label: 'Copy Link', separated: true, run: copyLink(deps, address) },
      ];
    }
    case 'tab': {
      const tabs = deps.tabs();
      const tab = tabs.find((t) => t.id === target.id);
      if (!tab) return [];
      const others = tabs.filter((t) => t.id !== tab.id);
      return [
        { id: 'new-tab', label: 'New Tab', run: () => deps.openTab() },
        { id: 'duplicate', label: 'Duplicate', run: () => deps.openTab(tab.address) },
        {
          id: 'to-window',
          label: 'Move to New Window',
          run: () =>
            void deps.openWindow(tab.address).then((opened) => {
              if (opened) deps.closeTab(tab.id);
            }),
        },
        { id: 'copy-link', label: 'Copy Link', separated: true, run: copyLink(deps, tab.address) },
        { id: 'close', label: 'Close', separated: true, run: () => deps.closeTab(tab.id) },
        {
          id: 'close-others',
          label: 'Close Other Tabs',
          disabled: others.length === 0,
          run: () => others.forEach((t) => deps.closeTab(t.id)),
        },
      ];
    }
    case 'page':
      return pageItems(deps);
  }
}
