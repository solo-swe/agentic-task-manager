import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import DocumentViewer from '../../src/lib/documents/DocumentViewer.svelte';
import type { DocumentRef, DocumentResult, ProjectDocument } from '../../src/lib/documents';
import type { DocumentSource } from '../../src/lib/documents/source';

// The document viewer as a component of its own (docs/code-review-design.md §3.1, ATM-571): it reads its files from
// the source it is given (here a review's round, made up), knows no dashboard state, and says what Back, Close and a
// document link do through its handlers
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

const file = (path: string, text: string): ProjectDocument => ({
  path,
  kind: 'markdown',
  language: null,
  text,
  document: text,
  line_offset: 0,
  lines: text.split('\n').length,
  bytes: text.length,
  modified_at: null,
  tracked: true,
  commit: 'abc1234',
  submodule: null,
});

/** A round's files at its revision: whatever was asked for, and how often */
function roundSource(files: Record<string, string>): DocumentSource & { asked: string[] } {
  const asked: string[] = [];
  return {
    id: 'CR-7/round-2',
    asked,
    async load(path): Promise<DocumentResult> {
      asked.push(path);
      const text = files[path];
      return text === undefined
        ? { unchanged: false, error: 'missing', message: `${path}: not in round 2` }
        : { unchanged: false, document: file(path, text), validators: {} };
    },
    imageUrl: () => null,
    repositoryLink: (_doc, ref) => ({ href: `https://example.invalid/blob/abc1234/${ref.path}`, host: 'example' }),
  };
}

const handlers = () => ({ onback: vi.fn(), onclose: vi.fn(), onopen: vi.fn<(ref: DocumentRef) => void>() });

afterEach(() => {
  vi.restoreAllMocks();
  loadCompmd.mockReset();
});

describe('DocumentViewer, with a source of its own', () => {
  it('shows the file its source gives, and links where the source says', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    loadCompmd.mockRejectedValue(new Error('no CompMD in this test')); // the text then shows as written
    const source = roundSource({ 'docs/plan.md': '# The plan\n\nShip it.' });
    const on = handlers();
    render(DocumentViewer, {
      source,
      ref: { path: 'docs/plan.md' },
      refresh: { every: null, reloadedAt: 0 },
      ...on,
    });
    expect(await screen.findByText(/Ship it\./)).toBeInTheDocument();
    expect(source.asked).toEqual(['docs/plan.md']);
    expect(document.querySelector('#doc-path')).toHaveTextContent('docs/plan.md');
    expect(document.querySelector('#doc-repo')).toHaveAttribute(
      'href',
      'https://example.invalid/blob/abc1234/docs/plan.md',
    );
    await fireEvent.click(document.querySelector('#doc-back')!);
    await fireEvent.click(document.querySelector('#doc-close')!);
    expect(on.onback).toHaveBeenCalledOnce();
    expect(on.onclose).toHaveBeenCalledOnce();
  });

  it('a file the source hasn’t says so; with no source, nothing shows', async () => {
    const on = handlers();
    const { unmount } = render(DocumentViewer, {
      source: roundSource({}),
      ref: { path: 'gone.md' },
      refresh: { every: null, reloadedAt: 0 },
      ...on,
    });
    expect(await screen.findByText('gone.md: not in round 2')).toBeInTheDocument();
    expect(document.querySelector('#doc-fallback')).toHaveAttribute('data-error', 'missing');
    unmount();
    render(DocumentViewer, { source: null, ref: { path: 'a.md' }, refresh: { every: null, reloadedAt: 0 }, ...on });
    expect(document.querySelector('#doc-viewer')).toBeNull();
  });
});
