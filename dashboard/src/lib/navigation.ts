// Moving between addresses (T-429). The page's layout hands in SvelteKit's router (`goto`, `page.state`), so its
// history entries are the router's and Back and links go through it; without one (the unit tests) the browser's
// history is used directly. The address written last is kept, so code that reads it right after writing sees the
// new one, though the router's navigation finishes a moment later.
import { addressOf } from './address';

/** What the document viewer marks its own history entries with (state.svelte.ts documentDepth) */
export type EntryState = { atmDocument?: boolean; atmDocumentDepth?: number };

export interface Router {
  go(address: string, options: { replace: boolean; state: EntryState }): void;
  /** The current history entry's state */
  state(): EntryState;
}

const browserRouter: Router = {
  go(address, { replace, state }) {
    if (replace) history.replaceState(state, '', address);
    else history.pushState(state, '', address);
  },
  state: () => (history.state as EntryState | null) ?? {},
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

/** Go to `address`: a new history entry, or with `replace` this one changed (no new entry for Back) */
export function navigate(
  address: string,
  { replace = false, state = {} }: { replace?: boolean; state?: EntryState } = {},
) {
  if (address === currentAddress() && !replace) return;
  written = address;
  router.go(address, { replace, state });
  if (router === browserRouter) written = null;
}

export function entryState(): EntryState {
  return router.state();
}
