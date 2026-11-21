// A project's vocabularies (docs/v4/data-model.md §2-3): statuses with kinds, types, priorities, labels, components
// and the workflow, as atm resolved them (the tasks response's `vocabulary`, ATM-718): the dashboard never parses meta
// for them. A vocabulary atm reads as broken is served as far as it works, with its problems (the footer shows them).
// A task's kind is atm's (`derived.kind`). The kinds and the defaults are common's, the data atm uses
// (crates/atm-common/schema/vocabularies.json, generated into $generated/vocabularies.ts, T-367).
import type { ProjectVocabulary } from '$generated/types';
import * as common from '$generated/vocabularies';
import type { StatusKind } from '$generated/vocabularies';
import type { Component } from './components';

export type { StatusKind } from '$generated/vocabularies';
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

export interface Workflow {
  type: string;
  base?: string;
  verify: boolean;
  commitTemplate?: string;
  /** Trailer lines every commit must carry */
  trailers: string[];
}

export interface Vocabulary {
  statuses: StatusDef[];
  types: Term[];
  /** Most urgent first */
  priorities: Term[];
  defaultPriority: string;
  /** Phase names, of a project from before sprints */
  phaseNames: ReadonlyMap<number, string>;
  /** meta.labels: the text and order of labels it lists (others show as they are, after them) */
  labels: Term[];
  /** The project's components, in order */
  components: Component[];
  /** The project's workflow, or null when it has none yet */
  workflow: Workflow | null;
}

export const DEFAULT_STATUSES: StatusDef[] = common.DEFAULT_STATUSES;
export const DEFAULT_TYPES: Term[] = common.DEFAULT_TYPES;
export const DEFAULT_PRIORITIES: Term[] = common.DEFAULT_PRIORITIES;

/** The vocabulary of a project that defines none: atm's defaults, until atm has said */
export const DEFAULT_VOCABULARY: Vocabulary = {
  statuses: DEFAULT_STATUSES,
  types: DEFAULT_TYPES,
  priorities: DEFAULT_PRIORITIES,
  defaultPriority: common.DEFAULT_PRIORITY,
  phaseNames: new Map(),
  labels: [],
  components: [],
  workflow: null,
};

/** The vocabulary atm serves (the tasks response's `vocabulary`), as the dashboard uses it; the defaults before atm
 * has served one */
export function fromServed(served: ProjectVocabulary | undefined): Vocabulary {
  if (!served) return DEFAULT_VOCABULARY;
  const w = served.workflow;
  return {
    statuses: served.statuses,
    types: served.types,
    priorities: served.priorities,
    defaultPriority: served.default_priority,
    phaseNames: new Map(served.phases.map((p) => [p.id, p.name])),
    labels: served.labels,
    components: served.components,
    workflow: w
      ? {
          type: w.type,
          base: w.base,
          verify: w.verify !== false,
          commitTemplate: w.commit_template,
          trailers: w.trailers?.require ?? [],
        }
      : null,
  };
}

/** A task's kind, as atm says (`derived.kind`); undefined for a status nobody defines (the Kanban's Other column) */
export function taskKind(task: { derived?: { kind?: string } }): StatusKind | undefined {
  const kind = task.derived?.kind;
  return kind && (KINDS as readonly string[]).includes(kind) ? (kind as StatusKind) : undefined;
}

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** "in_progress" → "In progress": the label of an ID no vocabulary names. */
export function humanize(id: string): string {
  return capitalize(id.replace(/_/g, ' '));
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

/** A status's kind in the vocabulary (a column's, a legend's; a task's is `taskKind`); undefined for a status nobody
 * defines */
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
