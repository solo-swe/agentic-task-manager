import { describe, expect, it } from 'vitest';
import type { ProjectEntry, ProjectSummary } from '../../src/lib/load';
import { idHash, isActive, monogram, needsYou, openTasks, orderProjects, statsLine } from '../../src/lib/projects';
import { summary } from './summary.helpers';

describe('statsLine (T-255)', () => {
  const now = new Date(2026, 9, 26, 12).getTime();
  const local = (y: number, m: number, d: number) => new Date(y, m, d, 12).toISOString();

  it('started, tasks and agent time in whole hours; the year only when it is another', () => {
    const day = (iso: string, year?: 'numeric') =>
      new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year });
    const aug = local(2026, 7, 12);
    expect(statsLine({ started: aug, tasks: 236, agent_minutes: 4440 }, now)).toBe(
      `started ${day(aug)} · 236 tasks · 74 h agent time`,
    );
    const old = local(2025, 1, 3);
    expect(statsLine({ started: old, tasks: 1, agent_minutes: 45 }, now)).toBe(
      `started ${day(old, 'numeric')} · 1 task · 45 m agent time`,
    );
  });

  it('leaves out what is not known: no start, no agent time', () => {
    expect(statsLine({ started: null, tasks: 3, agent_minutes: 0 }, now)).toBe('3 tasks');
    expect(statsLine({ started: 'garbage', tasks: 0, agent_minutes: 0 }, now)).toBe('0 tasks');
  });
});

describe('monogram and idHash', () => {
  it('initials of the first two words, else the first two letters, upper case', () => {
    expect(monogram('Agentic Task Manager')).toBe('AT');
    expect(monogram('my-cool_app')).toBe('MC');
    expect(monogram('atm')).toBe('AT');
    expect(monogram('Ünïcode')).toBe('ÜN');
    expect(monogram('x')).toBe('X');
    expect(monogram('—')).toBe('?');
  });

  it('a stable, non-negative number per id', () => {
    expect(idHash('abcd1234')).toBe(idHash('abcd1234'));
    expect(idHash('abcd1234')).not.toBe(idHash('abcd1235'));
    expect(idHash('')).toBe(0);
    expect(idHash('zzzzzzzzzzzzzzzzzzzzzzzz')).toBeGreaterThanOrEqual(0);
  });
});

const NOW = Date.parse('2026-03-01T12:00:00Z');

function project(id: string, fields: Partial<ProjectSummary> | null = {}, missing = false): ProjectEntry {
  const numbers =
    fields === null
      ? null
      : summary({ total: 10, done: 5, percent: 50, last_activity: '2026-03-01T10:00:00Z', ...fields });
  return { id, name: id, path: `/${id}`, missing, summary: numbers };
}

describe('needsYou and openTasks', () => {
  it('count reviews and questions for the user; open work is neither done nor skipped', () => {
    const needs_you = { review: 2, blocked_on_user: 1, together: 0 };
    const p = project('a', { total: 10, done: 4, skipped: 3, needs_you });
    expect([needsYou(p), openTasks(p)]).toEqual([3, 3]);
    expect([needsYou(project('b', null)), openTasks(project('b', null))]).toEqual([0, 0]);
  });
});

describe('isActive', () => {
  it('open work, or activity in the last 14 days', () => {
    const finished = { total: 4, done: 4 };
    expect(isActive(project('open', { last_activity: '2025-01-01T00:00:00Z' }), NOW)).toBe(true);
    expect(isActive(project('recent', { ...finished, last_activity: '2026-02-20T00:00:00Z' }), NOW)).toBe(true);
    expect(isActive(project('idle', { ...finished, last_activity: '2026-02-01T00:00:00Z' }), NOW)).toBe(false);
    expect(isActive(project('never', { ...finished, last_activity: null }), NOW)).toBe(false);
    expect(isActive(project('idle', { ...finished, last_activity: '2026-02-01T00:00:00Z' }), NOW, 60)).toBe(true);
  });

  it('a project whose folder is gone, or without a summary, stays visible', () => {
    expect(isActive(project('gone', null, true), NOW)).toBe(true);
    expect(isActive(project('unsummarized', null), NOW)).toBe(true);
  });
});

describe('orderProjects', () => {
  it('needs you first, then the most recent activity, then the name; gone ones last', () => {
    const asking = summary().needs_you;
    const list = [
      project('gone', null, true),
      project('old', { last_activity: '2026-01-01T00:00:00Z' }),
      project('asks', { needs_you: { ...asking, review: 1 }, last_activity: '2025-12-01T00:00:00Z' }),
      project('new', { last_activity: '2026-02-28T00:00:00Z' }),
      project('b-quiet', { last_activity: null }),
      project('a-quiet', { last_activity: null }),
    ];
    expect(orderProjects(list).map((p) => p.id)).toEqual(['asks', 'new', 'old', 'a-quiet', 'b-quiet', 'gone']);
  });
});
