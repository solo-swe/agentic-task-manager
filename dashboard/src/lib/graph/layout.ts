import dagre from '@dagrejs/dagre';
import type { Task } from '../types';

export const NODE_W = 160;
export const NODE_H = 52;
const PAD_X = 60;
const PAD_Y = 24;
const START_X = 30;
const START_Y = 20;
export const LABEL_MAX = 22;

export interface GraphNode {
  task: Task;
  x: number;
  y: number;
  cx: number;
  cy: number;
  /** Task name truncated to LABEL_MAX characters */
  label: string;
}

export interface GraphEdge {
  from: string;
  to: string;
  /** SVG path data: a smooth curve along dagre's route, from the border of `from` to the border of `to` */
  d: string;
}

export interface GraphLayout {
  width: number;
  height: number;
  /** In input order */
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export function truncate(str: string, max: number): string {
  return str.length > max ? str.substring(0, max - 1) + '…' : str;
}

/** `id` and every task it waits for, directly or through others, following the graph's edges. */
export function dependencyChain(edges: GraphEdge[], id: string): Set<string> {
  const deps = new Map<string, string[]>();
  for (const e of edges) deps.set(e.to, [...(deps.get(e.to) ?? []), e.from]);
  const seen = new Set([id]);
  const todo = [id];
  while (todo.length) {
    for (const dep of deps.get(todo.pop()!) ?? []) {
      if (seen.has(dep)) continue;
      seen.add(dep);
      todo.push(dep);
    }
  }
  return seen;
}

/** A smooth SVG path through dagre's edge points: straight ends, quadratic curves through the bends' midpoints. */
function smoothPath(points: { x: number; y: number }[]): string {
  const [first, ...rest] = points;
  let d = `M${first.x},${first.y}`;
  for (let i = 0; i < rest.length - 1; i++) {
    const p = rest[i];
    const next = rest[i + 1];
    d += ` Q${p.x},${p.y} ${(p.x + next.x) / 2},${(p.y + next.y) / 2}`;
  }
  const last = rest[rest.length - 1];
  return d + ` L${last.x},${last.y}`;
}

interface Placement {
  width: number;
  height: number;
  /** Each task's centre */
  centers: Map<string, { x: number; y: number }>;
  edges: GraphEdge[];
}

/** The graph's shape: which tasks, in which order, and the dependencies among them. Nothing else moves a task. */
function shapeKey(tasks: Task[], visible: Set<string>): string {
  return tasks.map((t) => t.id + ':' + (t.dependencies ?? []).filter((d) => visible.has(d)).join(',')).join('|');
}

function place(tasks: Task[], visible: Set<string>): Placement {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: 'LR', nodesep: PAD_Y, ranksep: PAD_X, marginx: START_X, marginy: START_Y });
  g.setDefaultEdgeLabel(() => ({}));
  for (const t of tasks) g.setNode(t.id, { width: NODE_W, height: NODE_H });
  for (const t of tasks)
    for (const dep of t.dependencies ?? []) if (visible.has(dep) && dep !== t.id) g.setEdge(dep, t.id);
  dagre.layout(g);
  const { width = 0, height = 0 } = g.graph();
  return {
    width,
    height,
    centers: new Map(tasks.map((t) => [t.id, { x: g.node(t.id).x, y: g.node(t.id).y }])),
    edges: g.edges().map(({ v, w }) => ({ from: v, to: w, d: smoothPath(g.edge(v, w).points ?? []) })),
  };
}

// The last few placements, the latest last: a reload that only changes statuses or names (every few seconds while
// agents work) reuses one, since dagre is the costly part of drawing the graph (~100 ms for 500 tasks); and so does
// going back to a project shown before, in the app's tabs or the project picker (T-525)
const KEPT = 4;
const placements = new Map<string, Placement>();

/** The placement of the shape `key`, kept or made now */
function placementOf(key: string, tasks: Task[], visible: Set<string>): Placement {
  const kept = placements.get(key);
  placements.delete(key);
  const placement = kept ?? place(tasks, visible);
  placements.set(key, placement);
  if (placements.size > KEPT) placements.delete(placements.keys().next().value!);
  return placement;
}

/**
 * Laid out by dagre, left to right: ranks by dependency (network simplex, so a task sits close to what it feeds),
 * fewer crossings, each task lined up with its neighbours. Only dependencies on tasks in `tasks` count.
 */
export function layoutGraph(tasks: Task[]): GraphLayout {
  if (tasks.length === 0) return { width: 0, height: 0, nodes: [], edges: [] };

  const visible = new Set(tasks.map((t) => t.id));
  const { width, height, centers, edges } = placementOf(shapeKey(tasks, visible), tasks, visible);
  const nodes = tasks.map((task) => {
    const { x: cx, y: cy } = centers.get(task.id)!;
    return { task, x: cx - NODE_W / 2, y: cy - NODE_H / 2, cx, cy, label: truncate(task.name, LABEL_MAX) };
  });
  return { width, height, nodes, edges };
}
