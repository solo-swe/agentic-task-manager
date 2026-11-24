// What arrived on a review since its page opened (ATM-733): comments, replies and rounds the reader hasn't seen. The
// page remembers what it showed first; what comes with each refresh after it (an agent's reply, a new round) is new
// until seen: a comment once it has been in view, a round once the Diffs show it. Badges say where: each sub-tab with
// how many new comments are under it, the round picker with the new round. The reader's own comments and drafts are
// never new to them.
import { getContext, setContext } from 'svelte';
import type { Chain, ReviewComment } from './types';

/** What the reader has seen of the review: comment IDs, and the last round */
export interface Seen {
  comments: ReadonlySet<string>;
  round: number;
}

/** Every comment of `chains`, replies too, with whether its chain is on the review as a whole (no file) */
function everyComment(chains: readonly Chain[]): { comment: ReviewComment; general: boolean }[] {
  const walk = (c: ReviewComment, general: boolean): { comment: ReviewComment; general: boolean }[] => [
    { comment: c, general },
    ...c.replies.flatMap((r) => walk(r, general)),
  ];
  return chains.flatMap((ch) => ch.comments.flatMap((c) => walk(c, ch.anchor.path === null)));
}

/** What the page showed when it opened: all of it seen */
export function seenAtFirst(chains: readonly Chain[], rounds: readonly number[]): Seen {
  return {
    comments: new Set(everyComment(chains).map(({ comment }) => comment.id)),
    round: Math.max(0, ...rounds),
  };
}

/** What is new: the comments the reader hasn't seen (not their own, not a draft), by where they are, and the newest
 * round when it is past the one seen */
export interface Unseen {
  comments: ReadonlySet<string>;
  /** On the review as a whole: the Overview's conversation */
  general: number;
  /** On a file: the Diffs */
  onFiles: number;
  /** The newest round, when the reader hasn't seen it; else null */
  round: number | null;
}

export function unseenOf(seen: Seen, chains: readonly Chain[], rounds: readonly number[], you: string): Unseen {
  const fresh = everyComment(chains).filter(
    ({ comment }) => !seen.comments.has(comment.id) && !comment.draft && comment.author !== you,
  );
  const newest = Math.max(0, ...rounds);
  return {
    comments: new Set(fresh.map(({ comment }) => comment.id)),
    general: fresh.filter((f) => f.general).length,
    onFiles: fresh.filter((f) => !f.general).length,
    round: newest > seen.round ? newest : null,
  };
}

/** `seen` with these comments seen too */
export function withSeen(seen: Seen, ids: Iterable<string>): Seen {
  return { ...seen, comments: new Set([...seen.comments, ...ids]) };
}

const KEY = Symbol('review-unseen');

/** What the chains under a review's page ask of it: whether a comment is new, and that it has been seen */
export interface UnseenMarks {
  isNew(id: string): boolean;
  seen(id: string): void;
}

export function setUnseenMarks(marks: UnseenMarks): void {
  setContext(KEY, marks);
}

/** The page's marks; none outside a review's page (a chain elsewhere is never marked new) */
export function unseenMarks(): UnseenMarks | undefined {
  return getContext<UnseenMarks | undefined>(KEY);
}

/** An attachment: `id` is seen once at least half of the element has been in view (`onseen`), once */
export function seenInView(id: string, marks: UnseenMarks | undefined) {
  return (el: HTMLElement) => {
    if (!marks?.isNew(id) || typeof IntersectionObserver === 'undefined') return;
    const watch = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          marks.seen(id);
          watch.disconnect();
        }
      },
      { threshold: 0.5 },
    );
    watch.observe(el);
    return () => watch.disconnect();
  };
}
