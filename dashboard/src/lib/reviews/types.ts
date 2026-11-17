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

export type ReviewStatus = 'draft' | 'open' | 'live' | 'changes_requested' | 'approved' | 'closed';

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
}

export interface ReviewList {
  reviews: ReviewEntry[];
  you: string;
}

/** What a round was made from */
export type RoundKind =
  { kind: 'range'; base: string; head: string } | { kind: 'working'; base: string } | { kind: 'files' };

export interface Round {
  number: number;
  kind: RoundKind;
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
  view: { round: number; against: number | null; files: ViewFile[] };
  needs_you: boolean;
  you: string;
}

/** What the reviews page lists by: each one given (not empty) must fit */
export interface ListQuery {
  q: string;
  status: ReviewStatus[];
  author: string;
  reviewer: string;
  task: string;
  branch: string;
}

export const NO_QUERY: ListQuery = { q: '', status: [], author: '', reviewer: '', task: '', branch: '' };

export const STATUS_LABELS: Record<ReviewStatus, string> = {
  draft: 'Draft',
  open: 'Open',
  live: 'Live',
  changes_requested: 'Changes requested',
  approved: 'Approved',
  closed: 'Closed',
};
