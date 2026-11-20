// Moving between addresses (T-429). The page's layout hands in SvelteKit's router (`goto`, `page.state`), so its
// history entries are the router's and Back and links go through it; without one (the unit tests) the browser's
// history is used directly. The address written last is kept, so code that reads it right after writing sees the
// new one, though the router's navigation finishes a moment later.
//
// Every entry pushed remembers the address it was reached from (ATM-686), so a page's own back button can go Back,
// saying where to (docs/tabs-and-navigation-design.md §4.3). A replaced entry keeps what it had; one a switch of the
// app's tabs pushes (`fresh`) has none: the entry before it is another tab's.
import { addressOf } from './address';

/** What the document viewer marks its own history entries with (state.svelte.ts documentDepth), and where an entry
 * was reached from in its tab: `atmFrom`, the address of the page before this one, `atmSteps` entries back (1 when
 * missing; more when this page has had something open over it, or another file or round, since) */
export type EntryState = { atmDocument?: boolean; atmDocumentDepth?: number; atmFrom?: string; atmSteps?: number };

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

/** The address shown: the one written last until the router has moved there, then the location's */
export function currentAddress(): string {
  return written ?? addressOf(location);
}

/** The router has arrived (a navigation of its own, Back, a link): the location is the address again */
export function arrived() {
  written = null;
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
  const { atmFrom, atmSteps } = router.state();
  const where: EntryState = fresh
    ? {}
    : replace
      ? { ...(atmFrom ? { atmFrom } : {}), ...(atmSteps ? { atmSteps } : {}) }
      : originOf(address, currentAddress(), router.state());
  written = address;
  router.go(address, { replace, state: { ...where, ...state } });
  if (router === browserRouter) written = null;
}

export function entryState(): EntryState {
  return router.state();
}

/** Where Back goes in this tab, out of this page: the page it was reached from, and how many entries back; `null`
 * for one opened on its own (a new tab, another tab shown, a link from elsewhere, the first page) */
export function backEntry(): { address: string; steps: number } | null {
  const { atmFrom, atmSteps } = router.state();
  return atmFrom ? { address: atmFrom, steps: atmSteps ?? 1 } : null;
}

/** Back out of this page to the one it was reached from (the page's back button, when `backEntry` names one) */
export function goBack() {
  const back = backEntry();
  if (back) router.back(back.steps);
}
