import { describe, expect, it } from 'vitest';
import { readReleases, releaseNotes, releaseSummaries, splitReleases } from '../../src/lib/releases';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import fixture from '../fixtures/sprints.json';

const file = fixture as TasksFile;
const tasks = file.tasks as Task[];
const vocab = resolveVocabulary(file.meta);
const model = sprintModel(file.meta, tasks, vocab);

describe('releases', () => {
  it('reads meta.releases leniently: no ID left out, an unknown state is planned, a non-number PR dropped', () => {
    const meta = {
      releases: [{ id: '' }, { id: '1.0', state: 'shipped', pr: { number: 'x' } }, { id: '1.0' }, 'x'],
    };
    expect(readReleases(meta)).toEqual([{ id: '1.0', state: 'planned' }]);
    expect(readReleases({})).toEqual([]);
  });

  it('sums up each release not archived: its sprints, its tasks by stage (own release, or the sprint’s)', () => {
    const summaries = releaseSummaries(file.meta, tasks, model, vocab);
    expect(summaries.map((s) => [s.release.id, s.sprints, s.total, s.done, s.inPr, s.merged])).toEqual([
      ['1.0', ['Foundations', 'Ada Lovelace'], 3, 1, 0, 0],
      ['1.1', [], 1, 0, 0, 0],
    ]);
    expect(releaseSummaries(file.meta, tasks, model, vocab, true).map((s) => s.release.id)).toContain('0.9');
  });

  it('splits the open releases, in order, from the released ones, newest first (T-323)', () => {
    const meta = {
      releases: [
        { id: '0.1', state: 'released', released_at: '2026-01-01T00:00:00Z' },
        { id: '0.3', state: 'released', released_at: '2026-03-01T00:00:00Z' },
        { id: '0.4', state: 'planned' },
        { id: '0.2', state: 'released' },
        { id: '0.5', state: 'planned' },
      ],
    };
    const { open, released } = splitReleases(releaseSummaries(meta, [], model, vocab));
    expect(open.map((s) => s.release.id)).toEqual(['0.4', '0.5']);
    // Without a date: after the dated ones
    expect(released.map((s) => s.release.id)).toEqual(['0.3', '0.1', '0.2']);
  });

  it('drafts the notes as atm releases notes does: done by type with outcomes, then what is not done', () => {
    const own = tasks
      .filter((t) => ['T-001', 'T-002', 'T-003'].includes(t.id))
      .map((t) => (t.id === 'T-001' ? { ...t, notes: [{ at: '', kind: 'outcome', text: 'Set up' }] } : t));
    expect(releaseNotes(readReleases(file.meta)[0], own as Task[], vocab)).toBe(
      [
        '# 1.0 “First”',
        '',
        '## Infra',
        '',
        '- **T-001** Repository: Set up',
        '',
        '## Not done yet',
        '',
        '- T-002 Sign-in form (in_progress)',
        '- T-003 Sessions (todo)',
      ].join('\n'),
    );
    expect(releaseNotes({ id: '2.0', state: 'planned' }, [], vocab)).toBe('# 2.0\n\nNothing done in it yet.');
  });
});
