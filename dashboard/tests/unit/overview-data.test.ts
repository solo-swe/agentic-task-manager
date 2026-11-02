import { describe, expect, it, vi } from 'vitest';
import type { Claim } from '../../src/lib/claims';
import type { ProjectEntry } from '../../src/lib/load';
import {
  combinedTimeline,
  loadProjectSources,
  mergedTasks,
  projectOfWorker,
  projectProgress,
  projectTasks,
  velocityByPeriod,
  type ProjectSource,
} from '../../src/lib/overview-data';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY, resolveVocabulary } from '../../src/lib/vocab';

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
const source = (p: ProjectEntry, tasks: Task[], claims = new Map<string, Claim>()): ProjectSource => ({
  project: p,
  tasks,
  claims,
  vocab: DEFAULT_VOCABULARY,
});

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
  it('every project’s tasks as one project’s: keyed by project, statuses by kind, claims re-keyed', () => {
    // A project naming its own statuses: "shipped" is done
    const vocab = resolveVocabulary({
      statuses: [
        { id: 'open', label: 'Open', kind: 'todo' },
        { id: 'doing', label: 'Doing', kind: 'active' },
        { id: 'shipped', label: 'Shipped', kind: 'done' },
      ],
    } as never);
    const claim: Claim = {
      task: 'T-2',
      agent: 'claude:x',
      branch: null,
      claimed_at: '2026-01-02T11:00:00Z',
      lease_expires_at: '2026-01-02T13:00:00Z',
    };
    const merged = mergedTasks([
      source(project('a'), [task('T-1', 'Ada', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z')]),
      {
        ...source(
          project('b'),
          [
            { ...task('T-1', 'Bo', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z'), status: 'shipped' },
            { ...task('T-2', 'claude:x', '2026-01-02T11:00:00Z', null), status: 'doing' },
          ],
          new Map([['T-2', claim]]),
        ),
        vocab,
      },
    ]);
    expect(merged.tasks.map((t) => [t.id, t.status])).toEqual([
      ['A: T-1', 'done'],
      ['B: T-1', 'done'],
      ['B: T-2', 'in_progress'],
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

describe('projectProgress and weeklyVelocity (T-329)', () => {
  it('a project’s done of total (skipped left out), its active sprint and its open releases', () => {
    const meta = {
      sprints: [
        { id: 'One', state: 'closed' },
        { id: 'Two', state: 'active', goal: 'Ship it', due: '2026-01-05', release: '1.0' },
      ],
      releases: [
        { id: '0.9', state: 'released' },
        { id: '1.0', state: 'planned', name: 'First' },
      ],
    } as never;
    const tasks = [
      { ...task('T-1', 'Ada', '2026-01-01T08:00:00Z', '2026-01-01T09:00:00Z'), sprint: 'One' },
      { ...task('T-2', 'Ada', '2026-01-02T08:00:00Z', '2026-01-02T09:00:00Z'), sprint: 'Two' },
      { ...task('T-3', 'Ada', '2026-01-02T10:00:00Z', null), sprint: 'Two' },
      { id: 'T-4', name: 'Dropped', type: 'feature', status: 'skipped', sprint: 'Two' } as Task,
    ];
    const row = projectProgress({ ...source(project('a'), tasks), meta }, NOW);
    expect([row.done, row.total]).toEqual([2, 3]);
    expect(row.sprint && [row.sprint.sprint.id, row.sprint.done, row.sprint.total, row.sprint.dueIn]).toEqual([
      'Two',
      1,
      2,
      3,
    ]);
    expect(row.releases).toEqual([{ id: '1.0', name: 'First', done: 1, total: 2 }]);
    // Without sprints or releases
    const plain = projectProgress(source(project('b'), tasks.slice(0, 1)), NOW);
    expect([plain.sprint, plain.releases]).toEqual([undefined, []]);
  });

  it('velocityByPeriod: done per day, week or month, split by project, the last bar ending now (T-329 review)', () => {
    const day = (d: number) => new Date(NOW - d * 86_400_000 + 60_000).toISOString();
    const sources = [
      source(project('a'), [task('T-1', 'Ada', day(1), day(1)), task('T-2', 'Ada', day(9), day(9))]),
      source(project('b'), [
        task('T-1', 'Bo', day(1), day(1)),
        task('T-2', 'Bo', day(1), day(1)),
        task('T-3', 'Bo', day(40), day(40)),
        task('T-4', 'Bo', day(1), null),
      ]),
    ];
    const week = velocityByPeriod(sources, NOW, 'week');
    expect(week.bars).toHaveLength(10);
    expect(week.bars.slice(-2).map((b) => b.byProject)).toEqual([
      [1, 0],
      [1, 2],
    ]);
    expect(week.bars[9]).toMatchObject({ start: NOW - 7 * 86_400_000, total: 3 });
    // 5 done in the 10 weeks (the one 40 days ago too)
    expect(week.average).toBe(0.5);
    const days = velocityByPeriod(sources, NOW, 'day');
    expect([days.bars.length, days.bars[13].total]).toEqual([14, 3]);
    const months = velocityByPeriod(sources, NOW, 'month');
    expect(months.bars.map((b) => b.total)).toEqual([0, 0, 0, 0, 1, 4]);
  });
});

describe('loadProjectSources', () => {
  const json = (body: unknown, etag = '"1"') =>
    new Response(JSON.stringify(body), { status: 200, headers: { ETag: etag } });

  it('loads each present project’s tasks and claims; a missing one is left out', async () => {
    const fetchFn = vi.fn(async (url: string | URL | Request) => {
      const u = String(url);
      if (u.startsWith('p/a/tasks.json'))
        return json({ meta: { project: 'A' }, tasks: [task('T-1', 'Ada', '2026-01-02T08:00:00Z', null)] });
      if (u.startsWith('p/a/claims.json')) return json({ claims: [] });
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
      if (u.startsWith('p/a/tasks.json')) {
        expect((init?.headers as Record<string, string>)['If-None-Match']).toBe('"1"');
        return new Response(null, { status: 304 });
      }
      if (u.startsWith('p/b/tasks.json')) return new Response('', { status: 500 });
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
});
