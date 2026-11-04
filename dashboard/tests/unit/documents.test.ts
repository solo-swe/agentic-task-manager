// lib/documents.ts: the document viewer's references, its URL hash, loading a file, its title and its window
// (docs/compmd-viewer-design.md §2, §3, §6)
import { describe, expect, it, vi } from 'vitest';
import {
  clampGeometry,
  defaultGeometry,
  documentTitle,
  isGeometry,
  loadDocument,
  parseDocHash,
  parseLines,
  findTextFragment,
  parseTextFragment,
  pathToCopy,
  referenceTo,
  repositoryFile,
  resolvePath,
  VIEWER_MIN,
  withDocHash,
  type ProjectDocument,
} from '../../src/lib/documents';
import type { Fetch } from '../../src/lib/api/http';
import { fileRef } from '../../src/lib/repo';
import { dashboard } from '../../src/lib/state.svelte';

describe('resolvePath: the project path a reference names', () => {
  it('from the project root for task text', () => {
    expect(resolvePath('docs/x.md')).toBe('docs/x.md');
    expect(resolvePath('./docs/x.md')).toBe('docs/x.md');
    expect(resolvePath('/docs/x.md')).toBe('docs/x.md');
    expect(resolvePath('docs/x.md#3-routes')).toBe('docs/x.md');
    expect(resolvePath('docs/a%20b.md')).toBe('docs/a b.md');
  });

  it('from the open document’s folder inside a document', () => {
    expect(resolvePath('../v4/data-model.md', 'docs/x.md')).toBe('v4/data-model.md');
    expect(resolvePath('other.md', 'docs/sub/x.md')).toBe('docs/sub/other.md');
    expect(resolvePath('/README.md', 'docs/x.md')).toBe('README.md');
  });

  it('never leaves the project, and never names another site', () => {
    for (const href of [
      '../x.md',
      'docs/../../x.md',
      'https://example.com/x.md',
      '//example.com/x',
      'mailto:a@b',
      '#top',
      '',
      'a\\b',
    ]) {
      expect(resolvePath(href), href).toBeNull();
    }
    expect(resolvePath('../../x.md', 'docs/x.md')).toBeNull();
  });
});

describe('referenceTo: the document a relative link names, with its lines or heading', () => {
  it('from the project root, or from the open document’s folder', () => {
    expect(referenceTo('docs/x.md')).toEqual({ path: 'docs/x.md' });
    expect(referenceTo('src/app.ts#L40')).toEqual({ path: 'src/app.ts', lines: [40, 40] });
    expect(referenceTo('src/app.ts#L40-L45')).toEqual({ path: 'src/app.ts', lines: [40, 45] });
    expect(referenceTo('docs/x.md#3-routes')).toEqual({ path: 'docs/x.md', heading: '3-routes' });
    expect(referenceTo('../v4/data-model.md#tasks', 'docs/x.md')).toEqual({
      path: 'v4/data-model.md',
      heading: 'tasks',
    });
  });
  it('none for a link that isn’t a project file', () => {
    for (const href of ['https://example.com/x.md', '#routes', '../x.md', 'mailto:a@b']) {
      expect(referenceTo(href), href).toBeNull();
    }
  });
  it('a heading survives the URL hash, and Copy path keeps it', () => {
    const hash = withDocHash('#view=list', { path: 'docs/x.md', heading: '3-routes' });
    expect(parseDocHash(hash)).toEqual({ path: 'docs/x.md', heading: '3-routes' });
    expect(pathToCopy({ path: 'docs/x.md', heading: '3-routes' })).toBe('docs/x.md#3-routes');
  });
});

describe('text fragments: #:~:text=…', () => {
  it('parses the directives, the parts decoded, -, , and & in the text encoded', () => {
    expect(parseTextFragment(':~:text=SR%2D2.1')).toEqual([{ start: 'SR-2.1' }]);
    expect(parseTextFragment(':~:text=Routes-,One%20route')).toEqual([{ prefix: 'Routes', start: 'One route' }]);
    expect(parseTextFragment(':~:text=one,two,-three')).toEqual([{ start: 'one', end: 'two', suffix: 'three' }]);
    expect(parseTextFragment('routes:~:text=a%2C%20b%26c&text=d')).toEqual([{ start: 'a, b&c' }, { start: 'd' }]);
    expect(parseTextFragment(':~:text=')).toEqual([]);
    expect(parseTextFragment('routes')).toEqual([]);
    expect(parseTextFragment(':~:text=%E0%A4%A')).toEqual([]); // malformed encoding: left out
  });

  it('a reference keeps it, encoded, through the URL hash and Copy path', () => {
    const ref = referenceTo('docs/x.md#:~:text=SR%2D2.1');
    expect(ref).toEqual({ path: 'docs/x.md', text: ':~:text=SR%2D2.1' });
    // After a heading, the text fragment wins
    expect(referenceTo('docs/x.md#routes:~:text=One')).toEqual({ path: 'docs/x.md', text: ':~:text=One' });
    expect(parseDocHash(withDocHash('#view=list', ref))).toEqual(ref);
    expect(pathToCopy(ref!)).toBe('docs/x.md#:~:text=SR%2D2.1');
  });

  it('finds the first match: case and whitespace aside, across inline elements, with prefix, suffix and range', () => {
    const root = document.createElement('div');
    root.innerHTML =
      '<h2>Routes</h2><p>One   <strong>route</strong> per project.</p><table><tr><td>SR-2.1</td><td>Sprint states</td></tr></table>' +
      '<p>One route again, then the end.</p>';
    const text = (d: Parameters<typeof findTextFragment>[1]) => findTextFragment(root, d)?.toString();
    expect(text({ start: 'one route' })).toBe('One   route');
    expect(text({ start: 'SR-2.1' })).toBe('SR-2.1');
    // A suffix picks the second match
    expect(text({ start: 'one route', suffix: 'again' })).toBe('One route');
    expect(findTextFragment(root, { start: 'one route', suffix: 'again' })?.startContainer.parentElement?.tagName).toBe(
      'P',
    );
    // A prefix across blocks: the heading, then the paragraph
    expect(text({ prefix: 'Routes', start: 'One' })).toBe('One');
    // A range from one text to another
    expect(text({ start: 'One route again', end: 'the end' })).toBe('One route again, then the end');
    // Text in two cells is apart, not run together
    expect(text({ start: 'SR-2.1 Sprint' })).toBe('SR-2.1Sprint');
    expect(text({ start: 'nowhere' })).toBeUndefined();
  });
});

describe('the URL hash: #doc=<path>:L<n>', () => {
  it('reads a path and its lines', () => {
    expect(parseDocHash('#view=list&doc=docs/x.md')).toEqual({ path: 'docs/x.md' });
    expect(parseDocHash('#doc=src/app.ts:L40')).toEqual({ path: 'src/app.ts', lines: [40, 40] });
    expect(parseDocHash('#doc=src/app.ts:L45-L40')).toEqual({ path: 'src/app.ts', lines: [40, 45] });
    expect(parseDocHash('#doc=../etc/passwd')).toBeNull();
    expect(parseDocHash('#view=list')).toBeNull();
    // A name that starts like a scheme (`notes:`) reads as one, as a browser reads it in a link
    expect(parseDocHash('#doc=notes:draft.md')).toBeNull();
    expect(parseDocHash('#doc=docs/notes:draft.md')).toEqual({ path: 'docs/notes:draft.md' });
  });

  it('writes it readably, keeping the other parameters, and removes it', () => {
    const hash = withDocHash('#view=list&project=p1', { path: 'docs/x.md', lines: [3, 7] });
    expect(hash).toBe('#view=list&project=p1&doc=docs/x.md:L3-L7');
    expect(parseDocHash(hash)).toEqual({ path: 'docs/x.md', lines: [3, 7] });
    expect(withDocHash(hash, null)).toBe('#view=list&project=p1');
  });

  it('lines', () => {
    expect(parseLines('L7')).toEqual([7, 7]);
    expect(parseLines('L7-9')).toEqual([7, 9]);
    expect(parseLines('L0')).toBeUndefined();
    expect(parseLines('7')).toBeUndefined();
  });

  it('Copy path gives the path from the root, with the lines', () => {
    expect(pathToCopy({ path: 'docs/x.md' })).toBe('docs/x.md');
    expect(pathToCopy({ path: 'src/app.ts', lines: [40, 40] })).toBe('src/app.ts#L40');
    expect(pathToCopy({ path: 'src/app.ts', lines: [40, 45] })).toBe('src/app.ts#L40-L45');
  });
});

describe('the title', () => {
  const md = (text: string) => ({ path: 'docs/the-design.md', kind: 'markdown' as const, text });
  it('a Markdown document’s first heading, after its frontmatter and outside code', () => {
    expect(documentTitle(md('# The **design**\n\n## Later'))).toBe('The design');
    expect(documentTitle(md('---\ntitle: x\n---\n```\n# not this\n```\n## Routes'))).toBe('Routes');
    expect(documentTitle(md('No heading.'))).toBe('the-design.md');
  });
  it('else the file’s name', () => {
    expect(documentTitle({ path: 'src/app.ts', kind: 'code', text: '# comment' })).toBe('app.ts');
  });
});

describe('loading a file', () => {
  const doc: ProjectDocument = {
    path: 'docs/x.md',
    kind: 'markdown',
    language: null,
    text: '# X\n',
    document: '# X\n',
    line_offset: 0,
    lines: 1,
    bytes: 4,
    modified_at: '2026-11-03T10:00:00Z',
    tracked: true,
    commit: 'a'.repeat(40),
  };
  const answer = (status: number, body: unknown, headers: Record<string, string> = {}) =>
    vi.fn<Fetch>(async () => new Response(status === 304 ? null : JSON.stringify(body), { status, headers }));

  it('asks the documents route, conditionally once it has an ETag', async () => {
    const fetchFn = answer(200, doc, { ETag: '"e1"' });
    const result = await loadDocument('p 1', 'docs/x.md', fetchFn);
    expect(fetchFn.mock.calls[0][0]).toBe('api/v1/projects/p%201/documents?path=docs%2Fx.md');
    expect(result).toEqual({ unchanged: false, document: doc, validators: { etag: '"e1"' } });
    const again = answer(304, null);
    expect(await loadDocument('p 1', 'docs/x.md', again, { etag: '"e1"' })).toEqual({ unchanged: true });
    expect((again.mock.calls[0] as unknown[])[1]).toMatchObject({ headers: { 'If-None-Match': '"e1"' } });
  });

  it('says why a file can’t be shown: missing, refused, or a failure', async () => {
    expect(await loadDocument('p', 'x.md', answer(404, { error: 'x.md: not in the project' }))).toEqual({
      unchanged: false,
      error: 'missing',
      message: 'x.md: not in the project',
    });
    for (const status of [400, 403, 413]) {
      expect(await loadDocument('p', 'x.md', answer(status, { error: 'no' }))).toMatchObject({ error: 'refused' });
    }
    expect(await loadDocument('p', 'x.md', answer(500, 'oops'))).toMatchObject({
      error: 'failed',
      message: 'HTTP 500',
    });
    const offline = vi.fn(async () => {
      throw new Error('offline');
    });
    expect(await loadDocument('p', 'x.md', offline)).toMatchObject({ error: 'failed', message: 'offline' });
  });
});

describe('the window', () => {
  it('opens as wide as the details panel, offset, inside the window', () => {
    const g = defaultGeometry(1280, 800);
    expect(g.w).toBe(680);
    expect(g.x + g.w).toBeLessThanOrEqual(1280);
    expect(g.y + g.h).toBeLessThanOrEqual(800);
    expect(g.x).toBeGreaterThan((1280 - 680) / 2);
  });

  it('stays inside the window and above its minimum size', () => {
    expect(clampGeometry({ x: -50, y: 900, w: 100, h: 5000 }, 1280, 800)).toEqual({
      x: 0,
      y: 0,
      w: VIEWER_MIN.w,
      h: 800,
    });
    expect(clampGeometry({ x: 1200, y: 10, w: 600, h: 400 }, 1280, 800)).toEqual({ x: 680, y: 10, w: 600, h: 400 });
  });

  it('a stored geometry is used only when it is one', () => {
    expect(isGeometry({ x: 1, y: 2, w: 3, h: 4 })).toBe(true);
    expect(isGeometry({ x: 1, y: 2, w: 3 })).toBe(false);
    expect(isGeometry({ x: 'a', y: 2, w: 3, h: 4 })).toBe(false);
    expect(isGeometry(null)).toBe(false);
  });
});

describe('opening and Back (the dashboard state)', () => {
  it('a document opened in the dashboard is a history entry of the viewer’s: Back goes back to the one before', () => {
    history.replaceState(null, '', '#view=list');
    dashboard.openDocument({ path: 'docs/x.md', lines: [3, 3] });
    expect(dashboard.docRef).toEqual({ path: 'docs/x.md', lines: [3, 3] });
    expect(location.hash).toBe('#view=list&doc=docs/x.md:L3');
    expect(history.state).toEqual({ atmDocument: true, atmDocumentDepth: 1 });
    const back = vi.spyOn(history, 'back').mockImplementation(() => {});
    dashboard.documentBack();
    expect(back).toHaveBeenCalledTimes(1);
    back.mockRestore();
  });

  it('Close goes back past every document opened in the viewer, so Back doesn’t bring one back', () => {
    history.replaceState(null, '', '#view=list');
    dashboard.openDocument({ path: 'docs/a.md' });
    dashboard.openDocument({ path: 'docs/b.md' });
    expect(history.state).toEqual({ atmDocument: true, atmDocumentDepth: 2 });
    const go = vi.spyOn(history, 'go').mockImplementation(() => {});
    dashboard.closeDocument();
    expect(go).toHaveBeenCalledWith(-2);
    go.mockRestore();
  });

  it('one opened from a link or a reload closes, without a new history entry', () => {
    history.replaceState(null, '', '#view=list&doc=docs/x.md');
    dashboard.onHashChange();
    expect(dashboard.docRef).toEqual({ path: 'docs/x.md' });
    const length = history.length;
    dashboard.documentBack();
    expect(dashboard.docRef).toBeNull();
    expect(location.hash).toBe('#view=list');
    expect(history.length).toBe(length);
  });
});

describe('Open on GitHub: a file at a commit, in the repository’s web view', () => {
  const sha = 'a'.repeat(40);
  it('each host’s file path and line marks', () => {
    expect(fileRef({ repository: 'https://github.com/o/r' }, sha, 'docs/a b.md', [3, 7])).toEqual({
      href: `https://github.com/o/r/blob/${sha}/docs/a%20b.md#L3-L7`,
      host: 'GitHub',
    });
    expect(fileRef({ repository: 'https://gitlab.com/g/s/r' }, sha, 'x.ts', [3, 3])?.href).toBe(
      `https://gitlab.com/g/s/r/-/blob/${sha}/x.ts#L3`,
    );
    expect(fileRef({ repository: 'https://bitbucket.org/o/r' }, sha, 'x.ts', [3, 7])?.href).toBe(
      `https://bitbucket.org/o/r/src/${sha}/x.ts#lines-3:7`,
    );
    expect(fileRef({ repository: 'https://codeberg.org/o/r' }, sha, 'x.ts')).toEqual({
      href: `https://codeberg.org/o/r/src/commit/${sha}/x.ts`,
      host: 'Gitea',
    });
  });

  it('none without an address, or for a host atm builds no links for', () => {
    expect(fileRef(undefined, sha, 'x.ts')).toBeNull();
    expect(fileRef({ repository: 'https://example.com/o/r' }, sha, 'x.ts')).toBeNull();
    expect(fileRef({ repository: 'javascript:alert(1)' }, sha, 'x.ts')).toBeNull();
  });

  it('a file in a submodule: its own repository, at its commit, the path inside it (T-408)', () => {
    const meta = { repository: 'https://github.com/o/app' };
    const sub = { path: 'vendor/lib', commit: 'b'.repeat(40), repository: 'https://github.com/o/lib' };
    expect(repositoryFile({ tracked: true, commit: sha, submodule: sub }, 'vendor/lib/docs/x.md', meta)).toEqual({
      meta: { repository: 'https://github.com/o/lib' },
      commit: 'b'.repeat(40),
      path: 'docs/x.md',
    });
    // The project's own file, as before
    expect(repositoryFile({ tracked: true, commit: sha, submodule: null }, 'docs/x.md', meta)).toEqual({
      meta,
      commit: sha,
      path: 'docs/x.md',
    });
    // Nowhere to go: untracked, a submodule without a web address, no commit
    expect(repositoryFile({ tracked: false, commit: sha }, 'docs/x.md', meta)).toBeNull();
    expect(
      repositoryFile({ tracked: true, commit: sha, submodule: { ...sub, repository: null } }, 'vendor/lib/x.md', meta),
    ).toBeNull();
    expect(repositoryFile({ tracked: true, commit: null }, 'docs/x.md', meta)).toBeNull();
  });
});
