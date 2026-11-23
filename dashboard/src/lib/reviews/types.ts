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
  /** One of the user's drafts (ATM-612), not sent yet: only they see it; absent for any other comment */
  draft?: boolean;
  /** Whether you may change its text (ATM-723): atm's rule (yours, not a poke, the review not ended). A draft's is
   * the drafts' own (absent: only the server's comments carry it) */
  editable?: boolean;
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
  /** Decided: approved or closed, as atm says it (ATM-714) */
  ended: boolean;
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
  /** Whether the view holds the increment of the round it was made on (ATM-725, atm's): placed in the view when it
   * does, else listed as on another round */
  in_view: boolean;
}

/** One of the user's drafts on the review (ATM-612), as the page has it: a comment placed in the round shown as a chain
 * is, or a reply */
export type PlacedDraft = { id: string; at: string; body: string } & (
  | {
      kind: 'comment';
      path: string | null;
      lines: [number, number] | null;
      side: Side;
      quote: string | null;
      /** None: on what a live review is when it is sent */
      round: number | null;
      placed?: [number, number] | null;
      drifted?: { round: number; lines: [number, number] | null } | null;
      in_view?: boolean;
    }
  | { kind: 'reply'; to: string }
);

/** A review's page for one view (`GET …/reviews/<CR>?round=&against=`) */
export interface ReviewPageData {
  review: ReviewHeader;
  summary: string;
  branch: string | null;
  rounds: Round[];
  chains: PlacedChain[];
  /** The user's drafts on it (ATM-612). Optional, for an older atm's pages */
  drafts?: PlacedDraft[];
  /** `now`: a live review's working copy as it is now (ATM-576); its `round` is the one after `rounds` when it changed
   * since the last checkpoint (a comment on it makes it one) */
  view: { round: number; against: number | null; now?: boolean; files: ViewFile[] };
  needs_you: boolean;
  you: string;
  /** What this reader may do, and why not (ATM-633, docs/task-review-state-machine.md §5.2): the server's state
   * machine answering for each action, so the page shows a button for what it allows and never repeats the rules
   * (ATM-723: the page has no rule of its own) */
  can: Record<ReviewAction, { ok: boolean; why?: string }>;
}

/** The actions a review's page offers, as the server's `can` names them */
export type ReviewAction = 'approve' | 'request_changes' | 'comment' | 'round' | 'close';

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
