// The All projects search (T-261), shared by the Omniboard and All's List (ATM-752): the server searches every
// project's tasks (api/search, their descriptions and notes too, which the summaries' cards don't carry), and the page
// keeps its cards among the results. Asked once per pause in typing, and again when the cards change (not on every
// poll).
import { boardOf, BOARD_COLUMNS, type BoardMatch } from '../allboard';
import { searchProjects, type ProjectEntry } from '../load';
import { dashboard } from '../state.svelte';

export const SEARCH_PAUSE_MS = 300;

/** The search's matches for the cards of `projects()`, kept current while the calling component lives: null while
 * nothing is searched */
export function boardSearch(projects: () => readonly ProjectEntry[]): { readonly match: BoardMatch } {
  let match = $state<BoardMatch>(null);
  const cards = $derived(
    projects()
      .flatMap((p) =>
        BOARD_COLUMNS.flatMap(
          ({ id }) => boardOf(p, dashboard.boardScope)?.[id].cards.map((card) => `${p.id}/${card.id}`) ?? [],
        ),
      )
      .join(' '),
  );
  $effect(() => {
    const query = dashboard.boardQuery.trim();
    void cards;
    if (!query) {
      match = null;
      return;
    }
    let current = true;
    const timer = setTimeout(
      () =>
        searchProjects(query)
          .then((found) => current && (match = found))
          .catch((e) => console.error('Searching the projects failed:', e)),
      SEARCH_PAUSE_MS,
    );
    return () => {
      current = false;
      clearTimeout(timer);
    };
  });
  return {
    get match() {
      return match;
    },
  };
}
