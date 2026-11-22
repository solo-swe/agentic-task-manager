import { describe, expect, it } from 'vitest';
import { agentTimeline, timeTicks, UNATTRIBUTED } from '../../src/lib/agents';
import type { Claim } from '../../src/lib/claims';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { vocabulary, withKinds } from './views';

/** The timeline of `tasks` as atm serves them: with their kinds, in `vocab` */
const timeline = (
  tasks: Task[],
  claims: Parameters<typeof agentTimeline>[1],
  now: number,
  matching?: ReadonlySet<string>,
  vocab: Vocabulary = DEFAULT_VOCABULARY,
) => agentTimeline(withKinds(tasks, vocab), claims, now, matching);

const M = 60_000;
const T0 = Date.parse('2026-04-01T08:00:00Z');
const iso = (ms: number) => new Date(ms).toISOString();

/** A task started at T0 + `start` minutes and, when `end` is given, done at T0 + `end` minutes */
const worked = (id: string, start: number, end: number | null, agent?: string): Task => ({
  id,
  name: `Task ${id}`,
  phase: 0,
  type: 'feature',
  description: '',
  status: end === null ? 'in_progress' : 'done',
  started_at: iso(T0 + start * M),
  ...(end === null ? {} : { completed_at: iso(T0 + end * M) }),
  ...(agent ? { agent } : {}),
});

const claim = (task: string, agent: string): Claim => ({
  task,
  agent,
  branch: null,
  claimed_at: iso(T0),
  lease_expires_at: iso(T0 + 600 * M),
});

describe('agentTimeline', () => {
  it('ends a bar when the task asked for a review; reviews are finished work but not done', () => {
    const reviewing = { ...worked('a', 0, null, 'alice'), status: 'in_review', review_requested_at: iso(T0 + 15 * M) };
    const approved = { ...worked('b', 20, 120, 'alice'), review_requested_at: iso(T0 + 30 * M) };
    const t = timeline([reviewing, approved], new Map(), T0 + 200 * M)!;
    const [alice] = t.lanes;
    expect(alice.bars.map((b) => [b.task.id, (b.end - b.start) / M, b.live])).toEqual([
      ['a', 15, false],
      ['b', 10, false],
    ]);
    expect(alice).toMatchObject({ done: 1, medianMinutes: 12.5, current: [] });
  });

  it("follows a project's statuses by kind", () => {
    const vocab = vocabulary({
      statuses: [
        { id: 'backlog', label: 'Backlog', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
      ],
    });
    const t = timeline(
      [
        { ...worked('a', 0, 10, 'alice'), status: 'shipped' },
        { ...worked('b', 20, null, 'alice'), status: 'doing' },
      ],
      new Map(),
      T0 + 60 * M,
      undefined,
      vocab,
    )!;
    expect(t.lanes[0]).toMatchObject({ done: 1, current: [expect.objectContaining({ id: 'b' })] });
  });

  it('is null when no task has been started', () => {
    expect(timeline([], new Map(), T0)).toBeNull();
    const todo = { ...worked('a', 0, 10), status: 'todo' };
    const noStart = { ...worked('b', 0, 10), started_at: undefined };
    expect(timeline([todo, noStart], new Map(), T0)).toBeNull();
  });

  it('gives each agent a lane of bars from start to completion, in time order', () => {
    const t = timeline(
      [worked('b', 30, 50, 'alice'), worked('a', 0, 20, 'alice'), worked('c', 10, 40, 'bob')],
      new Map(),
      T0 + 60 * M,
    )!;
    expect(t.start).toBe(T0);
    expect(t.end).toBe(T0 + 50 * M);
    const alice = t.lanes.find((l) => l.agent === 'alice')!;
    expect(alice.bars.map((b) => [b.task.id, (b.end - b.start) / M, b.live])).toEqual([
      ['a', 20, false],
      ['b', 20, false],
    ]);
    expect(alice).toMatchObject({ done: 2, medianMinutes: 20, current: [], lastActive: T0 + 50 * M });
  });

  it('work in progress runs to now and belongs to the agent holding the claim', () => {
    const now = T0 + 45 * M;
    const t = timeline(
      [worked('a', 0, 20, 'alice'), worked('b', 30, null, 'alice')],
      new Map([['b', claim('b', 'bob')]]),
      now,
    )!;
    const bob = t.lanes.find((l) => l.agent === 'bob')!;
    expect(bob.bars[0]).toMatchObject({ start: T0 + 30 * M, end: now, live: true });
    expect(bob.current.map((task) => task.id)).toEqual(['b']);
    expect(bob).toMatchObject({ done: 0, medianMinutes: null });
    expect(t.end).toBe(now);
  });

  it('a task without a recorded agent goes to the unattributed lane, which comes last', () => {
    const t = timeline([worked('old', 50, 60), worked('a', 0, 10, 'alice')], new Map(), T0)!;
    expect(t.lanes.map((l) => l.agent)).toEqual(['alice', UNATTRIBUTED]);
  });

  it('lanes are ordered by most recent activity', () => {
    const t = timeline(
      [worked('a', 0, 10, 'alice'), worked('b', 0, 30, 'bob'), worked('c', 0, 20, 'carol')],
      new Map(),
      T0,
    )!;
    expect(t.lanes.map((l) => l.agent)).toEqual(['bob', 'carol', 'alice']);
  });

  it('a done task without completion time is a zero-length bar; one ending before it started has none, as atm says', () => {
    const noEnd = { ...worked('a', 10, 20, 'alice'), completed_at: undefined };
    const backwards = worked('b', 30, 25, 'alice');
    const bars = timeline([noEnd, backwards], new Map(), T0)!.lanes[0].bars;
    expect(bars.map((b) => [b.task.id, b.end - b.start])).toEqual([['a', 0]]);
  });

  it('a bar per stretch of its work, in the lane of who did it (ATM-721): an attempt taken back is another agent’s', () => {
    const t = worked('a', 60, 90, 'bob');
    // atm's work: alice's attempt that lapsed, then bob's
    const work = {
      minutes: 50,
      live: false,
      spans: [
        { from: iso(T0), to: iso(T0 + 20 * M), by: 'alice' },
        { from: iso(T0 + 60 * M), to: iso(T0 + 90 * M), by: 'bob' },
      ],
    };
    const lanes = timeline([{ ...t, derived: { ...t.derived, work } as Task['derived'] }], new Map(), T0)!.lanes;
    expect(lanes.map((l) => [l.agent, l.bars.map((b) => (b.end - b.start) / M), l.done])).toEqual([
      // Done in bob's lane, who finished it; alice's attempt was taken over
      ['bob', [30], 1],
      ['alice', [20], 0],
    ]);
  });
});

describe('timeTicks', () => {
  it('places about six ticks on round times', () => {
    const ticks = timeTicks(T0 + 7 * M, T0 + 6 * 60 * M);
    expect(ticks.length).toBeGreaterThanOrEqual(3);
    expect(ticks.length).toBeLessThanOrEqual(6);
    for (const t of ticks) expect(new Date(t).getMinutes()).toBe(0);
    expect(ticks[0]).toBeGreaterThanOrEqual(T0 + 7 * M);
  });

  it('uses minute steps for short spans', () => {
    const ticks = timeTicks(T0, T0 + 5 * M);
    expect(ticks[1] - ticks[0]).toBe(M);
  });

  it('caps the step at a week for very long spans', () => {
    const ticks = timeTicks(T0, T0 + 365 * 24 * 60 * M);
    expect(ticks[1] - ticks[0]).toBe(7 * 24 * 60 * M);
  });
});
