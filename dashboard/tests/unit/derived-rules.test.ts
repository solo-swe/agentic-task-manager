// @vitest-environment node
// ATM-724 (docs/dashboard-derived-state.md §4.6): the dashboard decides nothing from a task's raw state fields. What a
// task's state means (its kind, whether it is ready, who it waits on, its stage, its work time) is atm's, served as
// `derived`; a status's kind from the vocabulary, the review state, the times a status set, and the sprints and
// statuses in `meta` are atm's to read. A file that uses one is on the list below, with what it does with it: it only
// shows it, or it is a rule still to move (its task named). A new file that reads one fails, so a rule can't come back
// unnoticed.
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const src = path.resolve(import.meta.dirname, '../../src');
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(path.join(dir, e.name)) : [path.join(dir, e.name)],
  );

/** What reads a task's raw state, or the project's raw statuses and sprints */
const RAW = [
  /\bkindOf\(/,
  /\.review_state\b/,
  /\.started_at\b/,
  /\.completed_at\b/,
  /\.review_requested_at\b/,
  /\.blocked_at\b/,
  /\bmeta\.sprints\b/,
  /\bmeta\.statuses\b/,
];

/** The files that may, and why: each only shows the field, or keeps a rule a later task moves to atm */
const ALLOWED: Record<string, string> = {
  'lib/vocab.ts': 'kindOf itself: a status’s kind in atm’s resolved vocabulary (ATM-718)',
  'lib/work.ts': 'shows the times a task holds in its details',
  'lib/cardfacts.ts': 'shows when a card’s status was set ("done 2 h ago")',
  'lib/details/DetailPanel.svelte': 'shows a task’s times and review state',
  'lib/kanban/KanbanCard.svelte': 'shows the review state’s label on the card',
  'lib/overview/ProjectCard.svelte': 'shows when the last task was done',
  'lib/overview/AllProjectsBoard.svelte': 'shows when a card was done',
  'lib/allboard.ts': 'sorts the Omniboard’s done cards by when, as the summary serves them',
  'lib/graph/DepGraph.svelte': 'colours a node by its status’s kind',
  'lib/progress/ProgressWidget.svelte': 'colours the status split by kind',
  'lib/kanban.ts': 'orders a column’s cards by when they entered it',
  'lib/projectinfo.ts': 'counts the vocabulary’s statuses for the Project tab',
  // Still the dashboard's, to move (ATM-722's review: the agents' lanes and the Activity timeline)
  'lib/timeline.ts': 'the Activity timeline (to move)',
  'lib/timeuse.ts': 'the Activity widget’s time use (to move)',
  'lib/progress/ActivityWidget.svelte': 'the Activity widget (to move)',
  'lib/burndown.ts': 'the burndown’s points from creation and completion times (to move)',
  'lib/insights.ts': 'the Agents tab’s insights (to move)',
  'lib/forecast.ts': 'keys atm’s forecast rows by the model’s sprints',
};

/** The raw fields a source reads, outside comments */
function rawUses(text: string): string[] {
  const code = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:])\/\/.*$/gm, '$1');
  return RAW.filter((pattern) => pattern.test(code)).map((pattern) => pattern.source);
}

describe('rules from atm, not the dashboard (ATM-724)', () => {
  const sources = files(src).filter((f) => /\.(ts|svelte)$/.test(f));

  it('only the listed files read a task’s raw state or the project’s raw statuses and sprints', () => {
    const offenders = sources
      .map((f) => [path.relative(src, f).split(path.sep).join('/'), rawUses(readFileSync(f, 'utf8'))] as const)
      .filter(([file, uses]) => uses.length > 0 && !(file in ALLOWED))
      .map(([file, uses]) => `${file}: ${uses.join(', ')}`);
    expect(offenders).toEqual([]);
  });

  it('every listed file still reads one (an entry for a file that no longer does is dropped)', () => {
    const stale = Object.keys(ALLOWED).filter((file) => {
      const full = path.join(src, file);
      try {
        return rawUses(readFileSync(full, 'utf8')).length === 0;
      } catch {
        return true;
      }
    });
    expect(stale).toEqual([]);
  });

  it('catches a rule that reads them, and not a comment that names them', () => {
    expect(rawUses('const late = task.started_at && Date.parse(task.started_at) < now;')).toEqual(['\\.started_at\\b']);
    expect(rawUses("if (kindOf(vocab, t.status) === 'done') count++;")).toEqual(['\\bkindOf\\(']);
    expect(rawUses('const s = data.meta.sprints.find((x) => x.state === "active");')).toEqual(['\\bmeta\\.sprints\\b']);
    expect(rawUses('// task.started_at is atm’s now\n/* meta.statuses too */ const x = 1;')).toEqual([]);
    expect(rawUses('const a = task.derived?.work?.live;')).toEqual([]);
  });
});
