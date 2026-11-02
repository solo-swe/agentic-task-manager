import { describe, expect, it } from 'vitest';
import { cardFacts, cardNote } from '../../src/lib/cardfacts';
import type { Task } from '../../src/lib/types';

const NOW = Date.parse('2026-01-02T12:00:00Z');
const base: Task = { id: 'T-1', name: 'A task', type: 'feature', status: 'todo' } as Task;

describe('cardFacts', () => {
  it('a to-do task: who created it and when', () => {
    const f = cardFacts({ ...base, created_by: 'Ada', created_at: '2026-01-02T10:00:00Z' }, NOW);
    expect(f.people).toEqual([{ role: 'created', who: 'Ada' }]);
    expect(f.when).toBe('created 2 h ago');
    expect(f.took).toBeNull();
  });

  it('a task in progress: who works on it, since when, and its work so far', () => {
    const f = cardFacts(
      { ...base, status: 'in_progress', started_by: 'claude:abc', started_at: '2026-01-02T11:20:00Z' },
      NOW,
    );
    expect(f.people).toEqual([{ role: 'worked', who: 'claude:abc' }]);
    expect(f.when).toBe('started 40 m ago');
    expect(f.took).toBe('40 m so far');
  });

  it('a done task: who finished it when not its worker, when, and how long it took', () => {
    const f = cardFacts(
      {
        ...base,
        status: 'done',
        agent: 'bob@host:/wt',
        completed_by: 'Ada',
        started_at: '2026-01-02T09:00:00Z',
        completed_at: '2026-01-02T09:45:00Z',
      },
      NOW,
    );
    expect(f.people).toEqual([
      { role: 'worked', who: 'bob@host' },
      { role: 'finished', who: 'Ada' },
    ]);
    expect(f.when).toMatch(/^done .* ago$/);
    expect(f.took).toBe('took 45 m');
  });

  it('a done task finished at once has no work time; a skipped one says when', () => {
    const at = '2026-01-02T09:00:00Z';
    expect(cardFacts({ ...base, status: 'done', started_at: at, completed_at: at }, NOW).took).toBeNull();
    expect(cardFacts({ ...base, status: 'skipped', skipped_at: at }, NOW).when).toMatch(/^skipped .* ago$/);
  });

  it('nothing known: no people, no time', () => {
    expect(cardFacts({ ...base, status: 'in_review' }, NOW)).toEqual({ people: [], when: null, took: null });
  });
});

describe('cardNote', () => {
  it('the latest outcome or handoff, not other kinds', () => {
    const t = {
      ...base,
      notes: [
        { at: '2026-01-01T10:00:00Z', kind: 'handoff', text: 'old' },
        { at: '2026-01-01T11:00:00Z', kind: 'outcome', text: 'new' },
        { at: '2026-01-01T12:00:00Z', kind: 'review', text: 'check' },
      ],
    } as Task;
    expect(cardNote(t)?.text).toBe('new');
    expect(cardNote(base)).toBeUndefined();
  });
});
