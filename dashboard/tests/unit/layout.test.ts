import { describe, expect, it } from 'vitest';
import { dependencyChain, layoutGraph, NODE_H, NODE_W, truncate, type GraphLayout } from '../../src/lib/graph/layout';
import type { Task } from '../../src/lib/types';
import standard from '../fixtures/standard.json';
import cycle from '../fixtures/cycle.json';

const task = (id: string, dependencies: string[] = []): Task => ({
  id,
  name: `Task ${id}`,
  phase: 0,
  type: 'feature',
  description: '',
  status: 'todo',
  dependencies,
});
const node = (l: GraphLayout, id: string) => l.nodes.find((n) => n.task.id === id)!;
const overlap = (a: GraphLayout['nodes'][number], b: GraphLayout['nodes'][number]) =>
  a.x < b.x + NODE_W && b.x < a.x + NODE_W && a.y < b.y + NODE_H && b.y < a.y + NODE_H;

// Laid out by dagre: the tests check what the layout promises, not dagre's exact coordinates
describe('layoutGraph', () => {
  it('puts every task to the right of what it waits for', () => {
    const l = layoutGraph(standard.tasks as Task[]);
    for (const e of l.edges) expect(node(l, e.to).x).toBeGreaterThan(node(l, e.from).x + NODE_W);
  });

  it('a chain on one line', () => {
    const l = layoutGraph([task('a'), task('b', ['a']), task('c', ['b'])]);
    expect(new Set(l.nodes.map((n) => n.cy)).size).toBe(1);
    expect(l.nodes.map((n) => n.x)).toEqual([...l.nodes.map((n) => n.x)].sort((x, y) => x - y));
  });

  it('no two tasks overlap, and all fit in the canvas', () => {
    const l = layoutGraph(standard.tasks as Task[]);
    l.nodes.forEach((a, i) => l.nodes.slice(i + 1).forEach((b) => expect(overlap(a, b)).toBe(false)));
    for (const n of l.nodes) {
      expect(n.x).toBeGreaterThanOrEqual(0);
      expect(n.y).toBeGreaterThanOrEqual(0);
      expect(n.x + NODE_W).toBeLessThanOrEqual(l.width);
      expect(n.y + NODE_H).toBeLessThanOrEqual(l.height);
      expect([n.cx, n.cy]).toEqual([n.x + NODE_W / 2, n.y + NODE_H / 2]);
    }
  });

  it('returns nodes in input order', () => {
    const tasks = [task('b', ['a']), task('a')];
    expect(layoutGraph(tasks).nodes.map((n) => n.task.id)).toEqual(['b', 'a']);
  });

  it('draws one edge per visible dependency, from the border of one task to the border of the other', () => {
    const l = layoutGraph([task('a'), task('b', ['a', 'a', 'hidden', 'b'])]);
    expect(l.edges.map((e) => [e.from, e.to])).toEqual([['a', 'b']]);
    const [, x1, y1] = /^M([\d.]+),([\d.]+)/.exec(l.edges[0].d)!.map(Number);
    const [, x2, y2] = /L([\d.]+),([\d.]+)$/.exec(l.edges[0].d)!.map(Number);
    expect([x1, y1]).toEqual([node(l, 'a').x + NODE_W, node(l, 'a').cy]);
    expect([x2, y2]).toEqual([node(l, 'b').x, node(l, 'b').cy]);
    expect(layoutGraph(standard.tasks as Task[]).edges).toHaveLength(12);
  });

  it('a dependency cycle still places and connects every task', () => {
    const l = layoutGraph(cycle.tasks as Task[]);
    expect(l.nodes).toHaveLength(5);
    expect(l.edges).toHaveLength(5);
    expect(l.nodes.every((n) => Number.isFinite(n.x) && Number.isFinite(n.y))).toBe(true);
  });

  it('truncates labels to 22 characters', () => {
    const long = { ...task('a'), name: 'A very long task name that keeps going' };
    expect(layoutGraph([long]).nodes[0].label).toBe('A very long task name…');
  });

  it('lays out again only when the shape changes, not for a new status or name', () => {
    const tasks = [task('a'), task('b', ['a']), task('c')];
    const first = layoutGraph(tasks);
    const renamed = layoutGraph([{ ...tasks[0], status: 'done', name: 'Renamed' }, tasks[1], tasks[2]]);
    expect(renamed.edges).toBe(first.edges); // the same placement, reused
    expect(renamed.nodes[0].task.status).toBe('done');
    expect(renamed.nodes[0].label).toBe('Renamed');
    expect(renamed.nodes.map((n) => [n.x, n.y])).toEqual(first.nodes.map((n) => [n.x, n.y]));

    const rewired = layoutGraph([tasks[0], tasks[1], task('c', ['b'])]);
    expect(rewired.edges).not.toBe(first.edges);
    expect(rewired.edges).toHaveLength(2);
  });

  it('keeps the last few shapes: going back to a project shown before lays out nothing again (T-525)', () => {
    const shape = (n: number) => [task(`p${n}`), task(`q${n}`, [`p${n}`])];
    const large = layoutGraph(shape(0));
    // Another project, then back: the same placement
    layoutGraph(shape(1));
    expect(layoutGraph(shape(0)).edges).toBe(large.edges);
    // Four others since: it was let go
    for (const n of [1, 2, 3, 4]) layoutGraph(shape(n));
    expect(layoutGraph(shape(0)).edges).not.toBe(large.edges);
  });

  it('is empty for no tasks', () => {
    expect(layoutGraph([])).toEqual({ width: 0, height: 0, nodes: [], edges: [] });
  });
});

describe('dependencyChain', () => {
  const l = layoutGraph(standard.tasks as Task[]);

  it('is the task and everything it waits for, directly or not', () => {
    // T-008 needs T-005 and T-006, which need T-004, which needs T-002 and T-003, which need T-001
    expect([...dependencyChain(l.edges, 'T-008')].sort()).toEqual([
      'T-001',
      'T-002',
      'T-003',
      'T-004',
      'T-005',
      'T-006',
      'T-008',
    ]);
  });

  it('leaves out what waits on the task', () => {
    expect([...dependencyChain(l.edges, 'T-001')]).toEqual(['T-001']);
  });

  it('ends in a cycle', () => {
    const c = layoutGraph(cycle.tasks as Task[]);
    expect(dependencyChain(c.edges, 'T-002').has('T-002')).toBe(true);
  });
});

describe('truncate', () => {
  it('only shortens strings over the limit', () => {
    expect(truncate('exactly', 7)).toBe('exactly');
    expect(truncate('too long', 7)).toBe('too lo…');
  });
});
