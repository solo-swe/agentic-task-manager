// The task manager's document source (ATM-571): a project's files from its working copy, through the REST API (or
// the app's commands), as the document viewer reads them (lib/documents/source.ts).
import { browserFetch, inApp, v1FileUrl } from './api/http';
import { loadDocument, repositoryFile } from './documents';
import type { DocumentSource } from './documents/source';
import { fileRef } from './repo';

type Meta = Parameters<typeof repositoryFile>[2];

/** Project `id`'s files; `meta` gives its repository's web address (read when a file is shown, so it stays current) */
export function projectDocuments(id: string, meta: () => Meta): DocumentSource {
  return {
    id,
    load: (path, since) => loadDocument(id, path, browserFetch, since),
    // In the atm app there is no files route to load from: the image shows its alt text for now (T-457's follow-up)
    imageUrl: (path) => (inApp ? null : v1FileUrl(id, path)),
    repositoryLink: (doc, ref) => {
      // Only for a file git tracks, at the commit the server named; a file in a submodule in the submodule's own
      // repository (T-408)
      const where = repositoryFile(doc, ref.path, meta());
      return where ? fileRef(where.meta, where.commit, where.path, ref.lines) : null;
    },
  };
}
