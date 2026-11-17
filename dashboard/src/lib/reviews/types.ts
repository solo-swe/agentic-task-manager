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
