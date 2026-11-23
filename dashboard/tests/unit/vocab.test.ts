import { describe, expect, it } from 'vitest';
import type { ProjectVocabulary } from '$generated/types';
import {
  DEFAULT_PRIORITIES,
  DEFAULT_STATUSES,
  DEFAULT_TYPES,
  DEFAULT_VOCABULARY,
  fromServed,
  humanize,
  kindOf,
  phaseLabel,
  priorityLabel,
  priorityOf,
  priorityRank,
  statusLabel,
  statusRank,
  taskKind,
  typeLabel,
} from '../../src/lib/vocab';

// A project's vocabulary as atm serves it (the tasks response's `vocabulary`, ATM-718): atm-lib resolves meta
const custom: ProjectVocabulary = {
  statuses: [
    { id: 'backlog', label: 'Backlog', kind: 'todo' },
    { id: 'doing', label: 'Doing', kind: 'active' },
    { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
    { id: 'shipped', label: 'Shipped', kind: 'done' },
  ],
  types: [
    { id: 'spike', label: 'Spike' },
    { id: 'chore', label: 'Chore' },
  ],
  priorities: [
    { id: 'must', label: 'Must' },
    { id: 'should', label: 'Should' },
    { id: 'could', label: 'Could' },
  ],
  default_priority: 'should',
  labels: [{ id: 'ux', label: 'UX' }],
  components: [{ id: 'api', label: 'API', paths: ['crates/**'], review: 'async' }],
  workflow: {
    type: 'pull-request',
    base: 'main',
    verify: true,
    max_rounds: 3,
    commit_template: 'feat({id}): {name}',
    trailers: { require: ['Signed-off-by: x'] },
  },
  review: 'blocker',
  phases: [{ id: 1, name: 'Foundations' }],
};

describe('fromServed', () => {
  it('the defaults until atm has served a vocabulary, in_review included', () => {
    expect(fromServed(undefined)).toBe(DEFAULT_VOCABULARY);
    expect(DEFAULT_STATUSES.map((s) => `${s.id}:${s.kind}`)).toEqual([
      'todo:todo',
      'in_progress:active',
      'in_review:review',
      'blocked:blocked',
      'done:done',
      'skipped:skipped',
    ]);
    expect(DEFAULT_TYPES.map((t) => t.id)).toEqual([
      'feature',
      'bugfix',
      'test',
      'infra',
      'docs',
      'refactor',
      'polish',
    ]);
    expect(DEFAULT_PRIORITIES.map((p) => p.id)).toEqual(['P0', 'P1', 'P2', 'P3']);
    expect(DEFAULT_VOCABULARY.defaultPriority).toBe('P2');
  });

  it("takes the project's lists as atm serves them", () => {
    const v = fromServed(custom);
    expect(v.statuses.map((s) => s.id)).toEqual(['backlog', 'doing', 'awaiting_user', 'shipped']);
    expect(v.types).toEqual(custom.types);
    expect(v.priorities.map((p) => p.id)).toEqual(['must', 'should', 'could']);
    expect(v.defaultPriority).toBe('should');
    expect([...v.phaseNames]).toEqual([[1, 'Foundations']]);
    expect(v.labels).toEqual([{ id: 'ux', label: 'UX' }]);
    expect(v.components).toEqual(custom.components);
  });

  it('the workflow: verify on unless atm says false, and the trailers commits must carry; none without one', () => {
    expect(fromServed(custom).workflow).toEqual({
      type: 'pull-request',
      base: 'main',
      verify: true,
      commitTemplate: 'feat({id}): {name}',
      trailers: ['Signed-off-by: x'],
    });
    const direct = fromServed({ ...custom, workflow: { type: 'direct', verify: false } }).workflow;
    expect(direct).toMatchObject({ type: 'direct', verify: false, trailers: [] });
    expect(fromServed({ ...custom, workflow: null }).workflow).toBeNull();
  });
});

describe('taskKind', () => {
  it("is atm's kind for the task; none for one atm gives none, or a kind it doesn't know", () => {
    expect(taskKind({ derived: { kind: 'review' } })).toBe('review');
    expect(taskKind({ derived: { kind: '' } })).toBeUndefined();
    expect(taskKind({ derived: { kind: 'waiting' } })).toBeUndefined();
    expect(taskKind({})).toBeUndefined();
  });
});

describe('lookups', () => {
  const v = fromServed(custom);

  it('finds kinds, falling back to the default statuses, else none', () => {
    expect(kindOf(v, 'awaiting_user')).toBe('review');
    expect(kindOf(v, 'in_progress')).toBe('active'); // a default status the project didn't list
    expect(kindOf(v, 'weird')).toBeUndefined();
  });

  it('labels statuses, types and phases', () => {
    expect(statusLabel(v, 'shipped')).toBe('Shipped');
    expect(statusLabel(v, 'in_review')).toBe('In Review');
    expect(statusLabel(v, 'on_hold')).toBe('On hold');
    expect(typeLabel(v, 'chore')).toBe('Chore');
    expect(typeLabel(v, 'bug')).toBe('Bug');
    expect(phaseLabel(v, 1)).toBe('Phase 1 · Foundations');
    expect(phaseLabel(v, 2)).toBe('Phase 2');
    expect(humanize('in_progress')).toBe('In progress');
  });

  it('resolves priorities: missing or unknown ones are the default, ranked like it', () => {
    expect(priorityOf(v, { priority: 'must' })).toBe('must');
    expect(priorityOf(v, {})).toBe('should');
    expect(priorityOf(v, { priority: 'P0' })).toBe('should');
    // atm's answer, when it gave one, is the task's (ATM-680): the local rule is only for a task it hasn't served
    expect(priorityOf(v, { priority: 'P0', derived: { priority: 'could' } })).toBe('could');
    expect(priorityOf(v, { priority: 'must', derived: { priority: '' } })).toBe('must');
    expect([priorityRank(v, 'must'), priorityRank(v, 'could'), priorityRank(v, 'nope')]).toEqual([0, 2, 1]);
    expect(priorityLabel(v, 'could')).toBe('Could');
    expect(priorityLabel(v, 'P0')).toBe('P0');
  });

  it("ranks statuses in the vocabulary's order, unknown ones last", () => {
    expect(statusRank(v, 'backlog')).toBe(0);
    expect(statusRank(v, 'shipped')).toBe(3);
    expect(statusRank(v, 'weird')).toBe(4);
  });
});
