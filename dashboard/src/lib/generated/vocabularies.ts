// The status kinds and the default vocabularies, generated from atm-cli/atm/common/schema/vocabularies.json by
// atm-cli/build.py: don't edit it, change that file and run `npm run build:cli` in dashboard/ (the pre-commit hook does it too).

/** What a status means; everything acts on kinds, never on status names. */
export type StatusKind = 'todo' | 'active' | 'review' | 'blocked' | 'done' | 'skipped';
export const KINDS: readonly StatusKind[] = ['todo', 'active', 'review', 'blocked', 'done', 'skipped'];

export const DEFAULT_STATUSES: { id: string; label: string; kind: StatusKind }[] = [
  { id: 'todo', label: 'To Do', kind: 'todo' },
  { id: 'in_progress', label: 'In Progress', kind: 'active' },
  { id: 'in_review', label: 'In Review', kind: 'review' },
  { id: 'blocked', label: 'Blocked', kind: 'blocked' },
  { id: 'done', label: 'Done', kind: 'done' },
  { id: 'skipped', label: 'Skipped', kind: 'skipped' },
];
export const DEFAULT_TYPES: { id: string; label: string }[] = [
  { id: 'feature', label: 'Feature' },
  { id: 'bugfix', label: 'Bugfix' },
  { id: 'test', label: 'Test' },
  { id: 'infra', label: 'Infra' },
  { id: 'docs', label: 'Docs' },
  { id: 'refactor', label: 'Refactor' },
  { id: 'polish', label: 'Polish' },
];
export const DEFAULT_PRIORITIES: { id: string; label: string }[] = [
  { id: 'P0', label: 'Critical' },
  { id: 'P1', label: 'High' },
  { id: 'P2', label: 'Medium' },
  { id: 'P3', label: 'Low' },
];
export const DEFAULT_PRIORITY = 'P2';
