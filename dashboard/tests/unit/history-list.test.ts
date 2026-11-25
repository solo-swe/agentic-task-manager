import { describe, expect, it } from 'vitest';
import { entryName, historyItems } from '../../src/lib/history-list';

const entries = ['/projects/web/kanban', '/projects/web/list?task=WEB-3', '/board', '/projects/api/reviews/CR-7'].map(
  (address) => ({ address, state: {} }),
);
const names: Record<string, string> = { web: 'Website' };
const name = (id: string) => names[id];

describe('historyItems', () => {
  it('lists the entries before the one shown, nearest first, with the steps back to each', () => {
    expect(historyItems({ entries, at: 2 }, -1, name)).toEqual([
      { address: '/projects/web/list?task=WEB-3', name: 'WEB-3 — Website', steps: 1 },
      { address: '/projects/web/kanban', name: 'Board — Website', steps: 2 },
    ]);
  });

  it('lists the entries after it, nearest first, with negative steps (forward)', () => {
    expect(historyItems({ entries, at: 1 }, 1, name)).toEqual([
      { address: '/board', name: 'Board', steps: -1 },
      { address: '/projects/api/reviews/CR-7', name: 'CR-7 — api', steps: -2 },
    ]);
  });

  it('lists nothing past either end, one entry, or no tab', () => {
    expect(historyItems({ entries, at: 0 }, -1)).toEqual([]);
    expect(historyItems({ entries, at: 3 }, 1)).toEqual([]);
    expect(historyItems({ entries: entries.slice(0, 1), at: 0 }, 1)).toEqual([]);
    expect(historyItems(undefined, -1)).toEqual([]);
  });
});

describe('entryName', () => {
  it.each([
    ['/projects/web/graph', 'Dependencies — Website'],
    ['/projects/web?doc=docs/x.md', 'Document — Website'],
    ['/projects/other/list', 'List — other'],
    ['/projects', 'Projects'],
    ['/reviews', 'All reviews'],
  ])('names %s "%s"', (address, expected) => {
    expect(entryName(address, name)).toBe(expected);
  });
});
