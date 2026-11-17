// Where the document viewer gets its files (docs/code-review-design.md §3.1, ATM-571): the viewer is a component of
// its own, which knows no task and no dashboard state. Whoever shows it passes a source: the task manager its
// project's working copy (lib/project-documents.ts), a review a round's files at their revision.
import type { Validators } from '../api/http';
import type { DocumentRef, DocumentResult, ProjectDocument } from '../documents';

export interface DocumentSource {
  /** What it reads (a project, a review's round): another one, and the viewer loads the file afresh */
  readonly id: string;
  /** A file, with `since` a conditional request (unchanged: `{ unchanged: true }`) */
  load(path: string, since?: Validators): Promise<DocumentResult>;
  /** A relative image in a document, resolved from the project's root: its address, or null (its alt text shows) */
  imageUrl(path: string): string | null;
  /** Where the file is on the repository's host (Open on GitHub), or null */
  repositoryLink(doc: ProjectDocument, ref: DocumentRef): { href: string; host: string } | null;
}

/** When the viewer asks for the open file again */
export interface DocumentRefresh {
  /** Every so many seconds, a conditional request; null: no polling */
  every: number | null;
  /** Each time this changes (the app's reloads), once; 0: never */
  reloadedAt: number;
}
