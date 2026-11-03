// A project's vocabularies (docs/v4/data-model.md §2-3): statuses with kinds, phases, types and priorities, from
// tasks.json meta, each falling back to the defaults when absent. The dashboard is lenient where atm-cli is strict: a
// list it can't use is replaced by the default rather than reported. The kinds and the defaults are common's, the same
// data atm-cli uses (atm-cli/atm/common/schema/vocabularies.json, generated into generated/vocabularies.ts, T-367).
import * as common from './generated/vocabularies';
import type { StatusKind } from './generated/vocabularies';
import type { TasksFile } from './types';

export type { StatusKind } from './generated/vocabularies';
export const KINDS = common.KINDS;

export interface StatusDef {
  id: string;
  label: string;
  kind: StatusKind;
}
export interface Term {
  id: string;
  label: string;
}

export interface Vocabulary {
  statuses: StatusDef[];
  types: Term[];
  /** Most urgent first */
  priorities: Term[];
  defaultPriority: string;
  /** Phase names, where meta gives them */
  phaseNames: ReadonlyMap<number, string>;
  /** meta.labels: the text and order of labels it lists (others show as they are, after them) */
  labels: Term[];
}

export const DEFAULT_STATUSES: StatusDef[] = common.DEFAULT_STATUSES;
export const DEFAULT_TYPES: Term[] = common.DEFAULT_TYPES;
export const DEFAULT_PRIORITIES: Term[] = common.DEFAULT_PRIORITIES;

export const DEFAULT_VOCABULARY: Vocabulary = {
  statuses: DEFAULT_STATUSES,
  types: DEFAULT_TYPES,
  priorities: DEFAULT_PRIORITIES,
  defaultPriority: common.DEFAULT_PRIORITY,
  phaseNames: new Map(),
  labels: [],
};

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** "in_progress" → "In progress": the label of an ID no vocabulary names. */
export function humanize(id: string): string {
  return capitalize(id.replace(/_/g, ' '));
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const nonEmpty = (v: unknown): v is string => typeof v === 'string' && v !== '';

function terms(value: unknown): Term[] | null {
  if (!Array.isArray(value) || value.length === 0) return null;
  const list: Term[] = [];
  for (const item of value) {
    if (!isRecord(item) || !nonEmpty(item.id)) return null;
    list.push({ id: item.id, label: nonEmpty(item.label) ? item.label : humanize(item.id) });
  }
  return new Set(list.map((t) => t.id)).size === list.length ? list : null;
}

function statuses(value: unknown): StatusDef[] | null {
  const list = terms(value);
  if (!list) return null;
  const defs = list.map((t, i) => ({ ...t, kind: (value as Record<string, unknown>[])[i].kind as StatusKind }));
  if (!defs.every((d) => KINDS.includes(d.kind))) return null;
  // The kinds every workflow needs (atm-cli refuses a vocabulary without them)
  if (!(['todo', 'active', 'done'] as const).every((k) => defs.some((d) => d.kind === k))) return null;
  return defs;
}

export function resolveVocabulary(meta: TasksFile['meta']): Vocabulary {
  const m: Record<string, unknown> = isRecord(meta) ? meta : {};
  const priorities = terms(m.priorities) ?? DEFAULT_PRIORITIES;
  const phaseNames = new Map<number, string>();
  if (Array.isArray(m.phases)) {
    for (const p of m.phases)
      if (isRecord(p) && typeof p.id === 'number' && nonEmpty(p.name)) phaseNames.set(p.id, p.name);
  }
  const defaultPriority =
    nonEmpty(m.default_priority) && priorities.some((p) => p.id === m.default_priority)
      ? m.default_priority
      : (
          priorities.find((p) => p.id === common.DEFAULT_PRIORITY) ??
          priorities[Math.floor((priorities.length - 1) / 2)]
        ).id;
  return {
    statuses: statuses(m.statuses) ?? DEFAULT_STATUSES,
    types: terms(m.types) ?? DEFAULT_TYPES,
    priorities,
    defaultPriority,
    phaseNames,
    labels: terms(m.labels) ?? [],
  };
}

/** A label's text: meta.labels' label for it, else the label itself (labels aren't humanized: they are tags) */
export function labelText(vocab: Vocabulary, label: string): string {
  return vocab.labels.find((l) => l.id === label)?.label ?? label;
}

/** A task's labels (only strings), in meta.labels' order, then the others alphabetically */
export function labelsOf(vocab: Vocabulary, task: { labels?: unknown }): string[] {
  const labels = Array.isArray(task.labels) ? task.labels.filter((l): l is string => typeof l === 'string') : [];
  return [...new Set(labels)].sort(labelOrder(vocab));
}

export function labelOrder(vocab: Vocabulary) {
  const ids = vocab.labels.map((l) => l.id);
  const rank = (v: string) => (ids.includes(v) ? ids.indexOf(v) : ids.length);
  return (a: string, b: string) => rank(a) - rank(b) || a.localeCompare(b);
}

/** The status's definition. A status the vocabulary doesn't list keeps its default kind if it is a default status. */
export function statusDef(vocab: Vocabulary, status: string): StatusDef | undefined {
  return vocab.statuses.find((s) => s.id === status) ?? DEFAULT_STATUSES.find((s) => s.id === status);
}

/** The status's kind; undefined for a status nobody defines (shown in the Kanban's Other column). */
export function kindOf(vocab: Vocabulary, status: string): StatusKind | undefined {
  return statusDef(vocab, status)?.kind;
}

export function statusLabel(vocab: Vocabulary, status: string): string {
  return statusDef(vocab, status)?.label ?? humanize(status);
}

export function typeLabel(vocab: Vocabulary, type: string): string {
  return vocab.types.find((t) => t.id === type)?.label ?? humanize(type);
}

/** "Phase 2" or "Phase 2 · Foundations" when the phase is named */
export function phaseLabel(vocab: Vocabulary, phase: number): string {
  const name = vocab.phaseNames.get(phase);
  return name ? `Phase ${phase} · ${name}` : `Phase ${phase}`;
}

/** The task's priority ID: its own when the vocabulary has it; a missing or unknown priority is the default (DM-2). */
export function priorityOf(vocab: Vocabulary, task: { priority?: string }): string {
  return task.priority && vocab.priorities.some((p) => p.id === task.priority) ? task.priority : vocab.defaultPriority;
}

/** 0 for the most urgent priority */
export function priorityRank(vocab: Vocabulary, id: string): number {
  const i = vocab.priorities.findIndex((p) => p.id === id);
  return i === -1 ? priorityRank(vocab, vocab.defaultPriority) : i;
}

export function priorityLabel(vocab: Vocabulary, id: string): string {
  return vocab.priorities.find((p) => p.id === id)?.label ?? humanize(id);
}

/** Position in the vocabulary's status order; statuses it doesn't list come last. */
export function statusRank(vocab: Vocabulary, status: string): number {
  const i = vocab.statuses.findIndex((s) => s.id === status);
  return i === -1 ? vocab.statuses.length : i;
}
