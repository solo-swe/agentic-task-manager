import { getContext, setContext } from 'svelte';
import type { TaskLinks } from '../compmd';
import type { DocumentRef } from '../documents';

// What a link does in a review's texts: its summary and every comment, in whatever part of the page draws them (the
// Overview, the Comments tab, a chain in a diff). The review's page decides and gives it to them through the
// component tree: a link to one of the project's documents (ATM-586), and a task's key (ATM-593).

const KEY = Symbol('review-document-links');
const TASKS = Symbol('review-task-links');

/** The review's page: what a link to a document of the project does in the texts under it */
export function setDocumentLinks(open: (ref: DocumentRef) => void): void {
  setContext(KEY, open);
}

/** What a link to a document of the project does here; none outside a review's page (such a link then does nothing) */
export function documentLinks(): ((ref: DocumentRef) => void) | undefined {
  return getContext<((ref: DocumentRef) => void) | undefined>(KEY);
}

/** The review's page: which task keys in its texts are tasks, and what opening one does */
export function setTaskLinks(tasks: TaskLinks | undefined): void {
  setContext(TASKS, tasks);
}

/** The task keys of the texts here; none outside a review's page (keys then stay text) */
export function taskLinks(): TaskLinks | undefined {
  return getContext<TaskLinks | undefined>(TASKS);
}

const EDITS = Symbol('review-comment-edits');

/** Editing one's own comments on the page (ATM-638): what a change does (why it was refused, or null). Which comments
 * may be changed is each comment's `editable`, atm's (ATM-723). None on a page that can't change one */
export interface CommentEdits {
  edit(comment: string, body: string): Promise<string | null>;
}

export function setCommentEdits(edits: CommentEdits | undefined): void {
  setContext(EDITS, edits);
}

/** What editing one's own comment does here; none where it can't be done */
export function commentEdits(): CommentEdits | undefined {
  return getContext<CommentEdits | undefined>(EDITS);
}
