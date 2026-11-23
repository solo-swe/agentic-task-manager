// A question left open on a tab (ATM-700, docs/tabs-and-navigation-design.md §4.2): the app shows one page for all its
// tabs, so a switch of tab takes the page away and with it a form half filled (Submit review, a poke). It is parked
// here, by tab, as it was, and the page takes it back when its tab is shown again, as a macOS sheet stays on its
// window. The app's own pop-ups (Settings, Help, What's new, About) are the dashboard's, above every tab, and aren't
// parked; a menu or a dropdown just closes. In the browser there are no tabs of atm's: nothing is parked.

const parked = new Map<string, Map<string, unknown>>();

/** Keep `value` for tab `tab` under `key`, until it is taken back; none kept without a tab */
export function park(tab: string | undefined, key: string, value: unknown): void {
  if (!tab) return;
  const kept = parked.get(tab) ?? new Map<string, unknown>();
  kept.set(key, value);
  parked.set(tab, kept);
}

/** What tab `tab` kept under `key`, taken back (it is kept no longer); none when nothing was */
export function unpark<T>(tab: string | undefined, key: string): T | undefined {
  const kept = tab ? parked.get(tab) : undefined;
  if (!kept?.has(key)) return undefined;
  const value = kept.get(key) as T;
  kept.delete(key);
  if (kept.size === 0) parked.delete(tab!);
  return value;
}

/** Tab `tab` closed: what it kept goes with it */
export function forgetTab(tab: string): void {
  parked.delete(tab);
}
