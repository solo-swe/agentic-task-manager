// Colors and icons, lifted from the legacy dashboard (v0.4.0). Every color is a CSS token from tokens.css (or a tint of
// one), so themes can change them.
import type { StatusKind } from './vocab';

export interface ColorStyle {
  color: string;
  bg: string;
}

export const TYPE_COLORS: Record<string, ColorStyle> = {
  feature: { color: 'var(--green)', bg: 'color-mix(in srgb, var(--green) 15%, transparent)' },
  bugfix: { color: 'var(--red)', bg: 'color-mix(in srgb, var(--red) 15%, transparent)' },
  bug: { color: 'var(--red)', bg: 'color-mix(in srgb, var(--red) 15%, transparent)' },
  test: { color: 'var(--purple)', bg: 'color-mix(in srgb, var(--purple) 15%, transparent)' },
  infra: { color: 'var(--orange)', bg: 'color-mix(in srgb, var(--orange) 15%, transparent)' },
  docs: { color: 'var(--cyan)', bg: 'color-mix(in srgb, var(--cyan) 15%, transparent)' },
  refactor: { color: 'var(--yellow)', bg: 'color-mix(in srgb, var(--yellow) 15%, transparent)' },
  polish: { color: 'var(--pink)', bg: 'color-mix(in srgb, var(--pink) 15%, transparent)' },
};

export const FALLBACK_TYPE: ColorStyle = {
  color: 'var(--text-muted)',
  bg: 'color-mix(in srgb, var(--text-muted) 15%, transparent)',
};

/** Statuses are styled by kind, so a project's own status looks like the built-in status of its kind. */
export const KIND_COLORS: Record<StatusKind, ColorStyle> = {
  todo: { color: 'var(--text-muted)', bg: 'color-mix(in srgb, var(--text-muted) 15%, transparent)' },
  active: { color: 'var(--accent)', bg: 'color-mix(in srgb, var(--accent) 15%, transparent)' },
  review: { color: 'var(--yellow)', bg: 'color-mix(in srgb, var(--yellow) 15%, transparent)' },
  done: { color: 'var(--green)', bg: 'color-mix(in srgb, var(--green) 15%, transparent)' },
  blocked: { color: 'var(--red)', bg: 'color-mix(in srgb, var(--red) 15%, transparent)' },
  skipped: { color: 'var(--text-muted)', bg: 'color-mix(in srgb, var(--text-muted) 10%, transparent)' },
};

export const PHASE_PALETTE = [
  'var(--green)',
  'var(--accent)',
  'var(--purple)',
  'var(--orange)',
  'var(--pink)',
  'var(--cyan)',
  'var(--yellow)',
  'var(--red)',
];

/** Darker node fills for the dependency graph (tokens --node-*). */
export const GRAPH_NODE_BG: Record<string, string> = {
  feature: 'var(--node-feature)',
  bugfix: 'var(--node-bugfix)',
  bug: 'var(--node-bugfix)',
  test: 'var(--node-test)',
  infra: 'var(--node-infra)',
  docs: 'var(--node-docs)',
  refactor: 'var(--node-refactor)',
  polish: 'var(--node-polish)',
};
export const FALLBACK_GRAPH_NODE_BG = 'var(--surface2)';

export const TYPE_EMOJI: Record<string, string> = {
  test: '\u{1F9EA}',
  bugfix: '\u{1F41B}',
  bug: '\u{1F41B}',
  feature: '\u{2728}',
  infra: '\u{2699}️',
  docs: '\u{1F4DD}',
  refactor: '\u{1F527}',
  polish: '\u{2728}',
};

export const KIND_EMOJI: Record<StatusKind, string> = {
  todo: '\u{26AA}',
  active: '\u{1F535}',
  review: '\u{1F440}',
  done: '\u{2705}',
  blocked: '\u{1F534}',
  skipped: '\u{23ED}',
};

export function phaseColor(phase: number): string {
  return PHASE_PALETTE[phase % PHASE_PALETTE.length];
}

export function typeStyle(type: string): ColorStyle {
  return TYPE_COLORS[type] ?? FALLBACK_TYPE;
}

/** Priorities by rank, most urgent first: the two most urgent stand out, the rest are quiet. */
export const PRIORITY_COLORS: ColorStyle[] = [
  { color: 'var(--red)', bg: 'color-mix(in srgb, var(--red) 15%, transparent)' },
  { color: 'var(--orange)', bg: 'color-mix(in srgb, var(--orange) 15%, transparent)' },
  { color: 'var(--text-muted)', bg: 'color-mix(in srgb, var(--text-muted) 12%, transparent)' },
];

export function priorityStyle(rank: number): ColorStyle {
  return PRIORITY_COLORS[Math.min(rank, PRIORITY_COLORS.length - 1)];
}

/** A status nobody defines (no kind) is styled like todo. */
export function kindStyle(kind: StatusKind | undefined): ColorStyle {
  return KIND_COLORS[kind ?? 'todo'];
}

export function graphNodeBg(type: string): string {
  return GRAPH_NODE_BG[type] ?? FALLBACK_GRAPH_NODE_BG;
}
