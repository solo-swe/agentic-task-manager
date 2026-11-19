// A review's chains of comments, as atm prints them (`atm review comments CR-7 --json`; crates/atm-review's model,
// docs/code-review-design.md §3.4, §3.6). The review tool's own types: it knows a task only as a key.

export type Side = 'new' | 'old';

/** What a chain is about: a round, a file and its lines on one side; or the review as a whole (no file) */
export interface Anchor {
  round: number;
  path: string | null;
  side: Side;
  lines: [number, number] | null;
  quote: string | null;
}

/** A comment, with the replies to it nested under it; `resolved` is a top-level comment's box, none for a reply */
export interface ReviewComment {
  id: string;
  author: string;
  at: string;
  body: string;
  resolved: boolean | null;
  /** A poke (ATM-611): the user's nudge on the whole review, never open; absent for any other comment */
  poke?: boolean;
  replies: ReviewComment[];
}

export interface Chain {
  id: string;
  anchor: Anchor;
  comments: ReviewComment[];
}

/** Lines picked to start a chain: on one side, with the text they show */
export interface Picked {
  side: Side;
  lines: [number, number];
  quote: string;
}

/** Where a review stands; `waiting` is waiting on its reviewers (ATM-613: `open` until 0.41.0) */
export type ReviewStatus = 'draft' | 'waiting' | 'live' | 'changes_requested' | 'approved' | 'closed';

/** A status filter: one status, or `open`, every review not decided yet (draft, waiting, live, sent back) */
export type StatusFilter = ReviewStatus | 'open';

/** The statuses of an open review, one not decided yet (as atm-review's `Status::OPEN`) */
export const OPEN_STATUSES: readonly ReviewStatus[] = ['draft', 'waiting', 'live', 'changes_requested'];

/** A review's header (`review.md`'s): what lists and searches need */
export interface ReviewHeader {
  id: string;
  reference: string;
  number: number;
  title: string;
  status: ReviewStatus;
  tasks: string[];
  author: string | null;
  reviewers: string[];
  created: string;
  updated: string;
}

/** One review on the reviews page (`GET …/reviews`) */
export interface ReviewEntry {
  review: ReviewHeader;
  branch: string | null;
  chains: number;
  open: number;
  needs_you: boolean;
  /** What it waits on you for (ATM-606): its decision or round, or an answer since you last wrote; none, `null`.
   * Optional, for an older atm's lists */
  waiting?: 'review' | 'answered' | null;
  /** When it began waiting on its reviewers, its latest round's time (ATM-610). Optional, for an older atm's lists */
  waiting_since?: string | null;
  /** The project it is in, on a list of several projects' reviews (ATM-575, the overview's Reviews); the page's own */
  project?: { id: string; name: string };
}

/** A review's folder atm can't read (ATM-617): a status a newer atm wrote, a broken header */
export interface UnreadableReview {
  folder: string;
  reference: string;
  problem: string;
}

export interface ReviewList {
  reviews: ReviewEntry[];
  /** Optional, for an older atm's lists */
  unreadable?: UnreadableReview[];
  you: string;
}

/** What a round was made from */
export type RoundKind =
  { kind: 'range'; base: string; head: string } | { kind: 'working'; base: string } | { kind: 'files' };

export interface Round {
  number: number;
  kind: RoundKind;
  /** The submodule its commits are in (its folder), none for the project's own (ATM-583) */
  submodule?: string | null;
  files: { path: string; binary: boolean }[];
  by: string;
  at: string;
}

/** A file of the view shown: its patch, and the commits or trees its two sides are read from (none for a side it
 * doesn't have) */
export interface ViewFile {
  path: string;
  old_path: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  binary: boolean;
  /** A submodule this change moved (ATM-583): the commits it was and is at (none where added or removed), and its
   * repository's web address when it has one; none for a file */
  submodule?: { from: string | null; to: string | null; repository: string | null } | null;
  patch: string;
  before: string | null;
  after: string | null;
}

/** A chain, with where it shows in the round shown: its lines there, or drifted (made on another round, its text
 * gone), with the round and lines it was made on */
export interface PlacedChain extends Chain {
  placed: [number, number] | null;
  drifted: { round: number; lines: [number, number] | null } | null;
}

/** A review's page for one view (`GET …/reviews/<CR>?round=&against=`) */
export interface ReviewPageData {
  review: ReviewHeader;
  summary: string;
  branch: string | null;
  rounds: Round[];
  chains: PlacedChain[];
  /** `now`: a live review's working copy as it is now (ATM-576); its `round` is the one after `rounds` when it changed
   * since the last checkpoint (a comment on it makes it one) */
  view: { round: number; against: number | null; now?: boolean; files: ViewFile[] };
  needs_you: boolean;
  you: string;
}

/** What the reviews page lists by: each one given (not empty) must fit */
export interface ListQuery {
  q: string;
  /** `open` among them: every review not decided yet (the server expands it) */
  status: StatusFilter[];
  author: string;
  reviewer: string;
  task: string;
  branch: string;
}

export const NO_QUERY: ListQuery = { q: '', status: [], author: '', reviewer: '', task: '', branch: '' };

export const STATUS_LABELS: Record<ReviewStatus, string> = {
  draft: 'Draft',
  waiting: 'Waiting for review',
  live: 'Live',
  changes_requested: 'Changes requested',
  approved: 'Approved',
  closed: 'Closed',
};
