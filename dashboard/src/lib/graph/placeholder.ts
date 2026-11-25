// The lines a graph node shows in place of its text when zoomed out too far to read it (ATM-735): a node still reads as
// a card with content, not an empty box. Drawn once in the graph's <defs> and placed on each node (<use>), so 500
// nodes cost no more than their boxes did (ATM-629 left the text out for the time it took).
import { NODE_H, NODE_W } from './layout';

/** A wiggly line from (`x`, `y`), `width` long: half-waves of `step`, `amplitude` up and down, ending at its length */
export function wiggle(x: number, y: number, width: number, step = 8, amplitude = 2.5): string {
  const waves = Math.max(1, Math.round(width / step));
  const half = width / waves;
  const r = (n: number) => Math.round(n * 100) / 100;
  // The first half-wave up, each one after it mirrored (`t`), so the line rolls on evenly
  return `M${r(x)},${r(y)} q${r(half / 2)},${r(-amplitude * 2)} ${r(half)},0` + ` t${r(half)},0`.repeat(waves - 1);
}

/** A node's two placeholder lines, from its top left corner: its key line (as wide as a key, its name's start) and
 * its name's (most of the node), where the two lines of text are when it is labelled */
export const PLACEHOLDER = {
  title: wiggle(10, 16, NODE_W * 0.45),
  name: wiggle(10, 34, NODE_W * 0.75),
  height: NODE_H,
} as const;
