// The Documents page's list (docs/documents-design.md §4, ATM-766): the listing as a tree of folders or by last
// change, the search over titles and paths, and the page's address (`?list=recent&q=sidebar`: `list`, as `view` is
// the address's own name for the project's page).
import { addressParams, formatAddress } from './address';
import type { DocumentEntry, Submodule } from './api/documents';

export type DocumentsView = 'tree' | 'recent';

/** The page's view and search, as its address has them */
export interface DocumentsListAddress {
  view: DocumentsView;
  q: string;
}

/** The view and search an address names: the tree and no search by default (an unknown view is the tree) */
export function documentsListOf(address: string): DocumentsListAddress {
  const params = addressParams(address);
  return { view: params.get('list') === 'recent' ? 'recent' : 'tree', q: params.get('q')?.trim() ?? '' };
}

/** `address` with the view and search, the defaults left out */
export function withDocumentsList(address: string, list: DocumentsListAddress): string {
  const params = addressParams(address);
  params.delete('list');
  params.delete('q');
  if (list.view !== 'tree') params.set('list', list.view);
  if (list.q.trim()) params.set('q', list.q.trim());
  return formatAddress(params);
}

/** The documents whose title or path holds `q` (any case), as atm's route keeps them; every one without */
export function matching(documents: readonly DocumentEntry[], q: string): DocumentEntry[] {
  const query = q.trim().toLowerCase();
  if (!query) return [...documents];
  return documents.filter((d) => d.title.toLowerCase().includes(query) || d.path.toLowerCase().includes(query));
}

/** Every document by its last change, newest first (one without a time last), then by path */
export function byRecent(documents: readonly DocumentEntry[]): DocumentEntry[] {
  const at = (d: DocumentEntry) => (d.modified_at ? Date.parse(d.modified_at) : Number.NEGATIVE_INFINITY);
  return [...documents].sort((a, b) => at(b) - at(a) || a.path.localeCompare(b.path));
}

/** A folder of the tree: its name and path, its folders then its files, how many documents are in it all, and
 * whether it is a submodule (and checked out) */
export interface Folder {
  name: string;
  path: string;
  folders: Folder[];
  files: DocumentEntry[];
  count: number;
  submodule: { checked_out: boolean } | null;
}

/** The file's name, the last part of its path */
export const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** The documents as a tree by folder, from the project's root: each folder's folders first, then its files, each by
 * name; a submodule is a folder marked as one, there even when it holds no document (not checked out) */
export function treeOf(documents: readonly DocumentEntry[], submodules: readonly Submodule[] = []): Folder {
  const root: Folder = { name: '', path: '', folders: [], files: [], count: 0, submodule: null };
  const folderAt = (path: string): Folder => {
    let at = root;
    let walked = '';
    for (const name of path.split('/').filter(Boolean)) {
      walked = walked ? `${walked}/${name}` : name;
      let next = at.folders.find((f) => f.name === name);
      if (!next) {
        next = { name, path: walked, folders: [], files: [], count: 0, submodule: null };
        at.folders.push(next);
      }
      at = next;
    }
    return at;
  };
  for (const sub of submodules) folderAt(sub.path).submodule = { checked_out: sub.checked_out };
  for (const doc of documents) {
    const slash = doc.path.lastIndexOf('/');
    folderAt(slash < 0 ? '' : doc.path.slice(0, slash)).files.push(doc);
  }
  const settle = (folder: Folder): number => {
    folder.folders.sort((a, b) => a.name.localeCompare(b.name));
    folder.files.sort((a, b) => fileName(a.path).localeCompare(fileName(b.path)));
    folder.count = folder.files.length + folder.folders.reduce((n, f) => n + settle(f), 0);
    return folder.count;
  };
  settle(root);
  return root;
}

/** The folders a document's path is under (`docs/adr/x.md`: `docs`, `docs/adr`), to open the way to it */
export function foldersOf(path: string): string[] {
  const parts = path.split('/').slice(0, -1);
  return parts.map((_, i) => parts.slice(0, i + 1).join('/'));
}
