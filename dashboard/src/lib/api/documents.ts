// A project's documents (docs/documents-design.md §5, ATM-765, ATM-766): the listing the Documents page shows, read as
// the other routes are (conditionally, by `ETag`); anything unexpected in the answer left out.
import { browserFetch, documentsUrl, fetchFile, type Fetch, type Validators } from './http';

/** A document, as atm lists it: its path from the project's root, its title, size, last change, and whether git
 * tracks it (an untracked one is *new*) */
export interface DocumentEntry {
  path: string;
  title: string;
  bytes: number;
  modified_at: string | null;
  tracked: boolean;
}

/** A submodule of the project: its documents are among the listing's, by their path from the project's root */
export interface Submodule {
  path: string;
  checked_out: boolean;
}

/** The listing: the documents (at most 500), the submodules, how many more there were past the cap, and whether the
 * project is in git (outside it, atm lists none) */
export interface DocumentsListing {
  documents: DocumentEntry[];
  submodules: Submodule[];
  more: number;
  git: boolean;
}

export type DocumentsResult =
  { unchanged: true } | { unchanged: false; listing: DocumentsListing; validators: Validators };

const isEntry = (d: unknown): d is Record<string, unknown> =>
  !!d && typeof d === 'object' && typeof (d as { path?: unknown }).path === 'string';

/** The project's documents, those `q` names (titles and paths), every one without */
export async function loadDocuments(
  project: string,
  q = '',
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<DocumentsResult> {
  const file = await fetchFile(documentsUrl(project, q), fetchFn, since);
  if (file.unchanged) return file;
  const data = (file.data ?? {}) as Partial<Record<keyof DocumentsListing, unknown>>;
  const git = data.git !== false;
  const documents = (Array.isArray(data.documents) ? data.documents : []).filter(isEntry).map((d): DocumentEntry => ({
    path: d.path as string,
    title: typeof d.title === 'string' && d.title ? d.title : (d.path as string).split('/').pop()!,
    bytes: typeof d.bytes === 'number' ? d.bytes : 0,
    modified_at: typeof d.modified_at === 'string' ? d.modified_at : null,
    tracked: d.tracked !== false,
  }));
  const submodules = (Array.isArray(data.submodules) ? data.submodules : [])
    .filter(isEntry)
    .map((s): Submodule => ({ path: s.path as string, checked_out: s.checked_out !== false }));
  const more = typeof data.more === 'number' && data.more > 0 ? data.more : 0;
  return { unchanged: false, listing: { documents, submodules, more, git }, validators: file.validators };
}
