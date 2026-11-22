import { getContext, setContext } from 'svelte';
import type { Picked, PlacedChain, PlacedDraft, ReviewComment } from './types';

// A reviewer's drafts (ATM-612): comments and replies only they see, sent together with their decision, as GitHub's
// *start a review*. The review's page says what writing one does (`setDraftActions`); every box that writes a comment or
// a reply under it offers *Add to my review* beside *Comment now*, and the chains show the drafts, marked, with Edit and
// Remove. Without it (a page that can't keep drafts), the boxes send at once, as before.

const KEY = Symbol('review-drafts');

/** What drafting does on this page; each answers why it was refused, or null */
export interface DraftActions {
  /** How many drafts the user has on the review: while there are some, a box's main action is to add one more */
  readonly pending: number;
  /** A draft comment on lines (`picked`) of `path`, the whole file (`picked` null), or the review (`path` null) */
  comment(path: string | null, picked: Picked | null, body: string): Promise<string | null>;
  /** A draft reply to comment `to` */
  reply(to: string, body: string): Promise<string | null>;
  /** A draft's text changed */
  edit(id: string, body: string): Promise<string | null>;
  /** A draft thrown away */
  remove(id: string): Promise<string | null>;
}

export function setDraftActions(actions: DraftActions | undefined): void {
  setContext(KEY, actions);
}

/** What drafting does here; none where drafts can't be kept */
export function draftActions(): DraftActions | undefined {
  return getContext<DraftActions | undefined>(KEY);
}

/** A draft as a comment the chains show: the user's, never resolved, marked */
function asComment(d: PlacedDraft, you: string): ReviewComment {
  return { id: d.id, author: you, at: d.at, body: d.body, resolved: null, draft: true, replies: [] };
}

/**
 * The page's chains with the user's drafts among them: a draft comment a chain of its own, placed where the server
 * placed it (on the round it was written on, as any comment is), after the chains; a draft reply under the comment it
 * answers, after its replies. A reply to a comment the page doesn't have (gone) is left out: it can't be shown where
 * it goes. `round`: where a draft on what a live review is now goes, the round shown.
 */
export function withDrafts(
  chains: PlacedChain[],
  drafts: PlacedDraft[] | undefined,
  you: string,
  round: number,
): PlacedChain[] {
  if (!drafts?.length) return chains;
  const replies = new Map<string, PlacedDraft[]>();
  for (const d of drafts) if (d.kind === 'reply') replies.set(d.to, [...(replies.get(d.to) ?? []), d]);
  const answered = (c: ReviewComment): ReviewComment => {
    const mine = replies.get(c.id);
    const nested = c.replies.map(answered);
    return mine ? { ...c, replies: [...nested, ...mine.map((d) => asComment(d, you))] } : { ...c, replies: nested };
  };
  const withReplies = replies.size ? chains.map((ch) => ({ ...ch, comments: ch.comments.map(answered) })) : chains;
  const own: PlacedChain[] = drafts.flatMap((d) =>
    d.kind === 'comment'
      ? [
          {
            id: `draft-${d.id}`,
            anchor: { round: d.round ?? round, path: d.path, side: d.side, lines: d.lines, quote: d.quote },
            comments: [asComment(d, you)],
            placed: d.placed ?? null,
            drifted: d.drifted ?? null,
          },
        ]
      : [],
  );
  return [...withReplies, ...own];
}
