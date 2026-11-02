import { describe, expect, it, vi } from 'vitest';
import type { Claim } from '../../src/lib/claims';
import type { ProjectEntry } from '../../src/lib/load';
import { combinedTimeline, loadAgentSources, type AgentSource } from '../../src/lib/overview-agents';
import type { Task } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';

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
const source = (p: ProjectEntry, tasks: Task[], claims = new Map<string, Claim>()): AgentSource => ({
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

describe('loadAgentSources', () => {
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
    const sources = await loadAgentSources([project('a'), project('gone', true)], [], fetchFn);
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
    const sources = await loadAgentSources([project('a'), project('b')], [before], fetchFn);
    expect(sources.map((s) => s.project.id)).toEqual(['a']);
    expect(sources[0].tasks).toBe(before.tasks);
    // b failed and had nothing before; with something before, it keeps it
    const kept = await loadAgentSources([project('b')], [{ ...before, project: project('b') }], fetchFn);
    expect(kept.map((s) => s.tasks[0].id)).toEqual(['T-1']);
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });
});
