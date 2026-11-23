import { describe, expect, it } from 'vitest';
import { notesText, splitReleases, type Release } from '../../src/lib/releases';

// The releases as atm serves them with the tasks (ATM-720): their figures and notes are atm's (atm-lib's releases
// tests); the dashboard words and orders them
const release = (fields: Partial<Release> & Pick<Release, 'id'>): Release => ({
  state: 'planned',
  sprints: [],
  total: 0,
  done: 0,
  in_pr: 0,
  merged: 0,
  notes: { groups: [], open: [] },
  ...fields,
});

describe('releases', () => {
  it('splits the open releases, in order, from the released ones, newest first; archived in neither (T-323)', () => {
    const { open, released } = splitReleases([
      release({ id: '0.1', state: 'released', released_at: '2026-01-01T00:00:00Z' }),
      release({ id: '0.3', state: 'released', released_at: '2026-03-01T00:00:00Z' }),
      release({ id: '0.4' }),
      release({ id: '0.2', state: 'released' }),
      release({ id: '0.0', state: 'archived' }),
      release({ id: '0.5' }),
    ]);
    expect(open.map((r) => r.id)).toEqual(['0.4', '0.5']);
    // Without a date: after the dated ones
    expect(released.map((r) => r.id)).toEqual(['0.3', '0.1', '0.2']);
  });

  it("words atm's notes as `atm release notes` prints them: done by type with outcomes, then what is not done", () => {
    const notes = {
      groups: [
        { type: 'infra', label: 'Infra', tasks: [{ id: 'u1', key: 'ATM-1', name: 'Repository', outcome: 'Set up' }] },
      ],
      open: [
        { id: 'T-002', name: 'Sign-in form', status: 'in_progress' },
        { id: 'u3', key: 'ATM-3', name: 'Sessions', status: 'todo' },
      ],
    };
    expect(notesText(release({ id: '1.0', name: 'First', notes }))).toBe(
      [
        '# 1.0 “First”',
        '',
        '## Infra',
        '',
        '- **ATM-1** Repository: Set up',
        '',
        '## Not done yet',
        '',
        '- T-002 Sign-in form (in_progress)',
        '- ATM-3 Sessions (todo)',
      ].join('\n'),
    );
    expect(notesText(release({ id: '2.0' }))).toBe('# 2.0\n\nNothing done in it yet.');
  });
});
