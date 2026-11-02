// The All projects board (docs/all-projects-kanban-proposal.md §3, §4): every project's board summary (api/projects)
// merged into five columns, each sorted by its own rule and cut at one cap, with "+ N more" per project for the rest.
import type { BoardCard, BoardColumnId, ProjectBoard, ProjectEntry } from './load';

/** Which board (T-340): the active sprints' tasks (the default) or every task, as before */
export type BoardScope = 'active' | 'all';
export const BOARD_SCOPES: BoardScope[] = ['active', 'all'];

/** The project's board for the scope: an older server without the active sprint's board gives the whole one */
export function boardOf(project: ProjectEntry, scope: BoardScope): ProjectBoard | null {
  const summary = project.missing ? null : project.summary;
  if (!summary) return null;
  if (scope === 'active' && 'board_active_sprint' in summary) return summary.board_active_sprint ?? null;
  return summary.board ?? null;
}

/** With the active sprints shown, the projects that have none (and so nothing on the board) */
export function withoutActiveSprint(projects: readonly ProjectEntry[], scope: BoardScope): ProjectEntry[] {
  return scope === 'active'
    ? projects.filter(
        (p) => !p.missing && p.summary && 'board_active_sprint' in p.summary && !p.summary.board_active_sprint,
      )
    : [];
}

/** Cards per column, as the server caps each project's columns */
export const BOARD_CAP = 30;

export interface AllBoardCard {
  project: ProjectEntry;
  card: BoardCard;
}

export interface AllBoardColumn {
  id: BoardColumnId;
  label: string;
  cards: AllBoardCard[];
  /** Cards left out (by a project's own cap or the column's), per project, in project order */
  more: { project: ProjectEntry; count: number }[];
  /** What an empty column says */
  empty: string;
  /** The column's cards in all, shown or not, whatever a search leaves ("N of M") */
  total: number;
}

export const BOARD_COLUMNS: { id: BoardColumnId; label: string; empty: string }[] = [
  { id: 'todo', label: 'To Do', empty: 'Nothing to do' },
  { id: 'active', label: 'In progress', empty: 'Nothing in progress' },
  { id: 'review', label: 'In review', empty: 'Nothing waiting for a review' },
  { id: 'blocked', label: 'Blocked', empty: 'Nothing blocked' },
  { id: 'done', label: 'Done · last 24 h', empty: 'Nothing finished in the last 24 hours' },
];

// A time for sorting, oldest first or newest first; an unknown time sorts last either way
const time = (iso: string | null | undefined) => {
  const t = Date.parse(iso ?? '');
  return Number.isNaN(t) ? Infinity : t;
};
const recent = (iso: string | null | undefined) => {
  const t = Date.parse(iso ?? '');
  return Number.isNaN(t) ? -Infinity : t;
};

/** The column's cards from every project, in its order (§4): To Do by project (most recently active first) then the
 * loop's order; in progress longest running first; review longest waiting first; blocked on you first, then longest
 * waiting; done most recent first. */
function sortCards(id: BoardColumnId, cards: AllBoardCard[]): AllBoardCard[] {
  const sorted = [...cards];
  switch (id) {
    case 'todo':
      // Stable: within a project, the server's (the loop's) order stays
      return sorted.sort(
        (a, b) => recent(b.project.summary?.last_activity) - recent(a.project.summary?.last_activity) || 0,
      );
    case 'active':
    case 'review':
      return sorted.sort((a, b) => time(a.card.since) - time(b.card.since) || 0);
    case 'blocked':
      return sorted.sort(
        (a, b) =>
          Number(a.card.waiting_on !== 'user') - Number(b.card.waiting_on !== 'user') ||
          time(a.card.since) - time(b.card.since) ||
          0,
      );
    case 'done':
      return sorted.sort((a, b) => recent(b.card.completed_at) - recent(a.card.completed_at) || 0);
  }
}

/** A search's matches (api/search, T-261): task IDs per project id; null while not searching */
export type BoardMatch = ReadonlyMap<string, ReadonlySet<string>> | null;

/**
 * The board of `projects` (those the page shows), of the scope's board (T-340): projects without one (a missing one,
 * an older server, no active sprint) add nothing. With `match`, only the board's cards among the search's results,
 * and no "+ N more" (the search looks at the board's cards only); `total` still counts the whole column, for "N of M".
 */
export function allBoard(
  projects: readonly ProjectEntry[],
  cap = BOARD_CAP,
  match: BoardMatch = null,
  scope: BoardScope = 'all',
): AllBoardColumn[] {
  const withBoard = projects.flatMap((project) => {
    const board = boardOf(project, scope);
    return board ? [{ project, board }] : [];
  });
  // The active sprints' board holds all their tasks, every one done too, not the last 24 hours' (T-349)
  const columns =
    scope === 'active'
      ? BOARD_COLUMNS.map((c) => (c.id === 'done' ? { ...c, label: 'Done', empty: 'Nothing done in it yet' } : c))
      : BOARD_COLUMNS;
  return columns.map(({ id, label, empty }) => {
    const all = sortCards(
      id,
      withBoard.flatMap(({ project, board }) => board[id].cards.map((card) => ({ project, card }))),
    );
    const total = withBoard.reduce((n, { board }) => n + board[id].count, 0);
    if (match) {
      const cards = all.filter((c) => match.get(c.project.id)?.has(c.card.id)).slice(0, cap);
      return { id, label, cards, more: [], empty: 'No match', total };
    }
    const cards = all.slice(0, cap);
    const more = withBoard
      .map(({ project, board }) => ({
        project,
        count: board[id].count - cards.filter((c) => c.project === project).length,
      }))
      .filter((m) => m.count > 0);
    return { id, label, cards, more, empty, total };
  });
}

export interface AllBoardLane {
  project: ProjectEntry;
  columns: AllBoardColumn[];
}

/** Grouped by project (T-260, §2): a lane per project with the five columns, the most recently active first; a
 * project with nothing on the board (or, searching, nothing matching) gets no lane. */
export function allBoardLanes(
  projects: readonly ProjectEntry[],
  cap = BOARD_CAP,
  match: BoardMatch = null,
  scope: BoardScope = 'all',
): AllBoardLane[] {
  return projects
    .filter((p) => boardOf(p, scope))
    .sort((a, b) => recent(b.summary?.last_activity) - recent(a.summary?.last_activity) || 0)
    .map((project) => ({ project, columns: allBoard([project], cap, match, scope) }))
    .filter((lane) => lane.columns.some((c) => c.cards.length || c.more.length));
}
