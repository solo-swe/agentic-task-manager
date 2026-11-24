import { describe, expect, it } from 'vitest';
import { NODE_H, NODE_W } from '../../src/lib/graph/layout';
import { PLACEHOLDER, wiggle } from '../../src/lib/graph/placeholder';

// A graph node's placeholder lines, zoomed out too far for its text (lib/graph/placeholder.ts, ATM-735)

/** Where a path's half-waves end: its start, then each `q`/`t` step's end added up */
function ends(d: string): number[] {
  const [, x0] = /^M([\d.]+),/.exec(d)!;
  let x = Number(x0);
  const xs = [x];
  for (const [, , dx] of d.matchAll(/([qt])(?:[-\d.]+,[-\d.]+ )?([-\d.]+),0/g)) {
    x += Number(dx);
    xs.push(Math.round(x * 100) / 100);
  }
  return xs;
}

describe('wiggle', () => {
  it('rolls in even half-waves from its start to its length', () => {
    const d = wiggle(10, 16, 64, 8, 2.5);
    expect(d.startsWith('M10,16 q4,-5 8,0')).toBe(true);
    const xs = ends(d);
    expect(xs).toHaveLength(9);
    expect(xs.at(-1)).toBe(74);
    // Every half-wave the same length
    expect(new Set(xs.slice(1).map((x, i) => Math.round((x - xs[i]) * 100) / 100))).toEqual(new Set([8]));
  });

  it('a length not a whole number of steps is shared out evenly; a short one is one half-wave', () => {
    expect(ends(wiggle(0, 0, 30, 8)).at(-1)).toBe(30);
    expect(wiggle(0, 0, 3, 8)).toBe('M0,0 q1.5,-5 3,0');
  });
});

describe('the node placeholder', () => {
  it('two lines inside the node, where its two lines of text are: the key’s shorter', () => {
    const title = ends(PLACEHOLDER.title);
    const name = ends(PLACEHOLDER.name);
    for (const xs of [title, name]) {
      expect(xs[0]).toBeGreaterThan(0);
      expect(xs.at(-1)!).toBeLessThan(NODE_W);
    }
    expect(title.at(-1)! - title[0]).toBeLessThan(name.at(-1)! - name[0]);
    const y = (d: string) => Number(/^M[\d.]+,([\d.]+)/.exec(d)![1]);
    expect(y(PLACEHOLDER.title)).toBeLessThan(y(PLACEHOLDER.name));
    expect(y(PLACEHOLDER.name)).toBeLessThan(NODE_H);
  });
});
