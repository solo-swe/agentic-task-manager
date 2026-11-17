// The diff viewer's logic (docs/code-review-design.md §3.7, ATM-573): a file's hunks, as the round's patch has them,
// laid out for the split and the unified view, with the unchanged runs between them folded. The viewer is ours (atm
// draws the diff); each run of lines is rendered by CompMD, as the document viewer renders a source file.

/** A line of a hunk: kept, removed or added */
export type Op = ' ' | '-' | '+';

export interface HunkLine {
  op: Op;
  text: string;
}

/** A hunk: where it starts on each side and how many lines it spans there, and its lines */
export interface Hunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: HunkLine[];
}

const HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/** The hunks of one file's patch (git's unified diff: its `---`/`+++` lines and anything before the first hunk are
 * skipped, as is `\ No newline at end of file`) */
export function parseHunks(patch: string): Hunk[] {
  const hunks: Hunk[] = [];
  let hunk: Hunk | null = null;
  for (const line of patch.replace(/\r\n/g, '\n').split('\n')) {
    const header = HEADER.exec(line);
    if (header) {
      hunk = {
        oldStart: Number(header[1]),
        oldLines: header[2] === undefined ? 1 : Number(header[2]),
        newStart: Number(header[3]),
        newLines: header[4] === undefined ? 1 : Number(header[4]),
        lines: [],
      };
      hunks.push(hunk);
      continue;
    }
    if (!hunk || line.startsWith('\\')) continue;
    const op = line[0];
    if (op === ' ' || op === '-' || op === '+') hunk.lines.push({ op, text: line.slice(1) });
  }
  // A hunk ends after its counted lines: a trailing empty line of the patch isn't one of them
  for (const h of hunks) {
    let old = 0;
    let added = 0;
    const kept: HunkLine[] = [];
    for (const l of h.lines) {
      if (old >= h.oldLines && added >= h.newLines) break;
      if (l.op !== '+') old++;
      if (l.op !== '-') added++;
      kept.push(l);
    }
    h.lines = kept;
  }
  return hunks;
}

/** A row of the split view: a line on each side (or none), and what changed */
export interface SplitRow {
  kind: 'context' | 'changed' | 'removed' | 'added';
  old: number | null;
  new: number | null;
  oldText: string | null;
  newText: string | null;
}

/** A row of the unified view: one line, of one side or both */
export interface UnifiedRow {
  kind: 'context' | 'removed' | 'added';
  old: number | null;
  new: number | null;
  text: string;
}

/** An unchanged run between hunks, folded: its first line on each side and its length */
export interface Fold {
  oldFrom: number;
  newFrom: number;
  count: number;
}

export type Block<R> = { type: 'rows'; rows: R[] } | { type: 'fold'; fold: Fold };

/** A file's lines (1-based), from its text; none for a side that doesn't exist */
export function linesOf(text: string | null): string[] {
  if (text === null) return [];
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  if (lines.at(-1) === '') lines.pop();
  return lines;
}

/** The unchanged runs around the hunks: before the first, between two, after the last (to the end of `newLines`) */
function gaps(hunks: Hunk[], newCount: number): { before: Fold | null; after: Fold[] } {
  const fold = (oldFrom: number, newFrom: number, count: number) => (count > 0 ? { oldFrom, newFrom, count } : null);
  if (!hunks.length) return { before: fold(1, 1, newCount), after: [] };
  const first = hunks[0];
  const before = fold(1, 1, Math.max(0, first.newStart - 1));
  const after: Fold[] = [];
  hunks.forEach((h, i) => {
    const oldEnd = h.oldStart + h.oldLines;
    const newEnd = h.newStart + h.newLines;
    const next = hunks[i + 1];
    const count = next ? next.newStart - newEnd : newCount - newEnd + 1;
    after.push(fold(oldEnd, newEnd, Math.max(0, count)) ?? { oldFrom: oldEnd, newFrom: newEnd, count: 0 });
  });
  return { before, after };
}

/** The rows of an unchanged run, shown: the same line on both sides */
function contextRows(fold: Fold, newLines: string[]) {
  return Array.from({ length: fold.count }, (_, i) => ({
    old: fold.oldFrom + i,
    new: fold.newFrom + i,
    text: newLines[fold.newFrom + i - 1] ?? '',
  }));
}

/** A hunk as split rows: a run of removed lines and the run of added lines after it pair up, line by line */
function splitHunk(h: Hunk): SplitRow[] {
  const rows: SplitRow[] = [];
  let old = h.oldStart;
  let added = h.newStart;
  let i = 0;
  while (i < h.lines.length) {
    const line = h.lines[i];
    if (line.op === ' ') {
      rows.push({ kind: 'context', old: old++, new: added++, oldText: line.text, newText: line.text });
      i++;
      continue;
    }
    const removed: string[] = [];
    const adds: string[] = [];
    while (i < h.lines.length && h.lines[i].op === '-') removed.push(h.lines[i++].text);
    while (i < h.lines.length && h.lines[i].op === '+') adds.push(h.lines[i++].text);
    for (let k = 0; k < Math.max(removed.length, adds.length); k++) {
      const hasOld = k < removed.length;
      const hasNew = k < adds.length;
      rows.push({
        kind: hasOld && hasNew ? 'changed' : hasOld ? 'removed' : 'added',
        old: hasOld ? old++ : null,
        new: hasNew ? added++ : null,
        oldText: hasOld ? removed[k] : null,
        newText: hasNew ? adds[k] : null,
      });
    }
  }
  return rows;
}

function unifiedHunk(h: Hunk): UnifiedRow[] {
  let old = h.oldStart;
  let added = h.newStart;
  return h.lines.map((l) =>
    l.op === ' '
      ? { kind: 'context', old: old++, new: added++, text: l.text }
      : l.op === '-'
        ? { kind: 'removed', old: old++, new: null, text: l.text }
        : { kind: 'added', old: null, new: added++, text: l.text },
  );
}

/** A fold's key: where it starts on the new side (what `expanded` holds once *show more* opened it) */
export const foldKey = (fold: Fold) => fold.newFrom;

function layout<R>(
  hunks: Hunk[],
  newText: string | null,
  expanded: ReadonlySet<number>,
  rowsOf: (h: Hunk) => R[],
  context: (fold: Fold, lines: string[]) => R[],
): Block<R>[] {
  const lines = linesOf(newText);
  const { before, after } = gaps(hunks, lines.length);
  const blocks: Block<R>[] = [];
  const push = (rows: R[]) => {
    if (!rows.length) return;
    const last = blocks.at(-1);
    if (last?.type === 'rows') last.rows.push(...rows);
    else blocks.push({ type: 'rows', rows });
  };
  const gap = (fold: Fold | null) => {
    if (!fold || !fold.count) return;
    if (expanded.has(foldKey(fold))) push(context(fold, lines));
    else blocks.push({ type: 'fold', fold });
  };
  gap(before);
  hunks.forEach((h, i) => {
    push(rowsOf(h));
    gap(after[i]);
  });
  return blocks;
}

/** The split view: each hunk's rows, the unchanged runs folded unless `expanded` holds their key */
export function splitBlocks(hunks: Hunk[], newText: string | null, expanded: ReadonlySet<number> = new Set()) {
  return layout<SplitRow>(hunks, newText, expanded, splitHunk, (fold, lines) =>
    contextRows(fold, lines).map((r) => ({
      kind: 'context',
      old: r.old,
      new: r.new,
      oldText: r.text,
      newText: r.text,
    })),
  );
}

/** The unified view: each hunk's lines in order, the unchanged runs folded unless `expanded` holds their key */
export function unifiedBlocks(hunks: Hunk[], newText: string | null, expanded: ReadonlySet<number> = new Set()) {
  return layout<UnifiedRow>(hunks, newText, expanded, unifiedHunk, (fold, lines) =>
    contextRows(fold, lines).map((r) => ({ kind: 'context', old: r.old, new: r.new, text: r.text })),
  );
}

/** The lines each side changed: the old side's removed, the new side's added (for the rendered view's marks) */
export function changedLines(hunks: Hunk[]): { old: Set<number>; new: Set<number> } {
  const result = { old: new Set<number>(), new: new Set<number>() };
  for (const row of hunks.flatMap(unifiedHunk)) {
    if (row.kind === 'removed' && row.old !== null) result.old.add(row.old);
    if (row.kind === 'added' && row.new !== null) result.new.add(row.new);
  }
  return result;
}

/** Each change's first row (runs of changed rows in a row of blocks count as one): where *n* and *p* go. `[block,
 * row]` pairs, in order */
export function changeStarts<R extends { kind: string }>(blocks: Block<R>[]): [number, number][] {
  const starts: [number, number][] = [];
  blocks.forEach((block, b) => {
    if (block.type !== 'rows') return;
    block.rows.forEach((row, r) => {
      const prev = r > 0 ? block.rows[r - 1] : null;
      if (row.kind !== 'context' && (!prev || prev.kind === 'context')) starts.push([b, r]);
    });
  });
  return starts;
}

/** The change after (or before) `at`, wrapping round; none when there are no changes */
export function nextChange(starts: [number, number][], at: [number, number] | null, back = false) {
  if (!starts.length) return null;
  const before = (a: [number, number], b: [number, number]) => a[0] < b[0] || (a[0] === b[0] && a[1] < b[1]);
  if (!at) return back ? starts.at(-1)! : starts[0];
  if (back) return [...starts].reverse().find((s) => before(s, at)) ?? starts.at(-1)!;
  return starts.find((s) => before(at, s)) ?? starts[0];
}

/** `text` as a fenced code block of `language`, the fence longer than any run of backticks in it */
export function fenced(text: string, language: string | null): string {
  const longest = Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length));
  const marks = '`'.repeat(Math.max(3, longest + 1));
  return `${marks}${language ?? ''}\n${text}${text.endsWith('\n') || !text ? '' : '\n'}${marks}\n`;
}

/** The lines a selection spans, from the lines' boxes CompMD drew (the rendered view): those whose boxes it crosses */
export function linesUnder(
  drawn: readonly { line: number; rects: readonly { x: number; y: number; width: number; height: number }[] }[],
  selected: readonly { top: number; bottom: number }[],
): [number, number] | null {
  const hit = drawn
    .filter((l) => l.rects.some((r) => selected.some((s) => r.y < s.bottom && r.y + r.height > s.top)))
    .map((l) => l.line);
  return hit.length ? [Math.min(...hit), Math.max(...hit)] : null;
}
