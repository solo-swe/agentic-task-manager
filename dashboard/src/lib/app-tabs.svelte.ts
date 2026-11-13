// The atm app's tabs (T-514, docs/app-desktop-ui-design.md §3): a tab is an address. The window has one page and one
// dashboard state; switching tabs navigates to the tab's address, and the active tab follows the page as it moves (a
// link, Back, a view). Kept per window for its page's life, and over a reload (sessionStorage is the window's own).
import { addressParams } from './address';
import type { KeptTabs } from './api/shell';

/** A tab: its ID (never reused in the window) and the address it shows */
export interface AppTab {
  id: string;
  address: string;
}

/** Where the tabs are kept over a reload */
const STORE = 'atm-app-tabs';

/** What a tab shows: the project its address names, if any */
export function tabProject(address: string): string | null {
  return addressParams(address).get('project');
}

export class AppTabs {
  tabs = $state<AppTab[]>([]);
  active = $state('');
  #next = 1;
  #storage: Storage | null;

  constructor(storage: Storage | null = null) {
    this.#storage = storage;
  }

  /** Start with the tabs kept over a reload; else with those the app brings back as it starts (T-518), answering the
   * active one's address, to go to; else with one tab at `address` */
  start(address: string, restored: KeptTabs | null = null): string | null {
    try {
      const kept = JSON.parse(this.#storage?.getItem(STORE) ?? 'null') as { tabs: AppTab[]; active: string } | null;
      if (kept && kept.tabs.length > 0 && kept.tabs.some((t) => t.id === kept.active)) {
        this.tabs = kept.tabs;
        this.active = kept.active;
        this.#next = Math.max(...kept.tabs.map((t) => Number(t.id.replace(/\D/g, '')) || 0)) + 1;
        this.follow(address);
        return null;
      }
    } catch {
      // nothing kept, or nothing readable: a fresh start
    }
    if (restored && restored.addresses.length > 0) {
      this.tabs = restored.addresses.map((a) => ({ id: `t${this.#next++}`, address: a }));
      this.active = (this.tabs[restored.active] ?? this.tabs[0]).id;
      this.#save();
      return this.current!.address;
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

  /** The page moved: the active tab shows where it is now */
  follow(address: string) {
    const tab = this.current;
    if (tab && tab.address !== address) {
      tab.address = address;
      this.#save();
    }
  }

  /** A new tab at `address`, after the active one, and active. Its address, to go to */
  open(address: string): string {
    const tab = { id: `t${this.#next++}`, address };
    const at = this.tabs.findIndex((t) => t.id === this.active);
    this.tabs.splice(at < 0 ? this.tabs.length : at + 1, 0, tab);
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

/** The window's tabs (the app's; the browser has none) */
export const appTabs = new AppTabs(typeof sessionStorage === 'undefined' ? null : sessionStorage);
