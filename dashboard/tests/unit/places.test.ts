import { describe, expect, it } from 'vitest';
import { placeName } from '../../src/lib/places';

// A place's name from its address (lib/places.ts, ATM-686): what a review's back button says it goes back to

describe('placeName', () => {
  it.each([
    // A project without a tab is on its Overview (ATM-751, ATM-752: the Project tab is the Overview item); the Kanban
    // is the Board, the graph Dependencies, and All's board the Board too
    ['/projects/abc', 'Overview'],
    ['/projects/abc/kanban', 'Board'],
    ['/projects/abc/list', 'List'],
    ['/projects/abc/graph', 'Dependencies'],
    ['/projects/abc/progress', 'Progress'],
    ['/projects/abc/agents', 'Agents'],
    ['/projects/abc/project', 'Overview'],
    ['/projects/abc/reviews', 'Reviews'],
    ['/projects/abc/inbox', 'Inbox'],
    ['/projects/abc/documents', 'Documents'],
    ['/projects', 'Projects'],
    ['/board', 'Board'],
    ['/list', 'List'],
    ['/reviews', 'All reviews'],
    ['/progress', 'Progress'],
    ['/agents', 'Agents'],
    ['/w/work/board', 'Board'],
    ['/', 'Projects'],
  ])('a page: %s is %s', (address, name) => {
    expect(placeName(address)).toBe(name);
  });

  it('what is open over a page names it: a task’s details, a document', () => {
    expect(placeName('/projects/abc/kanban?task=ATM-12')).toBe('ATM-12');
    expect(placeName('/board?task=LFB-3')).toBe('LFB-3');
    expect(placeName('/projects/abc/list?doc=docs/x.md')).toBe('Document');
  });

  it('another review is named by its reference', () => {
    expect(placeName('/projects/abc/reviews/CR-7?round=2')).toBe('CR-7');
  });
});
