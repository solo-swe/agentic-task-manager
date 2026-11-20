import { describe, expect, it } from 'vitest';
import { placeName } from '../../src/lib/places';

// A place's name from its address (lib/places.ts, ATM-686): what a review's back button says it goes back to

describe('placeName', () => {
  it.each([
    ['/projects/abc', 'Kanban'],
    ['/projects/abc/kanban', 'Kanban'],
    ['/projects/abc/list', 'List'],
    ['/projects/abc/graph', 'Graph'],
    ['/projects/abc/progress', 'Progress'],
    ['/projects/abc/agents', 'Agents'],
    ['/projects/abc/project', 'Project'],
    ['/projects/abc/reviews', 'Reviews'],
    ['/projects', 'Projects'],
    ['/board', 'Omniboard'],
    ['/reviews', 'All reviews'],
    ['/progress', 'Progress'],
    ['/agents', 'Agents'],
    ['/w/work/board', 'Omniboard'],
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
