import { getContext, setContext } from 'svelte';
import type { DocumentRef } from '../documents';

// What a link to one of the project's documents does in a review's texts (ATM-586): its summary and every comment, in
// whatever part of the page draws them (the Overview, the Comments tab, a chain in a diff). The review's page decides
// and gives it to them through the component tree.

const KEY = Symbol('review-document-links');

/** The review's page: what a link to a document of the project does in the texts under it */
export function setDocumentLinks(open: (ref: DocumentRef) => void): void {
  setContext(KEY, open);
}

/** What a link to a document of the project does here; none outside a review's page (such a link then does nothing) */
export function documentLinks(): ((ref: DocumentRef) => void) | undefined {
  return getContext<((ref: DocumentRef) => void) | undefined>(KEY);
}
