import { describe, expect, it } from 'vitest';
import type { DocumentEntry } from '../../src/lib/api/documents';
import {
  byRecent,
  documentsListOf,
  fileName,
  foldersOf,
  matching,
  treeOf,
  withDocumentsList,
  type Folder,
} from '../../src/lib/document-list';

// The Documents page's list (lib/document-list.ts, ATM-766): its address, the search, Recent and the tree

const doc = (
  path: string,
  title = fileName(path),
  modified_at: string | null = null,
  tracked = true,
): DocumentEntry => ({
  path,
  title,
  bytes: 10,
  modified_at,
  tracked,
});

describe('the page’s address', () => {
  it('the view and the search, with their defaults (the tree, no search; an unknown view is the tree)', () => {
    expect(documentsListOf('/projects/p1/documents')).toEqual({ view: 'tree', q: '' });
    expect(documentsListOf('/projects/p1/documents?list=recent&q=+side+')).toEqual({ view: 'recent', q: 'side' });
    expect(documentsListOf('/projects/p1/documents?list=grid')).toEqual({ view: 'tree', q: '' });
  });

  it('written without the defaults, keeping the other parameters (a document open over the page) and the page', () => {
    expect(withDocumentsList('/projects/p1/documents?doc=a.md', { view: 'recent', q: ' adr ' })).toBe(
      '/projects/p1/documents?doc=a.md&list=recent&q=adr',
    );
    expect(withDocumentsList('/projects/p1/documents?list=recent&q=x', { view: 'tree', q: '' })).toBe(
      '/projects/p1/documents',
    );
  });
});

describe('the search', () => {
  const docs = [doc('docs/adr/ADR-003.md', 'Layered crates'), doc('README.md', 'Agentic Task Manager')];

  it('keeps those whose title or path holds it, in any case; none given keeps every one', () => {
    expect(matching(docs, 'LAYERED').map((d) => d.path)).toEqual(['docs/adr/ADR-003.md']);
    expect(matching(docs, 'adr/').map((d) => d.path)).toEqual(['docs/adr/ADR-003.md']);
    expect(matching(docs, 'readme').map((d) => d.path)).toEqual(['README.md']);
    expect(matching(docs, '  ')).toEqual(docs);
    expect(matching(docs, 'nothing')).toEqual([]);
  });
});

describe('Recent', () => {
  it('by last change, newest first; one without a time last; a tie by path', () => {
    const docs = [
      doc('b.md', 'B', '2026-11-20T10:00:00Z'),
      doc('none.md', 'None', null),
      doc('a.md', 'A', '2026-11-24T10:00:00Z'),
      doc('c.md', 'C', '2026-11-20T10:00:00Z'),
    ];
    expect(byRecent(docs).map((d) => d.path)).toEqual(['a.md', 'b.md', 'c.md', 'none.md']);
    // The listing as it was
    expect(docs[0].path).toBe('b.md');
  });
});

describe('the tree', () => {
  const names = (folder: Folder): unknown => ({
    folders: folder.folders.map((f) => [f.name, f.count, f.submodule, names(f)]),
    files: folder.files.map((d) => d.path),
  });

  it('none: an empty root', () => {
    expect(treeOf([])).toEqual({ name: '', path: '', folders: [], files: [], count: 0, submodule: null });
  });

  it('folders first, then files, each by name; each folder counts every document in it', () => {
    const tree = treeOf([
      doc('README.md'),
      doc('docs/z.md'),
      doc('docs/adr/ADR-002.md'),
      doc('docs/adr/ADR-001.md'),
      doc('CHANGELOG.md'),
      doc('packaging/README.md'),
    ]);
    expect(tree.count).toBe(6);
    expect(names(tree)).toEqual({
      folders: [
        [
          'docs',
          3,
          null,
          {
            folders: [['adr', 2, null, { folders: [], files: ['docs/adr/ADR-001.md', 'docs/adr/ADR-002.md'] }]],
            files: ['docs/z.md'],
          },
        ],
        ['packaging', 1, null, { folders: [], files: ['packaging/README.md'] }],
      ],
      files: ['CHANGELOG.md', 'README.md'],
    });
    expect(tree.folders[0].folders[0].path).toBe('docs/adr');
  });

  it('a submodule is a folder marked as one, nested ones too; one not checked out is there, empty', () => {
    const tree = treeOf(
      [doc('vendor/lib/README.md'), doc('vendor/lib/deep/x.md')],
      [
        { path: 'vendor/lib', checked_out: true },
        { path: 'vendor/lib/deep', checked_out: true },
        { path: 'vendor/gone', checked_out: false },
      ],
    );
    const vendor = tree.folders[0];
    expect(vendor.submodule).toBeNull();
    expect(vendor.folders.map((f) => [f.name, f.submodule, f.count])).toEqual([
      ['gone', { checked_out: false }, 0],
      ['lib', { checked_out: true }, 2],
    ]);
    expect(vendor.folders[1].folders[0].submodule).toEqual({ checked_out: true });
  });
});

describe('the folders a document is under', () => {
  it('each folder on its way from the root; none at the root', () => {
    expect(foldersOf('docs/adr/x.md')).toEqual(['docs', 'docs/adr']);
    expect(foldersOf('README.md')).toEqual([]);
    expect(fileName('docs/adr/x.md')).toBe('x.md');
    expect(fileName('README.md')).toBe('README.md');
  });
});
