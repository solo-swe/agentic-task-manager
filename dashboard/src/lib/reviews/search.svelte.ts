// A page's own search, which the title bar shows (ATM-738, docs/sidebar-overhaul-design.md §6): where a project's
// search is, the same field in the same place on every page that searches. A list (the reviews', ReviewList; the
// Documents page's, ATM-766) offers its search while it is shown; the title bar (Header) shows the field while one is
// offered, and what is typed goes to the list's query.

import type { Snippet } from 'svelte';

/** A list's search: its query's text now, and what to do with a new one; and what sits beside it (All reviews' project
 * filter, ATM-757) */
export interface OfferedSearch {
  readonly q: string;
  search: (q: string) => void;
  readonly aside?: Snippet;
  /** What the field says it searches (the Documents page's, ATM-766): the reviews' by default */
  readonly placeholder?: string;
  readonly title?: string;
}

let offered = $state.raw<OfferedSearch | null>(null);

/** The search offered now, if a reviews list is shown */
export function offeredReviewSearch(): OfferedSearch | null {
  return offered;
}

/** Offer `search` while the list is shown: the returned function takes it back (only it: another list may have
 * offered its own since) */
export function offerReviewSearch(search: OfferedSearch): () => void {
  offered = search;
  return () => {
    if (offered === search) offered = null;
  };
}
