import { describe, expect, it, vi } from 'vitest';
import type { Claim } from '../../src/lib/claims';
import type { ProjectEntry } from '../../src/lib/load';
import {
  combinedTimeline,
  loadProjectSources,
  mergedTasks,
  overviewBurndown,
  progressRows,
  projectOfWorker,
  projectTasks,
  velocityOf,
  withArchive,
  type ProjectSource,
} from '../../src/lib/overview-data';
import type { OverviewProgress, ProjectSummary } from '$generated/types';
import { summary } from './summary.helpers';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, type Vocabulary } from '../../src/lib/vocab';
import { asTaskViews, v1Tasks } from './api.helpers';
import { NO_SPRINTS } from '../../src/lib/sprints';
import { vocabulary, withKinds } from './views';

const NOW = Date.parse('2026-01-02T12:00:00Z');
const project = (id: string, missing = false): ProjectEntry => ({
  id,
  name: id.toUpperCase(),
  path: `/${id}`,
  missing,
  summary: null,
});
const task = (id: string, by: string, start: string, end: string | null, status = 'done'): Task =>
  ({
    id,
    name: `Task ${id}`,
    type: 'feature',
    status: end ? status : 'in_progress',
    started_by: by,
    started_at: start,
    completed_at: end ?? undefined,
  }) as Task;
/** A project's source as atm serves it: its tasks with their kinds in its vocabulary */
const source = (
  p: ProjectEntry,
  tasks: Task[],
  claims = new Map<string, Claim>(),
  vocab: Vocabulary = DEFAULT_VOCABULARY,
): ProjectSource => ({ project: p, tasks: withKinds(tasks, vocab), claims, vocab, sprints: NO_SPRINTS, releases: [] });

describe('combinedTimeline', () => {
  it('one lane per agent across projects; bars name their project and are keyed by it', () => {
    const timeline = combinedTimeline(
      [
        source(project('a'), [task('T-1', 'Ada', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z')]),
        source(project('b'), [
          task('T-1', 'Ada', '2026-01-02T10:00:00Z', '2026-01-02T10:30:00Z'),
          task('T-2', 'claude:x', '2026-01-02T11:00:00Z', null),
        ]),
      ],
      NOW,
    )!;
    expect(timeline.lanes.map((l) => l.agent)).toEqual(['claude:x', 'Ada']);
    const ada = timeline.lanes[1];
    expect(ada.bars.map((b) => b.key)).toEqual(['a/T-1', 'b/T-1']);
    expect(ada.bars.map((b) => b.project?.name)).toEqual(['A', 'B']);
    expect(ada.projects).toEqual(['A', 'B']);
    expect(ada.done).toBe(2);
    expect(ada.medianMinutes).toBe(45);
    expect(timeline.lanes[0].current.map((t) => t.id)).toEqual(['T-2']);
    expect(timeline.start).toBe(Date.parse('2026-01-02T08:00:00Z'));
    expect(timeline.end).toBe(NOW);
  });

  it('no started task anywhere: no timeline', () => {
    expect(combinedTimeline([source(project('a'), [])], NOW)).toBeNull();
    expect(combinedTimeline([], NOW)).toBeNull();
  });

  it('tasks without an agent stay last', () => {
    const t = { ...task('T-9', '', '2026-01-02T11:59:00Z', '2026-01-02T11:59:30Z'), started_by: undefined } as Task;
    const timeline = combinedTimeline(
      [source(project('a'), [t, task('T-1', 'Ada', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z')])],
      NOW,
    )!;
    expect(timeline.lanes.map((l) => l.agent)).toEqual(['Ada', '']);
  });
});

describe('mergedTasks (T-328)', () => {
  it('every project’s tasks as one project’s: keyed by project, each with its status and atm’s kind, claims re-keyed', () => {
    // A project naming its own statuses: "shipped" is done
    const vocab = vocabulary({
      statuses: [
        { id: 'open', label: 'Open', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
      ],
    });
    const claim: Claim = {
      task: 'T-2',
      agent: 'claude:x',
      branch: null,
      claimed_at: '2026-01-02T11:00:00Z',
      lease_expires_at: '2026-01-02T13:00:00Z',
    };
    const merged = mergedTasks([
      source(project('a'), [task('T-1', 'Ada', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z')]),
      source(
        project('b'),
        [
          { ...task('T-1', 'Bo', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z'), status: 'shipped' },
          { ...task('T-2', 'claude:x', '2026-01-02T11:00:00Z', null), status: 'doing' },
        ],
        new Map([['T-2', claim]]),
        vocab,
      ),
    ]);
    // The charts go by kind: the statuses stay each project's own
    expect(merged.tasks.map((t) => [t.id, t.status, t.derived?.kind])).toEqual([
      ['A: T-1', 'done', 'done'],
      ['B: T-1', 'shipped', 'done'],
      ['B: T-2', 'doing', 'active'],
    ]);
    expect([...merged.claims.keys()]).toEqual(['B: T-2']);
    expect(merged.claims.get('B: T-2')?.task).toBe('B: T-2');
    expect(merged.origin.get('B: T-1')).toEqual({ project: project('b'), id: 'T-1' });
  });

  it('projectTasks: by project, for the Activity charts (type = project, worker = project · agent) (T-328 review)', () => {
    const merged = mergedTasks([
      source(project('a'), [task('T-1', 'claude:x', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z')]),
      source(project('b'), [{ ...task('T-1', '', '2026-01-02T08:00:00Z', null), started_by: undefined }]),
    ]);
    const tasks = projectTasks(merged);
    expect(tasks.map((t) => [t.id, t.type, t.started_by])).toEqual([
      ['A: T-1', 'A', 'A · claude:x'],
      ['B: T-1', 'B', 'B · '],
    ]);
    expect(tasks.map((t) => projectOfWorker(t.started_by!))).toEqual(['A', 'B']);
    expect(projectOfWorker('no separator')).toBe('no separator');
  });
});

// The overview's Progress tab from the projects' summaries (ATM-722): atm's figures (atm-lib's stats::overview, tested
// there), added up and laid out here
describe('the overview’s Progress from summaries (T-329, ATM-722)', () => {
  const DAY = 86_400_000;
  const today = Math.floor(NOW / DAY);
  const withProgress = (id: string, progress: Partial<OverviewProgress>, fields: Partial<ProjectSummary> = {}) => ({
    ...project(id),
    summary: summary({
      ...fields,
      progress: { done: 0, total: 0, open_releases: [], done_days: [], burndown: [], forecast: null, ...progress },
    }),
  });

  it('a row per project with a summary: its done of total, its active sprint and open releases, as atm counts them', () => {
    const active = {
      id: 'Two',
      state: 'active' as const,
      due: null,
      release: null,
      done: 1,
      total: 2,
      percent: 50,
      goal: 'Ship it',
      days_left: 3,
    };
    const rows = progressRows([
      withProgress(
        'a',
        { done: 2, total: 3, open_releases: [{ id: '1.0', name: 'First', done: 1, total: 2 }] },
        { active_sprint: active },
      ),
      withProgress('b', { done: 1, total: 1 }),
      project('c'),
    ]);
    expect(rows.map((r) => [r.project.id, r.done, r.total])).toEqual([
      ['a', 2, 3],
      ['b', 1, 1],
    ]);
    expect(rows[0].sprint && [rows[0].sprint.id, rows[0].sprint.days_left]).toEqual(['Two', 3]);
    expect(rows[0].releases).toEqual([{ id: '1.0', name: 'First', done: 1, total: 2 }]);
    expect([rows[1].sprint, rows[1].releases]).toEqual([undefined, []]);
  });

  it('velocity: done per day, week or month, split by project, from atm’s days with completions', () => {
    const projects = [
      withProgress('a', {
        done_days: [
          [today - 1, 1],
          [today - 9, 1],
        ],
      }),
      withProgress('b', {
        done_days: [
          [today - 1, 2],
          [today - 40, 1],
        ],
      }),
    ];
    const week = velocityOf(projects, NOW, 'week');
    expect(week.bars).toHaveLength(10);
    expect(week.bars.slice(-2).map((b) => b.byProject)).toEqual([
      [1, 0],
      [1, 2],
    ]);
    expect(week.bars[9]).toMatchObject({ start: NOW - 7 * DAY, total: 3 });
    expect(week.average).toBe(0.5);
    const months = velocityOf(projects, NOW, 'month');
    expect(months.bars.map((b) => b.total)).toEqual([0, 0, 0, 0, 1, 4]);
  });

  it('one burndown: each day’s scope and remaining added up, and the work left in all of them, one after another', () => {
    const projects = [
      withProgress('a', {
        burndown: [
          [today - 1, 4, 3],
          [today, 4, 2],
        ],
        forecast: { minutes: { likely: 30, low: 20, high: 40 }, waiting: 1 },
      }),
      withProgress('b', {
        burndown: [[today, 2, 2]],
        forecast: { minutes: { likely: 10, low: 10, high: 10 }, waiting: 0 },
      }),
    ];
    const b = overviewBurndown(projects, NOW);
    expect(b.points.map((p) => [p.scope, p.remaining])).toEqual([
      [4, 3],
      [6, 4],
    ]);
    expect(b.points.at(-1)!.t).toBe(NOW);
    expect(b.forecast).toEqual({
      from: { t: NOW, remaining: 4 },
      to: { t: NOW + 40 * 60_000, remaining: 1 },
      range: { low: NOW + 30 * 60_000, high: NOW + 50 * 60_000 },
    });
    // A range: only what comes after its start
    expect(overviewBurndown(projects, NOW, NOW - 1).points).toHaveLength(1);
    // Nothing timed anywhere: nothing to draw
    expect(overviewBurndown([withProgress('c', {})], NOW)).toEqual({ points: [], untimed: true });
  });
});

describe('loadProjectSources', () => {
  const json = (body: unknown, etag = '"1"') =>
    new Response(JSON.stringify(body), { status: 200, headers: { ETag: etag } });

  it('loads each present project’s tasks and claims; a missing one is left out', async () => {
    const fetchFn = vi.fn(async (url: string | URL | Request) => {
      const u = String(url);
      if (u.startsWith(v1Tasks('a')))
        return json(asTaskViews({ meta: { project: 'A' }, tasks: [task('T-1', 'Ada', '2026-01-02T08:00:00Z', null)] }));
      if (u.startsWith('/p/a/claims.json')) return json({ claims: [] });
      return new Response('', { status: 404 });
    }) as unknown as typeof fetch;
    const sources = await loadProjectSources([project('a'), project('gone', true)], [], fetchFn);
    expect(sources.map((s) => s.project.id)).toEqual(['a']);
    expect(sources[0].tasks.map((t) => t.id)).toEqual(['T-1']);
    expect(sources[0].validators).toEqual({ etag: '"1"' });
  });

  it('an unchanged tasks.json (304) keeps the tasks; a failed one keeps what it had, or is left out', async () => {
    const before = {
      ...source(project('a'), [task('T-1', 'Ada', '2026-01-02T08:00:00Z', null)]),
      validators: { etag: '"1"' },
    };
    const fetchFn = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url);
      if (u.startsWith(v1Tasks('a'))) {
        expect((init?.headers as Record<string, string>)['If-None-Match']).toBe('"1"');
        return new Response(null, { status: 304 });
      }
      if (u.startsWith(v1Tasks('b'))) return new Response('', { status: 500 });
      return new Response('', { status: 404 });
    }) as unknown as typeof fetch;
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const sources = await loadProjectSources([project('a'), project('b')], [before], fetchFn);
    expect(sources.map((s) => s.project.id)).toEqual(['a']);
    expect(sources[0].tasks).toBe(before.tasks);
    // b failed and had nothing before; with something before, it keeps it
    const kept = await loadProjectSources([project('b')], [{ ...before, project: project('b') }], fetchFn);
    expect(kept.map((s) => s.tasks[0].id)).toEqual(['T-1']);
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });

  it('the archive joins each project’s history, the live copy winning; an unchanged one is kept (T-355)', async () => {
    const live = task('T-2', 'Ada', '2026-01-02T08:00:00Z', null);
    const archived = [
      task('T-1', 'Ada', '2026-01-01T08:00:00Z', '2026-01-01T09:00:00Z'),
      { ...task('T-2', 'Ada', '2026-01-01T08:00:00Z', '2026-01-01T09:00:00Z'), name: 'Old copy' },
    ];
    let archiveRequests = 0;
    const fetchFn = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url);
      if (u === `${v1Tasks('a')}?archived=true`) {
        archiveRequests++;
        if ((init?.headers as Record<string, string>)['If-None-Match'] === '"a1"')
          return new Response(null, { status: 304 });
        return json(asTaskViews({ tasks: archived }), '"a1"');
      }
      if (u.startsWith(v1Tasks('a'))) return json(asTaskViews({ meta: { project: 'A' }, tasks: [live] }), '"t1"');
      return new Response('', { status: 404 });
    }) as unknown as typeof fetch;
    const [first] = await loadProjectSources([project('a')], [], fetchFn);
    expect(first.tasks.map((t) => [t.id, t.name])).toEqual([
      ['T-2', 'Task T-2'],
      ['T-1', 'Task T-1'],
    ]);
    expect(first.archiveValidators).toEqual({ etag: '"a1"' });
    // tasks.json changed, the archive didn't: the archive comes from the last load
    const [second] = await loadProjectSources([project('a')], [first], fetchFn);
    expect(archiveRequests).toBe(2);
    expect(second.archived).toBe(first.archived);
    expect(second.tasks.map((t) => t.id)).toEqual(['T-2', 'T-1']);
  });

  it('withArchive: the live tasks, then the archived ones they lack; nothing archived, the same list', () => {
    const live = [task('T-2', 'Ada', '2026-01-02T08:00:00Z', null)];
    expect(withArchive(live, [])).toBe(live);
    expect(
      withArchive(live, [task('T-1', 'Ada', '2026-01-01T08:00:00Z', '2026-01-01T09:00:00Z')]).map((t) => t.id),
    ).toEqual(['T-2', 'T-1']);
  });
});
