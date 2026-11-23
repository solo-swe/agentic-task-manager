import { describe, expect, it } from 'vitest';
import { taskCardState, type CardContext } from '../../src/lib/card-state';
import type { Claim } from '../../src/lib/claims';
import { phaseColor } from '../../src/lib/theme';
import type { Task } from '../../src/lib/types';
import { sprintModel, viewed, withKinds } from './views';

// A card's state (ATM-683), the same for the Task List's card and the Kanban's
const NOW = Date.parse('2026-05-01T12:00:00Z');
const meta = {
  sprints: [
    { id: 'S1', state: 'closed' },
    { id: 'S2', state: 'active' },
  ],
};
const [inS2, inBacklog, active, inReview, leftover] = withKinds([
  { id: 'T-1', name: '1', type: 'feature', status: 'todo', sprint: 'S2' },
  { id: 'T-2', name: '2', type: 'bugfix', status: 'todo' },
  { id: 'T-3', name: '3', type: 'feature', status: 'in_progress', sprint: 'S2' },
  { id: 'T-4', name: '4', type: 'feature', status: 'in_review', sprint: 'S2' },
  { id: 'T-5', name: '5', type: 'feature', status: 'todo', sprint: 'S1' },
] as Task[]);
const claim: Claim = {
  task: 'T-3',
  agent: 'claude:a',
  branch: 'dev/x',
  claimed_at: '2026-05-01T11:00:00Z',
  lease_expires_at: '2026-05-01T13:00:00Z',
};
const context = (tasks: Task[]): CardContext => ({
  sprints: sprintModel(meta, tasks),
  leftovers: [leftover],
  claims: new Map([['T-3', claim]]),
  loadedAt: NOW,
});

describe('taskCardState', () => {
  const all = [inS2, inBacklog, active, inReview, leftover];

  it('a sprint keeps its colour by its place; the backlog is muted', () => {
    expect(taskCardState(inS2, context(all))).toMatchObject({ sprint: 'S2', sprintColor: phaseColor(1) });
    expect(taskCardState(inBacklog, context(all))).toMatchObject({
      sprint: undefined,
      sprintColor: 'var(--text-muted)',
    });
  });

  it('stranded only when open in a closed sprint', () => {
    expect(taskCardState(leftover, context(all)).stranded).toBe(true);
    expect(taskCardState(inS2, context(all)).stranded).toBe(false);
  });

  it('the claim only while active; the code review only while in review', () => {
    expect(taskCardState(active, context(all)).claim).toBe(claim);
    // A claim on a task that isn't active (a stale one) isn't shown
    const todoClaimed = { ...context(all), claims: new Map([['T-1', { ...claim, task: 'T-1' }]]) };
    expect(taskCardState(inS2, todoClaimed).claim).toBeUndefined();
    const review = { id: 'r1', reference: 'CR-1', status: 'waiting' } as const;
    const reviewed = viewed(inReview, { review } as never);
    expect(taskCardState(reviewed, context(all)).codeReview).toEqual(review);
    expect(taskCardState(viewed(inS2, { review } as never), context(all)).codeReview).toBeNull();
  });

  it('its kind and type style come with it', () => {
    const state = taskCardState(inBacklog, context(all));
    expect(state.kind).toBe('todo');
    expect(state.typeStyle).toBeTruthy();
  });
});
