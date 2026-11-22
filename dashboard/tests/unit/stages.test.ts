import { describe, expect, it } from 'vitest';
import type { KanbanCard } from '../../src/lib/kanban';
import { doneSections, hasStage, stageOf } from '../../src/lib/stages';
import type { Task } from '../../src/lib/types';
import { withKinds } from './views';

/** A task as atm serves it: with its kind */
const task = (id: string, fields: Partial<Task> = {}): Task =>
  withKinds([{ id, name: id, phase: 0, type: 'feature', description: '', status: 'done', ...fields } as Task])[0];
const card = (t: Task): KanbanCard => ({ task: t, nextUp: false });

describe('stageOf', () => {
  it('a done task is Done, In PR with a pr, Merged with merged_at', () => {
    expect(stageOf(task('A'))).toBe('done');
    expect(stageOf(task('A', { pr: { number: 3, url: 'u' } }))).toBe('in_pr');
    expect(stageOf(task('A', { pr: { number: 3 }, merged_at: '2026-04-01T00:00:00Z' }))).toBe('merged');
  });

  it('tasks that are not done have no stage; a malformed pr is ignored', () => {
    expect(stageOf(task('A', { status: 'in_progress', pr: { number: 3 } }))).toBeUndefined();
    expect(stageOf(task('A', { pr: { url: 'u' } as unknown as Task['pr'] }))).toBe('done');
    expect(hasStage(task('A'))).toBe(false);
    expect(hasStage(task('A', { merged_at: '2026-04-01T00:00:00Z' }))).toBe(true);
  });

  // ATM-680, ATM-719: atm's `derived.stage` is the task's, whatever its own fields say; no rule of the dashboard's
  it('is atm’s stage; one it doesn’t know, or none, is no stage', () => {
    const served = (stage: string | undefined, fields: Partial<Task> = {}) => {
      const t = task('A', fields);
      return { ...t, derived: { ...t.derived!, stage } } as Task;
    };
    expect(stageOf(served('merged'))).toBe('merged');
    expect(stageOf(served('in_pr', { merged_at: '2026-04-01T00:00:00Z' }))).toBe('in_pr');
    expect(stageOf(served('shipped', { pr: { number: 3 } }))).toBeUndefined();
    expect(stageOf(served(undefined, { pr: { number: 3 } }))).toBeUndefined();
  });
});

describe('doneSections', () => {
  it('In PR per PR (newest PR first), then Done, then Merged; empty sections left out; order kept', () => {
    const cards = [
      task('A', { pr: { number: 7, url: 'https://example.com/pr/7' } }),
      task('B'),
      task('C', { pr: { number: 9, url: 'javascript:alert(9)' } }),
      task('D', { pr: { number: 5 }, merged_at: '2026-04-01T00:00:00Z' }),
      task('E', { pr: { number: 7, url: 'https://example.com/pr/7' } }),
    ].map(card);
    const sections = doneSections(cards);
    expect(sections.map((s) => [s.label, s.cards.map((c) => c.task.id)])).toEqual([
      ['In PR #9', ['C']],
      ['In PR #7', ['A', 'E']],
      ['Done', ['B']],
      ['Merged', ['D']],
    ]);
    expect(sections[1].url).toBe('https://example.com/pr/7');
    expect(sections[0].url).toBeUndefined(); // a javascript: URL is never a link (T-225)
    expect(doneSections([card(task('B'))]).map((s) => s.id)).toEqual(['done']);
  });
});
