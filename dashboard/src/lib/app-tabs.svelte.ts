// The atm app's tabs (T-514, docs/tabs-and-navigation-design.md §4.1): a tab is a place with its own history. The
// window has one page and one dashboard state; switching tabs shows the tab as it was left (adding to no history), and
// Back and Forward walk the active tab's (ATM-694). Kept per window for its page's life, and over a reload
// (sessionStorage is the window's own).
import { addressParams } from './address';
import type { KeptTabs } from './api/shell';
import type { HistoryEntry, TabHistory } from './navigation';

/** A tab: its ID (never reused in the window), the address it shows, and its own history (ATM-694,
 * docs/tabs-and-navigation-design.md §4.1): the entries it went through and where it is in them. `address` is always
 * the current entry's */
export interface AppTab {
  id: string;
  address: string;
  history: { entries: HistoryEntry[]; at: number };
}

/** A tab's history, starting at `address` */
const historyAt = (address: string): AppTab['history'] => ({ entries: [{ address, state: {} }], at: 0 });

/** A tab kept by an older page (no history): its address its one entry */
function withHistory(tab: Partial<AppTab> & { id: string; address: string }): AppTab {
  const history = tab.history;
  const usable =
    history &&
    Array.isArray(history.entries) &&
    history.entries.length > 0 &&
    history.at >= 0 &&
    history.at < history.entries.length;
  return { id: tab.id, address: tab.address, history: usable ? history : historyAt(tab.address) };
}

/** How many entries a tab's history keeps: the oldest go first */
const HISTORY_LIMIT = 50;

/** Where the tabs are kept over a reload */
const STORE = 'atm-app-tabs';

/** What a tab shows: the project its address names, if any */
export function tabProject(address: string): string | null {
  return addressParams(address).get('project');
}

export class AppTabs implements TabHistory {
  tabs = $state<AppTab[]>([]);
  active = $state('');
  #next = 1;
  #storage: Storage | null;

  constructor(storage: Storage | null = null) {
    this.#storage = storage;
  }

  /** Start with those the app brings back (as it starts, T-518, or after its Reload, ATM-584, which loads the page at
   * the app's root), answering the active one's address, to go to; else with the tabs kept over a reload of the page
   * itself, the active one following `address`; else with one tab at `address` */
  start(address: string, restored: KeptTabs | null = null): string | null {
    if (restored && restored.addresses.length > 0) {
      this.tabs = restored.addresses.map((a) => ({ id: `t${this.#next++}`, address: a, history: historyAt(a) }));
      this.active = (this.tabs[restored.active] ?? this.tabs[0]).id;
      this.#save();
      return this.current!.address;
    }
    try {
      const kept = JSON.parse(this.#storage?.getItem(STORE) ?? 'null') as { tabs: AppTab[]; active: string } | null;
      if (kept && kept.tabs.length > 0 && kept.tabs.some((t) => t.id === kept.active)) {
        this.tabs = kept.tabs.map(withHistory);
        this.active = kept.active;
        this.#next = Math.max(...kept.tabs.map((t) => Number(t.id.replace(/\D/g, '')) || 0)) + 1;
        this.follow(address);
        return null;
      }
    } catch {
      // nothing kept, or nothing readable: a fresh start
    }
    this.tabs = [];
    this.open(address);
    return null;
  }

  /** The tabs as the app keeps them for the next start (T-518): their addresses, in order, and the active one's place */
  get kept(): KeptTabs {
    return {
      addresses: this.tabs.map((t) => t.address),
      active: Math.max(
        0,
        this.tabs.findIndex((t) => t.id === this.active),
      ),
    };
  }

  /** The active tab */
  get current(): AppTab | undefined {
    return this.tabs.find((t) => t.id === this.active);
  }

  /** The page moved without the tab's history (its first address written another way, a move of the router's own):
   * the active tab's current entry shows where it is now */
  follow(address: string) {
    const tab = this.current;
    if (tab && tab.address !== address) {
      tab.address = address;
      tab.history.entries[tab.history.at] = { ...tab.history.entries[tab.history.at], address };
      this.#save();
    }
  }

  // The active tab's history (ATM-694): navigation.ts moves through it in the app, where the page's own history stays
  // one entry deep

  /** A step: the entry after the current one, those that were after it gone (as a browser's Forward is) */
  push(entry: HistoryEntry) {
    const tab = this.current;
    if (!tab) return;
    const entries = [...tab.history.entries.slice(0, tab.history.at + 1), entry].slice(-HISTORY_LIMIT);
    tab.history = { entries, at: entries.length - 1 };
    tab.address = entry.address;
    this.#save();
  }

  /** The current entry, changed in place */
  replace(entry: HistoryEntry) {
    const tab = this.current;
    if (!tab) return;
    tab.history.entries[tab.history.at] = entry;
    tab.address = entry.address;
    this.#save();
  }

  /** The entry `delta` steps from the current one (−1 Back, 1 Forward), now current; none past either end */
  go(delta: number): HistoryEntry | null {
    const tab = this.current;
    const to = tab ? tab.history.at + delta : -1;
    if (!tab || delta === 0 || to < 0 || to >= tab.history.entries.length) return null;
    tab.history = { ...tab.history, at: to };
    tab.address = tab.history.entries[to].address;
    this.#save();
    return tab.history.entries[to];
  }

  /** Whether the active tab can go `delta` steps (Back disabled at its first entry) */
  canGo(delta: number): boolean {
    const tab = this.current;
    const to = tab ? tab.history.at + delta : -1;
    return !!tab && delta !== 0 && to >= 0 && to < tab.history.entries.length;
  }

  /** The active tab's current entry */
  entry(): HistoryEntry | null {
    const tab = this.current;
    return tab ? tab.history.entries[tab.history.at] : null;
  }

  /** A new tab at `address`, after the active one, and active, its history starting there (Back disabled until it
   * moves). Its address, to go to */
  open(address: string): string;
  /** In the `background` (⌘-click, a middle click; ATM-698): right of the active one, which stays shown: nothing to
   * go to */
  open(address: string, background: boolean): string | undefined;
  open(address: string, background = false): string | undefined {
    const tab: AppTab = { id: `t${this.#next++}`, address, history: historyAt(address) };
    const at = this.tabs.findIndex((t) => t.id === this.active);
    this.tabs.splice(at < 0 ? this.tabs.length : at + 1, 0, tab);
    if (background && at >= 0) {
      this.#save();
      return undefined;
    }
    this.active = tab.id;
    this.#save();
    return address;
  }

  /** Make tab `id` active. Its address, to go to; none for no such tab */
  select(id: string): string | null {
    const tab = this.tabs.find((t) => t.id === id);
    if (!tab) return null;
    this.active = id;
    this.#save();
    return tab.address;
  }

  /** The tab `step` places after the active one (−1 before), round the end: its address, to go to */
  step(step: 1 | -1): string | null {
    if (this.tabs.length < 2) return null;
    const at = this.tabs.findIndex((t) => t.id === this.active);
    const to = (at + step + this.tabs.length) % this.tabs.length;
    return this.select(this.tabs[to].id);
  }

  /** Close tab `id`. When it was active, the one taking its place (the next, else the one before) is, and its address
   * is the answer, to go to; `null` when it was the last tab (the window closes); `undefined` when nothing else
   * changes */
  close(id: string): string | null | undefined {
    const at = this.tabs.findIndex((t) => t.id === id);
    if (at < 0) return undefined;
    this.tabs.splice(at, 1);
    if (this.tabs.length === 0) {
      this.#save();
      return null;
    }
    if (id !== this.active) {
      this.#save();
      return undefined;
    }
    return this.select(this.tabs[Math.min(at, this.tabs.length - 1)].id);
  }

  /** Move tab `id` to place `to` (a drag) */
  move(id: string, to: number) {
    const at = this.tabs.findIndex((t) => t.id === id);
    if (at < 0) return;
    const [tab] = this.tabs.splice(at, 1);
    this.tabs.splice(Math.max(0, Math.min(to, this.tabs.length)), 0, tab);
    this.#save();
  }

  #save() {
    try {
      this.#storage?.setItem(STORE, JSON.stringify({ tabs: this.tabs, active: this.active }));
    } catch {
      // a full or blocked storage: the tabs last until the page goes
    }
  }
}

/** The window's session storage, or null where there is none or site data is blocked: reading `sessionStorage` itself
 * throws then, and at module level that kept the page from loading (ATM-676, audit D5) */
export function sessionStore(): Storage | null {
  try {
    return typeof sessionStorage === 'undefined' ? null : sessionStorage;
  } catch {
    return null;
  }
}

/** The window's tabs (the app's; the browser has none) */
export const appTabs = new AppTabs(sessionStore());
