import { describe, expect, it } from 'vitest';
import type { ProjectDocument } from '../../src/lib/documents';
import { projectDocuments } from '../../src/lib/project-documents';

// The task manager's document source (ATM-571): a project's working copy, as the viewer reads it
const doc = (over: Partial<ProjectDocument> = {}): ProjectDocument => ({
  path: 'docs/a.md',
  kind: 'markdown',
  language: null,
  text: 'a',
  document: 'a',
  line_offset: 0,
  lines: 1,
  bytes: 1,
  modified_at: '2026-11-17T00:00:00Z',
  tracked: true,
  commit: 'abc1234',
  submodule: null,
  ...over,
});

describe('projectDocuments', () => {
  it('reads the project’s files, its images from the files route, and links to its repository at the commit', () => {
    let meta: { repository?: string } = { repository: 'https://github.com/o/r' };
    const source = projectDocuments('p1', () => meta);
    expect(source.id).toBe('p1');
    expect(source.imageUrl('docs/x.png')).toContain('p1');
    expect(source.repositoryLink(doc(), { path: 'docs/a.md', lines: [2, 3] })).toEqual({
      href: 'https://github.com/o/r/blob/abc1234/docs/a.md#L2-L3',
      host: 'GitHub',
    });
    // An untracked file has no link; the repository is read when asked, so a change shows
    expect(source.repositoryLink(doc({ tracked: false }), { path: 'docs/a.md' })).toBeNull();
    meta = {};
    expect(source.repositoryLink(doc(), { path: 'docs/a.md' })).toBeNull();
  });
});
