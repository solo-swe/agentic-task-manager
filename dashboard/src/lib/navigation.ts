// Moving between addresses (T-429). The page's layout hands in SvelteKit's router (`goto`, `page.state`), so its
// history entries are the router's and Back and links go through it; without one (the unit tests) the browser's
// history is used directly. The address written last is kept, so code that reads it right after writing sees the
// new one, though the router's navigation finishes a moment later.
//
// Every entry pushed remembers the address it was reached from (ATM-686), so a page's own back button can go Back,
// saying where to (docs/tabs-and-navigation-design.md §4.3). A replaced entry keeps what it had.
//
// In the app each title-bar tab has its own history (ATM-694, §4.1): `useTabHistory` hands in the tabs, `navigate`
// steps in the active tab's history, `back` and `forward` walk it, each entry's scroll restored, and the page's own
// history stays one entry deep (every move replaces it). A switch of tab shows the other tab as it was left
// (`showTab`), adding nothing. In the browser, a browser tab is the tab: its own history, as before.
import { addressOf } from './address';

/** What the document viewer marks its own history entries with (state.svelte.ts documentDepth), and where an entry
 * was reached from in its tab: `atmFrom`, the address of the page before this one, `atmSteps` entries back (1 when
 * missing; more when this page has had something open over it, or another file or round, since); `atmPrev`, the
 * address of the entry just before this one, the one it was pushed from (ATM-696: closing what is open over a page goes
 * Back when that is the page under it) */
export type EntryState = {
  atmDocument?: boolean;
  atmDocumentDepth?: number;
  atmFrom?: string;
  atmSteps?: number;
  atmPrev?: string;
};

/** An address's page: its path, without what is open over it (the query) */
const pathOf = (address: string) => address.split(/[?#]/)[0];

export interface Router {
  go(address: string, options: { replace: boolean; state: EntryState }): void;
  /** The current history entry's state */
  state(): EntryState;
  /** `steps` entries back, as the browser's Back */
  back(steps: number): void;
}

const browserRouter: Router = {
  go(address, { replace, state }) {
    if (replace) history.replaceState(state, '', address);
    else history.pushState(state, '', address);
  },
  state: () => (history.state as EntryState | null) ?? {},
  back: (steps) => history.go(-steps),
};

let router: Router = browserRouter;
let written: string | null = null;

export function useRouter(next: Router) {
  router = next;
}

/** Where a page was scrolled: each scrolled element with an ID, by it */
export type ScrollMemo = Record<string, { top: number; left: number }>;

/** An entry of a tab's own history (ATM-694): its address, its state, and where its page was scrolled when it was
 * left */
export interface HistoryEntry {
  address: string;
  state: EntryState;
  scroll?: ScrollMemo;
}

/** A tab's own history (the app's tabs, app-tabs.svelte.ts): in the app the page's history stays one entry deep and
 * Back and Forward walk this instead, the active tab's (docs/tabs-and-navigation-design.md §4.1) */
export interface TabHistory {
  push(entry: HistoryEntry): void;
  replace(entry: HistoryEntry): void;
  /** The entry `delta` steps away, now current; none past either end */
  go(delta: number): HistoryEntry | null;
  canGo(delta: number): boolean;
  entry(): HistoryEntry | null;
}

let tabHistory: TabHistory | null = null;

/** In the app: navigate, Back and Forward go through the active tab's history (ATM-694) */
export function useTabHistory(next: TabHistory | null) {
  tabHistory = next;
}

/** The most scrolled elements an entry remembers */
const SCROLL_LIMIT = 20;

/** Where the page is scrolled now: the elements with an ID that are scrolled */
export function scrollOf(root: ParentNode = document): ScrollMemo {
  const memo: ScrollMemo = {};
  let kept = 0;
  for (const el of root.querySelectorAll<HTMLElement>('[id]')) {
    if (el.scrollTop === 0 && el.scrollLeft === 0) continue;
    memo[el.id] = { top: el.scrollTop, left: el.scrollLeft };
    if (++kept >= SCROLL_LIMIT) break;
  }
  return memo;
}

/** The scroll to restore once the page has drawn the entry gone to (arrived) */
let pendingScroll: ScrollMemo | null = null;

/** Scroll the page as `memo` says, once its elements are there: now, and again after the next frames (a view drawn
 * late) */
export function restoreScroll(memo: ScrollMemo, root: Document = document) {
  const put = () => {
    for (const [id, { top, left }] of Object.entries(memo)) {
      const el = root.getElementById(id);
      if (el) {
        el.scrollTop = top;
        el.scrollLeft = left;
      }
    }
  };
  put();
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => requestAnimationFrame(put));
}

/** The shown entry of the active tab keeps where its page is scrolled, before the page leaves it (a step, Back, a
 * switch of tab) */
export function rememberScroll() {
  const entry = tabHistory?.entry();
  if (entry && typeof document !== 'undefined') tabHistory!.replace({ ...entry, scroll: scrollOf() });
}

/** Show `entry` of the tab's history: the page's one entry replaced by it, its scroll restored once it is drawn */
function show(entry: HistoryEntry) {
  written = entry.address;
  pendingScroll = entry.scroll ?? null;
  router.go(entry.address, { replace: true, state: entry.state });
  if (router === browserRouter) arrived();
}

/** Show the active tab as it was left (a switch of tab, ATM-694): no step in any history */
export function showTab() {
  const entry = tabHistory?.entry();
  if (entry) show(entry);
}

/** The address shown: the one written last until the router has moved there, then the location's */
export function currentAddress(): string {
  return written ?? addressOf(location);
}

/** The router has arrived (a navigation of its own, Back, a link): the location is the address again, and an entry
 * gone back or forward to is scrolled as it was left */
export function arrived() {
  written = null;
  if (pendingScroll) {
    const memo = pendingScroll;
    pendingScroll = null;
    restoreScroll(memo);
  }
}

/** Where an entry pushed at `address` now is reached from: the one shown; or, on the same page (its path), where
 * that page was reached from, a step further back */
export function originOf(address: string, shown: string, shownState: EntryState): EntryState {
  if (pathOf(address) !== pathOf(shown)) return { atmFrom: shown };
  if (!shownState.atmFrom) return {};
  return { atmFrom: shownState.atmFrom, atmSteps: (shownState.atmSteps ?? 1) + 1 };
}

/** Go to `address`: a new history entry, reached from the one shown, or with `replace` this one changed (no new
 * entry for Back; it keeps where it was reached from). `fresh`: a new entry with no way back in its tab (another of
 * the app's tabs shown) */
export function navigate(
  address: string,
  { replace = false, fresh = false, state = {} }: { replace?: boolean; fresh?: boolean; state?: EntryState } = {},
) {
  if (address === currentAddress() && !replace) return;
  const { atmFrom, atmSteps, atmPrev } = router.state();
  const where: EntryState = fresh
    ? {}
    : replace
      ? { ...(atmFrom ? { atmFrom } : {}), ...(atmSteps ? { atmSteps } : {}), ...(atmPrev ? { atmPrev } : {}) }
      : { ...originOf(address, currentAddress(), router.state()), atmPrev: currentAddress() };
  const entryState = { ...where, ...state };
  if (tabHistory) {
    // The app (ATM-694): a step in the active tab's history; the page's own history stays one entry deep
    if (replace) {
      const shown = tabHistory.entry();
      tabHistory.replace({ address, state: entryState, ...(shown?.scroll ? { scroll: shown.scroll } : {}) });
    } else {
      rememberScroll();
      tabHistory.push({ address, state: entryState });
    }
    written = address;
    router.go(address, { replace: true, state: entryState });
    if (router === browserRouter) written = null;
    return;
  }
  written = address;
  router.go(address, { replace, state: entryState });
  if (router === browserRouter) written = null;
}

export function entryState(): EntryState {
  return router.state();
}

/** `steps` entries back (negative: forward) in this tab's history: the app's tab's own, else the browser's. Whether
 * there was one to go to (the browser's always says yes) */
export function back(steps = 1): boolean {
  if (!tabHistory) {
    router.back(steps);
    return true;
  }
  if (!tabHistory.canGo(-steps)) return false;
  rememberScroll();
  const entry = tabHistory.go(-steps);
  if (entry) show(entry);
  return entry !== null;
}

/** One entry forward in this tab's history */
export function forward(): boolean {
  return back(-1);
}

/** Whether this tab can go Back (`-1`) or Forward (`1`); in the browser, its own buttons know */
export function canGo(delta: number): boolean {
  return tabHistory ? tabHistory.canGo(delta) : true;
}

/** Where Back goes in this tab, out of this page: the page it was reached from, and how many entries back; `null`
 * for one opened on its own (a new tab, another tab shown, a link from elsewhere, the first page) */
export function backEntry(): { address: string; steps: number } | null {
  const { atmFrom, atmSteps } = router.state();
  return atmFrom ? { address: atmFrom, steps: atmSteps ?? 1 } : null;
}

/** Leave what is open over this page for `under`, the page itself (ATM-696): Back when the entry before is that page,
 * so a Back after it never lands on the same page twice; else this entry replaced by it */
export function closeTo(under: string) {
  if (router.state().atmPrev === under && back(1)) return;
  navigate(under, { replace: true });
}

/** Back out of this page to the one it was reached from (the page's back button, when `backEntry` names one) */
export function goBack() {
  const entry = backEntry();
  if (entry) back(entry.steps);
}
