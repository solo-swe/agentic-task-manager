import { describe, expect, it } from 'vitest';
import {
  ACTIVE,
  BACKLOG,
  emptyFilters,
  facetCounts,
  filterOptions,
  filterTasks,
  isActive,
  searchTasks,
  shownSprintOptions,
  sortTasks,
  sprintSelection,
  toggleFilter,
  type Filters,
} from '../../src/lib/filters';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import standardFile from '../fixtures/standard.json';
import edgeFile from '../fixtures/edge-cases.json';
import sprintsFile from '../fixtures/sprints.json';

describe('shownSprintOptions (SR-7.1)', () => {
  const sprints = (n: number) => Array.from({ length: n }, (_, i) => ({ value: `S${i}`, label: `S${i}` }));
  const values = (r: { shown: { value: string }[] }) => r.shown.map((o) => o.value);

  it('lists every sprint when there are few', () => {
    const r = shownSprintOptions(sprints(6), new Set(['S0']), new Set(), false);
    expect(values(r)).toEqual(['S0', 'S1', 'S2', 'S3', 'S4', 'S5']);
    expect(r).toMatchObject({ closed: 0, hidden: 0 });
  });

  it('with many, only the open ones and any selected closed one; the toggle shows the rest', () => {
    const closed = new Set(['S3', 'S4', 'S5', 'S6', 'S7', 'S8', 'S9']);
    const r = shownSprintOptions(sprints(10), closed, new Set(['S5']), false);
    expect(values(r)).toEqual(['S0', 'S1', 'S2', 'S5']);
    expect(r).toMatchObject({ closed: 7, hidden: 6 });
    const all = shownSprintOptions(sprints(10), closed, new Set(['S5']), true);
    expect(values(all)).toHaveLength(10);
    expect(all).toMatchObject({ closed: 7, hidden: 0 });
  });

  it('nothing to toggle when no sprint is closed', () => {
    const r = shownSprintOptions(sprints(8), new Set(), new Set(), false);
    expect(values(r)).toHaveLength(8);
    expect(r).toMatchObject({ closed: 0, hidden: 0 });
  });
});

const standard = standardFile.tasks as Task[];
// standard.json has phases 0-2 and no sprints: read as the sprints Phase 0 (all done: closed), Phase 1, Phase 2
const model = sprintModel(standardFile.meta, standard, resolveVocabulary(standardFile.meta));
const ids = (tasks: Task[]) => tasks.map((t) => t.id);
const filters = (f: Partial<Record<keyof Filters, string[]>>): Filters => ({
  sprint: new Set(f.sprint),
  release: new Set(f.release),
  component: new Set(f.component),
  type: new Set(f.type),
  status: new Set(f.status),
  priority: new Set(f.priority),
  stage: new Set(f.stage),
  label: new Set(f.label),
  agent: new Set(f.agent),
});

describe('filterOptions', () => {
  it("derives options in the vocabulary's order, with its labels", () => {
    const o = filterOptions(standard, undefined, model);
    expect(o.sprint.map((x) => x.label)).toEqual(['Phase 1', 'Phase 2', 'Phase 0']);
    expect(o.type.map((x) => x.value)).toEqual(['feature', 'bugfix', 'test', 'infra', 'docs', 'refactor', 'polish']);
    expect(o.type[1].label).toBe('Bugfix');
    expect(o.status).toEqual([
      { value: 'todo', label: 'To Do' },
      { value: 'in_progress', label: 'In Progress' },
      { value: 'in_review', label: 'In Review' }, // no task is in review: still offered
      { value: 'blocked', label: 'Blocked' },
      { value: 'done', label: 'Done' },
      { value: 'skipped', label: 'Skipped' },
    ]);
  });

  it('offers every status of the vocabulary, also when all tasks have the same one', () => {
    const allDone = standard.map((t) => ({ ...t, status: 'done' }));
    expect(filterOptions(allDone).status.map((x) => x.value)).toEqual([
      'todo',
      'in_progress',
      'in_review',
      'blocked',
      'done',
      'skipped',
    ]);
  });

  it("uses a project's vocabulary: its order and labels, phase names; values it doesn't list come last", () => {
    const meta = {
      statuses: [
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'todo', label: 'Backlog', kind: 'todo' },
        { id: 'done', label: 'Shipped', kind: 'done' },
      ],
      types: [{ id: 'test', label: 'Tests' }],
      phases: [{ id: 1, name: 'Core' }],
    };
    const vocab = resolveVocabulary(meta);
    const o = filterOptions(standard, vocab, sprintModel(meta, standard, vocab));
    // Doing has no task but is the project's: listed. Statuses it doesn't list but tasks use come last
    expect(o.status.map((x) => x.label)).toEqual(['Doing', 'Backlog', 'Shipped', 'Blocked', 'In Progress', 'Skipped']);
    expect(o.type.slice(0, 3).map((x) => x.label)).toEqual(['Tests', 'Bugfix', 'Docs']);
    // A named phase reads as a sprint called after it
    expect(o.sprint.map((x) => x.label)).toEqual(['Core', 'Phase 2', 'Phase 0']);
  });

  it('lists sprints in their order (phases numerically, not as strings)', () => {
    const t = { name: 'x', type: 'feature', description: '', status: 'todo' };
    const tasks = [10, 2, 1].map((phase, i) => ({ ...t, id: `T-${i}`, phase })) as Task[];
    const m = sprintModel({}, tasks, resolveVocabulary({}));
    expect(filterOptions(tasks, undefined, m).sprint.map((x) => x.value)).toEqual(['Phase 1', 'Phase 2', 'Phase 10']);
  });

  it('puts unknown statuses last', () => {
    expect(filterOptions(edgeFile.tasks as Task[]).status.map((x) => x.value)).toEqual([
      'todo',
      'in_progress',
      'in_review',
      'blocked',
      'done',
      'skipped',
      'review',
    ]);
  });

  it('is empty for no tasks, except the statuses, which come from the vocabulary', () => {
    expect({ ...filterOptions([]), status: [] }).toEqual({
      sprint: [],
      release: [],
      component: [],
      type: [],
      status: [],
      priority: [],
      stage: [],
      label: [],
      agent: [],
    });
  });

  it('offers the priorities in the data, most urgent first; a missing or unknown priority is the default', () => {
    const tasks = [
      { ...standard[0], priority: 'P3' },
      { ...standard[1], priority: 'P0' },
      { ...standard[2], priority: 'urgent!' },
      standard[3],
    ];
    expect(filterOptions(tasks).priority).toEqual([
      { value: 'P0', label: 'P0 Critical' },
      { value: 'P2', label: 'P2 Medium' },
      { value: 'P3', label: 'P3 Low' },
    ]);
    const p2 = filterTasks(tasks, filters({ priority: ['P2'] }));
    expect(ids(p2)).toEqual([standard[2].id, standard[3].id]);
    expect(facetCounts(tasks, filters({})).priority.values.get('P2')).toBe(2);
  });
});

describe('sprints, releases and components (SR-7.1)', () => {
  const file = sprintsFile as TasksFile;
  const tasks = file.tasks as Task[];
  const m = sprintModel(file.meta, tasks, resolveVocabulary(file.meta));
  const withComponents = tasks.map((t, i) => (i < 2 ? { ...t, components: ['api', 'ui'] } : t));

  it('the Sprint group: the active sprint, the planned ones, the backlog, then the closed ones', () => {
    // Alan Turing (planned) has no task here: left out. The active one is ACTIVE: whichever sprint is active (T-323)
    expect(filterOptions(tasks, undefined, m).sprint).toEqual([
      { value: ACTIVE, label: 'Ada Lovelace', state: 'active' },
      { value: BACKLOG, label: 'Backlog' },
      { value: 'Foundations', label: 'Foundations', state: 'closed' },
    ]);
  });

  it('ACTIVE selects the active sprint, and nothing while none is active (T-323)', () => {
    expect(ids(filterTasks(tasks, filters({ sprint: [ACTIVE] }), undefined, m))).toEqual(['T-002', 'T-003', 'T-004']);
    expect(facetCounts(tasks, filters({}), undefined, m).sprint.values.get(ACTIVE)).toBe(3);
    const none = sprintModel(
      { ...file.meta, sprints: (file.meta!.sprints as object[]).map((s) => ({ ...s, state: 'closed' })) },
      tasks,
      resolveVocabulary(file.meta),
    );
    expect(sprintSelection(new Set([ACTIVE, BACKLOG]), none)).toEqual(new Set([BACKLOG]));
    expect(filterTasks(tasks, filters({ sprint: [ACTIVE] }), undefined, none)).toHaveLength(tasks.length);
    const selected = new Set([ACTIVE]);
    expect(sprintSelection(selected, m)).toBe(selected);
  });

  it('filters by sprint (a name in any case) and by the backlog', () => {
    expect(ids(filterTasks(tasks, filters({ sprint: ['Ada Lovelace'] }), undefined, m))).toEqual([
      'T-002',
      'T-003',
      'T-004',
    ]);
    expect(ids(filterTasks(tasks, filters({ sprint: [BACKLOG] }), undefined, m))).toEqual(['T-005']);
  });

  it("a task's release is its own, else its sprint's; tasks without one match only All", () => {
    expect(filterOptions(tasks, undefined, m).release.map((o) => o.value)).toEqual(['1.0', '1.1']);
    expect(ids(filterTasks(tasks, filters({ release: ['1.0'] }), undefined, m))).toEqual(['T-001', 'T-002', 'T-003']);
    expect(facetCounts(tasks, filters({}), undefined, m).release.values.get('1.1')).toBe(1);
  });

  it('components: a task matches any of its components', () => {
    expect(filterOptions(withComponents, undefined, m).component.map((o) => o.value)).toEqual(['api', 'ui']);
    expect(ids(filterTasks(withComponents, filters({ component: ['ui'] }), undefined, m))).toEqual(['T-001', 'T-002']);
  });
});

describe('labels', () => {
  const withLabels = [
    { ...standard[0], labels: ['ux', 'spike-2'] },
    { ...standard[1], labels: ['ux'] },
    { ...standard[2], labels: ['api', 'ux', 7 as unknown as string] },
    standard[3],
  ];

  it('offers the labels in the data, alphabetically; meta.labels sets their text and order first', () => {
    expect(filterOptions(withLabels).label).toEqual([
      { value: 'api', label: 'api' },
      { value: 'spike-2', label: 'spike-2' },
      { value: 'ux', label: 'ux' },
    ]);
    const vocab = resolveVocabulary({ labels: [{ id: 'ux', label: 'UX' }, { id: 'nope' }] });
    expect(filterOptions(withLabels, vocab).label.map((o) => o.label)).toEqual(['UX', 'api', 'spike-2']);
  });

  it('a task matches when it has any selected label; tasks without labels match only "All"', () => {
    expect(ids(filterTasks(withLabels, filters({ label: ['spike-2', 'api'] })))).toEqual([
      standard[0].id,
      standard[2].id,
    ]);
    expect(filterTasks(withLabels, filters({}))).toHaveLength(4);
  });

  it('counts each label once per task that has it', () => {
    const c = facetCounts(withLabels, filters({}));
    expect(Object.fromEntries(c.label.values)).toEqual({ ux: 3, 'spike-2': 1, api: 1 });
    expect(c.label.all).toBe(4);
    expect(facetCounts(withLabels, filters({ label: ['api'] })).sprint.all).toBe(1);
  });

  it('search matches labels', () => {
    expect(ids(searchTasks(withLabels, 'spike-2'))).toEqual([standard[0].id]);
  });
});

describe('the stage group', () => {
  it('offers the stages done tasks have, in order; filters by them; tasks not done have none', () => {
    const done = standard.filter((t) => t.status === 'done');
    const tasks = [
      { ...done[0], merged_at: '2026-04-01T00:00:00Z', pr: { number: 2 } },
      { ...done[1], pr: { number: 3, url: 'u' } },
      done[2],
      { ...standard.find((t) => t.status === 'todo')!, pr: { number: 3 } },
    ];
    expect(filterOptions(tasks).stage).toEqual([
      { value: 'done', label: 'Done' },
      { value: 'in_pr', label: 'In PR' },
      { value: 'merged', label: 'Merged' },
    ]);
    expect(ids(filterTasks(tasks, filters({ stage: ['in_pr'] })))).toEqual([done[1].id]);
  });
});

describe('the agent group', () => {
  const started = [
    { ...standard[0], started_by: 'claude:aaaa' },
    { ...standard[1], agent: 'claude:bbbb' },
    { ...standard[2], started_by: 'alice@host:/repo' },
    standard[3],
  ];

  it('offers who started tasks (an older agent field too), shortened; filters and counts by it', () => {
    expect(filterOptions(started).agent).toEqual([
      { value: 'alice@host:/repo', label: 'alice@host' },
      { value: 'claude:aaaa', label: 'claude:aaaa' },
      { value: 'claude:bbbb', label: 'claude:bbbb' },
    ]);
    expect(ids(filterTasks(started, filters({ agent: ['claude:bbbb'] })))).toEqual([standard[1].id]);
    expect(facetCounts(started, filters({})).agent.values.get('claude:aaaa')).toBe(1);
  });
});

describe('sortTasks', () => {
  const t = (id: string, phase: number, priority?: string): Task => ({
    id,
    name: id,
    phase,
    type: 'feature',
    description: '',
    status: 'todo',
    ...(priority ? { priority } : {}),
  });
  const tasks = [t('T-10', 0, 'P1'), t('T-2', 1, 'P0'), t('T-3', 0), t('T-1', 0, 'P1'), t('T-4', 0, 'P0')];

  it('keeps the file order, or sorts by priority, then phase, then ID', () => {
    expect(ids(sortTasks(tasks, 'file'))).toEqual(['T-10', 'T-2', 'T-3', 'T-1', 'T-4']);
    expect(ids(sortTasks(tasks, 'priority'))).toEqual(['T-4', 'T-2', 'T-1', 'T-10', 'T-3']);
  });

  it("uses the project's priorities", () => {
    const vocab = resolveVocabulary({ priorities: [{ id: 'P1' }, { id: 'P0' }], default_priority: 'P0' });
    expect(ids(sortTasks(tasks, 'priority', vocab))).toEqual(['T-1', 'T-10', 'T-3', 'T-4', 'T-2']);
  });
});

describe('filterTasks', () => {
  it('returns everything with no filters', () => {
    expect(filterTasks(standard, emptyFilters())).toHaveLength(12);
  });

  it('ORs values within a group', () => {
    expect(filterTasks(standard, filters({ sprint: ['Phase 1', 'Phase 2'] }), undefined, model)).toHaveLength(8);
  });

  it('ANDs across groups', () => {
    const f = filters({ sprint: ['Phase 1', 'Phase 2'], type: ['test'] });
    expect(ids(filterTasks(standard, f, undefined, model))).toEqual(['T-009']);
    expect(ids(filterTasks(standard, filters({ type: ['feature'], status: ['done'] })))).toEqual(['T-004']);
  });

  it('can match nothing', () => {
    expect(filterTasks(standard, filters({ status: ['blocked'], type: ['docs'] }))).toEqual([]);
  });

  it('keeps file order', () => {
    expect(ids(filterTasks(standard, filters({ status: ['done'] })))).toEqual([
      'T-001',
      'T-002',
      'T-003',
      'T-004',
      'T-012',
    ]);
  });
});

describe('toggleFilter / isActive', () => {
  it('toggles values without mutating the input', () => {
    const start = emptyFilters();
    const one = toggleFilter(start, 'type', 'test');
    expect(start.type.size).toBe(0);
    expect([...one.type]).toEqual(['test']);
    expect(toggleFilter(one, 'type', 'test').type.size).toBe(0);
  });

  it('"all" clears only its group', () => {
    const f = toggleFilter(toggleFilter(emptyFilters(), 'sprint', 'Phase 1'), 'type', 'test');
    const cleared = toggleFilter(f, 'sprint', 'all');
    expect(cleared.sprint.size).toBe(0);
    expect([...cleared.type]).toEqual(['test']);
  });

  it('"all" is active exactly when the group is empty', () => {
    const f = toggleFilter(emptyFilters(), 'status', 'done');
    expect(isActive(f, 'status', 'all')).toBe(false);
    expect(isActive(f, 'status', 'done')).toBe(true);
    expect(isActive(f, 'sprint', 'all')).toBe(true);
  });
});

describe('facetCounts', () => {
  // standard.json: 12 tasks, 4 per phase (read as sprints). Phase 0: infra×2, docs, test (all done). Phase 1: feature
  // (done), feature (in_progress), bugfix (todo), refactor (blocked). Phase 2: polish, test, feature (todo), docs
  // (skipped).
  const counts = (f: Partial<Record<keyof Filters, string[]>>) => facetCounts(standard, filters(f), undefined, model);
  const asObject = (m: ReadonlyMap<string, number>) => Object.fromEntries([...m].sort());

  it('without selections: every value over all tasks', () => {
    const c = counts({});
    expect(c.sprint.all).toBe(12);
    expect(asObject(c.sprint.values)).toEqual({ 'Phase 0': 4, 'Phase 1': 4, 'Phase 2': 4 });
    expect(asObject(c.type.values)).toEqual({
      bugfix: 1,
      docs: 2,
      feature: 3,
      infra: 2,
      polish: 1,
      refactor: 1,
      test: 2,
    });
    expect(asObject(c.status.values)).toEqual({ blocked: 1, done: 5, in_progress: 1, skipped: 1, todo: 4 });
  });

  it("a group's own selection doesn't change its counts; the other groups count within it", () => {
    const c = counts({ sprint: ['Phase 1'] });
    expect(c.sprint.all).toBe(12);
    expect(asObject(c.sprint.values)).toEqual({ 'Phase 0': 4, 'Phase 1': 4, 'Phase 2': 4 });
    expect(c.type.all).toBe(4);
    expect(asObject(c.type.values)).toEqual({ bugfix: 1, feature: 2, refactor: 1 });
    expect(asObject(c.status.values)).toEqual({ blocked: 1, done: 1, in_progress: 1, todo: 1 });
  });

  it('selections in two groups: each group counts under the other two', () => {
    const c = counts({ sprint: ['Phase 1'], type: ['feature'] });
    expect(asObject(c.sprint.values)).toEqual({ 'Phase 1': 2, 'Phase 2': 1 }); // features by sprint
    expect(asObject(c.type.values)).toEqual({ bugfix: 1, feature: 2, refactor: 1 }); // Phase 1 by type
    expect(c.status.all).toBe(2);
    expect(asObject(c.status.values)).toEqual({ done: 1, in_progress: 1 });
  });

  it('several values in one group combine with OR', () => {
    const c = counts({ sprint: ['Phase 1', 'Phase 2'] });
    expect(c.type.all).toBe(8);
    expect(c.type.values.get('feature')).toBe(3);
    expect(c.type.values.get('infra')).toBeUndefined();
  });

  it('a selection that matches nothing leaves the other groups at zero', () => {
    const c = counts({ status: ['nonexistent'] });
    expect(c.sprint.all).toBe(0);
    expect(c.sprint.values.size).toBe(0);
    expect(c.status.all).toBe(12);
  });

  it("agrees with filterTasks: a value's count is what selecting only it would show", () => {
    const c = counts({ type: ['feature', 'test'] });
    for (const [sprint, n] of c.sprint.values) {
      const shown = filterTasks(
        standard,
        { ...emptyFilters(), type: new Set(['feature', 'test']), sprint: new Set([sprint]) },
        undefined,
        model,
      );
      expect(shown.length).toBe(n);
    }
  });
});

describe('searchTasks', () => {
  const task = (id: string, name: string, description?: string, type = 'feature'): Task => ({
    id,
    name,
    phase: 0,
    type,
    description: description as string,
    status: 'todo',
  });
  const tasks = [
    task('T-001', 'Login form', 'Email and password'),
    task('T-002', 'Logout button', undefined, 'bugfix'),
    task('T-010', 'Docs', 'How to log in', 'docs'),
  ];
  const ids = (query: string) => searchTasks(tasks, query).map((t) => t.id);

  it('an empty or blank query matches everything', () => {
    expect(searchTasks(tasks, '')).toBe(tasks);
    expect(ids('   ')).toEqual(['T-001', 'T-002', 'T-010']);
  });

  it('matches ID, name, description and type, ignoring case', () => {
    expect(ids('t-01')).toEqual(['T-010']);
    expect(ids('LOG')).toEqual(['T-001', 'T-002', 'T-010']);
    expect(ids('password')).toEqual(['T-001']);
    expect(ids('bugfix')).toEqual(['T-002']);
  });

  it('every word must match', () => {
    expect(ids('log button')).toEqual(['T-002']);
    expect(ids('log nothing')).toEqual([]);
  });
});
