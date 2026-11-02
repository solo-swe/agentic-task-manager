// Numbers behind the Progress and Activity widgets in the top bar: completion shares (overall, per phase, per type)
// and activity over time (completions, who was working, time per task).
import { isAgent } from './actors';
import { INTERACTIVE_LABEL } from './forecast';
import { completions, type Completion } from './timeline';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, kindOf, type StatusKind, type Vocabulary } from './vocab';
import { startedBy, workSpan } from './work';

export interface Share {
  key: string;
  label: string;
  done: number;
  /** Tasks that count: all but the skipped ones, which are neither done nor still to do */
  total: number;
  /** Skipped tasks, shown apart */
  skipped: number;
  /** done / total, 0–1; 1 when every task was skipped, 0 when there are no tasks */
  ratio: number;
}

export interface StatusCount {
  status: string;
  count: number;
}

export interface ProgressBreakdown {
  /** Every task of the project, the archived ones included */
  overall: Share;
  /** Archived tasks: counted as done in `overall`, and in the other numbers only when they were passed in */
  archived: number;
  byStatus: StatusCount[];
  byPhase: Share[];
  byType: Share[];
}

/** The status split's order: by kind, finished first; statuses without a kind last */
const KIND_ORDER: StatusKind[] = ['done', 'active', 'review', 'todo', 'blocked', 'skipped'];

/** Completion of a group of tasks, as atm counts it: done / (total − skipped). A group of only skipped tasks is
 * finished (nothing is left to do). */
export function share(key: string, label: string, tasks: Task[], vocab: Vocabulary = DEFAULT_VOCABULARY): Share {
  const done = tasks.filter((t) => kindOf(vocab, t.status) === 'done').length;
  const skipped = tasks.filter((t) => kindOf(vocab, t.status) === 'skipped').length;
  const total = tasks.length - skipped;
  return { key, label, done, total, skipped, ratio: total ? done / total : skipped ? 1 : 0 };
}

function groupBy<K>(tasks: Task[], key: (t: Task) => K): Map<K, Task[]> {
  const groups = new Map<K, Task[]>();
  for (const t of tasks) groups.set(key(t), [...(groups.get(key(t)) ?? []), t]);
  return groups;
}

/**
 * The Progress widget's numbers. `tasks` is tasks.json's tasks, or the history once the archive is loaded;
 * `unloadedArchived` is how many archived tasks are not among them (meta.archived until the archive is loaded): they
 * are all finished, so the overall share counts them as done without loading the archive (T-238), and archiving
 * never lowers the percentage. A skipped one among them counts as done until the archive is loaded and the numbers
 * become exact.
 */
export function progressBreakdown(
  tasks: Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  unloadedArchived = 0,
  archived = unloadedArchived,
): ProgressBreakdown {
  const statusCounts = groupBy(tasks, (t) => t.status);
  const rank = (s: string) => {
    const kind = kindOf(vocab, s);
    return kind ? KIND_ORDER.indexOf(kind) : KIND_ORDER.length;
  };
  const current = share('all', 'All tasks', tasks, vocab);
  const done = current.done + unloadedArchived;
  const total = current.total + unloadedArchived;
  return {
    overall: { ...current, done, total, ratio: total ? done / total : current.ratio },
    archived,
    byStatus: [...statusCounts]
      .map(([status, list]) => ({ status, count: list.length }))
      .sort((a, b) => rank(a.status) - rank(b.status) || a.status.localeCompare(b.status)),
    byPhase: [...groupBy(tasks, (t) => t.phase)]
      .sort(([a], [b]) => a - b)
      .map(([phase, list]) => share(String(phase), `Phase ${phase}`, list, vocab)),
    byType: [...groupBy(tasks, (t) => t.type)]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([type, list]) => share(type, type, list, vocab)),
  };
}

// ---- activity over time ----

export interface ActivityBucket {
  start: number;
  end: number;
  completed: Completion[];
  /** Completions per task type in this bucket */
  byType: Record<string, number>;
  /** Mean completions per bucket over this and the two before it */
  average: number;
  /** How much of the bucket each agent spent working on tasks, 0–1 per agent (agent "" = unattributed) */
  agentBusy: Record<string, number>;
  /** agentBusy summed by kind (T-247): Claude sessions and unattributed work are agents, anyone else people */
  byKind: { agents: number; people: number };
}

export interface CycleTime {
  id: string;
  name: string;
  type: string;
  /** When it was completed (ms) */
  time: number;
  minutes: number;
  /** An `interactive` task: its time includes working with you (docs/activity-popup-review.md §3) */
  withYou: boolean;
}

export interface ActivitySeries {
  /** Bucket length, e.g. "1 hour", "1 day" */
  unitLabel: string;
  bucketMs: number;
  buckets: ActivityBucket[];
  cycleTimes: CycleTime[];
  medianCycleMinutes: number | null;
  totalCompleted: number;
  /** Task types that have completions, most completed first */
  types: { type: string; count: number }[];
  /** Agents that worked in the period, by name (unattributed last): the order their colours follow */
  agents: string[];
  /** Stretches of at least MIN_IDLE_MS when no agent worked on any task, between the first start and now */
  idle: { start: number; end: number }[];
  idleMs: number;
}

/** Idle stretches from the first work (or `from`, if later) to `to`, and their total. */
function idleIn(work: { start: number; end: number }[], from: number, to: number) {
  if (work.length === 0) return { idle: [], idleMs: 0 };
  const begin = Math.max(from, Math.min(...work.map((w) => w.start)));
  const idle = begin < to ? gaps(work, begin, to) : [];
  return { idle, idleMs: idle.reduce((n, g) => n + g.end - g.start, 0) };
}

/** Shorter pauses (an agent picking up the next task) don't count as idle */
export const MIN_IDLE_MS = 10 * 60_000;

/** Gaps of at least `min` in the union of `intervals`, within [from, to]. */
export function gaps(intervals: { start: number; end: number }[], from: number, to: number, min = MIN_IDLE_MS) {
  const out: { start: number; end: number }[] = [];
  let cursor = from;
  for (const iv of [...intervals].sort((a, b) => a.start - b.start)) {
    if (iv.end <= cursor) continue;
    if (iv.start >= to) break;
    if (iv.start - cursor >= min) out.push({ start: cursor, end: iv.start });
    cursor = Math.max(cursor, iv.end);
    if (cursor >= to) break;
  }
  if (to - cursor >= min) out.push({ start: cursor, end: to });
  return out;
}

const MINUTE = 60_000;
const HOUR = 3_600_000;
// Labels read after "per" and "the whole": "per hour", "the whole 6 hours"
const BUCKET_SIZES: [number, string][] = [
  [5 * MINUTE, '5 minutes'],
  [15 * MINUTE, '15 minutes'],
  [30 * MINUTE, '30 minutes'],
  [HOUR, 'hour'],
  [3 * HOUR, '3 hours'],
  [6 * HOUR, '6 hours'],
  [12 * HOUR, '12 hours'],
  [24 * HOUR, 'day'],
  [2 * 24 * HOUR, '2 days'],
  [7 * 24 * HOUR, 'week'],
  [30 * 24 * HOUR, '30 days'],
];
export const MAX_ACTIVITY_BUCKETS = 36;

/**
 * The bucket each stepped period reads naturally in: an hour per 5 minutes, a day per hour, a week or a month per
 * day. All time and custom periods pick the smallest size that fits (activitySeries).
 */
export const PRESET_BUCKET_MS: Partial<Record<ActivityPreset, number>> = {
  hour: 5 * MINUTE,
  day: HOUR,
  week: 24 * HOUR,
  month: 24 * HOUR,
};

const ms = (iso: string | undefined) => {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? null : t;
};

export function median(values: number[]): number | null {
  if (values.length === 0) return null;
  const s = [...values].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

/**
 * The top of the time-per-task scale (T-248): the 90th percentile (nearest rank), so one very slow task doesn't push
 * all others to the floor; slower ones are drawn on the top edge. At least 1 minute.
 */
export function cycleScaleMax(minutes: number[]): number {
  if (minutes.length === 0) return 1;
  const s = [...minutes].sort((a, b) => a - b);
  return Math.max(1, s[Math.ceil(0.9 * s.length) - 1]);
}

// ---- the period the Activity panel looks at ----

export const ACTIVITY_PRESETS = ['hour', 'day', 'week', 'month', 'all', 'custom'] as const;
export type ActivityPreset = (typeof ACTIVITY_PRESETS)[number];
export const ACTIVITY_PRESET_LABELS: Record<ActivityPreset, string> = {
  hour: 'Hour',
  day: 'Day',
  week: 'Week',
  month: 'Month',
  all: 'All',
  custom: 'Custom',
};
/** Presets that step back and forth in time (‹ ›) */
export const STEPPED_PRESETS: readonly ActivityPreset[] = ['hour', 'day', 'week', 'month'];

export interface ActivityRange {
  preset: ActivityPreset;
  /** For stepped presets: 0 = the current hour/day/week/month, -1 the one before, … */
  offset?: number;
  /** For "custom": the period, in ms */
  from?: number;
  to?: number;
}

/** Local calendar start of the day, week (Monday) or month containing `t`, moved by `offset` units. */
function unitStart(preset: 'day' | 'week' | 'month', t: number, offset: number): number {
  const d = new Date(t);
  if (preset === 'day') return new Date(d.getFullYear(), d.getMonth(), d.getDate() + offset).getTime();
  if (preset === 'week')
    return new Date(d.getFullYear(), d.getMonth(), d.getDate() - ((d.getDay() + 6) % 7) + 7 * offset).getTime();
  return new Date(d.getFullYear(), d.getMonth() + offset, 1).getTime();
}

/**
 * The period a range stands for at `now`, or null for all time. The current hour is the last 60 minutes; the
 * current day, week (from Monday) and month run up to now; earlier ones are whole (local calendar).
 */
export function rangeWindow(range: ActivityRange, now: number): { start: number; end: number } | null {
  const offset = Math.min(0, range.offset ?? 0);
  switch (range.preset) {
    case 'hour':
      return { start: now - HOUR + offset * HOUR, end: now + offset * HOUR };
    case 'day':
    case 'week':
    case 'month': {
      const start = unitStart(range.preset, now, offset);
      return { start, end: offset === 0 ? now : unitStart(range.preset, now, offset + 1) };
    }
    case 'custom':
      return range.from !== undefined && range.to !== undefined && range.to > range.from
        ? { start: range.from, end: range.to }
        : null;
    default:
      return null;
  }
}

/**
 * The time axis the charts draw for a range (T-350): the whole period, even while it runs (all of today, the whole
 * week from Monday, the whole month), so a Monday's "This week" is seven days with one filled in, not one bar across
 * the chart. The last hour, an earlier period and a custom one are their window; all time is null.
 */
export function rangeAxis(range: ActivityRange, now: number): { start: number; end: number } | null {
  const w = rangeWindow(range, now);
  if (!w || (range.offset ?? 0) < 0) return w;
  if (range.preset === 'day' || range.preset === 'week' || range.preset === 'month')
    return { start: w.start, end: unitStart(range.preset, now, 1) };
  return w;
}

/** "Today", "Yesterday", "Wed, Oct 21", "Last week", "Week of Oct 5", "August 2026", … */
export function rangeLabel(range: ActivityRange, now: number): string {
  const offset = Math.min(0, range.offset ?? 0);
  const w = rangeWindow(range, now);
  const date = (t: number, o: Intl.DateTimeFormatOptions) => new Date(t).toLocaleDateString(undefined, o);
  const time = (t: number) => new Date(t).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  switch (range.preset) {
    case 'hour':
      return offset === 0 ? 'Last hour' : `${time(w!.start)} – ${time(w!.end)}`;
    case 'day':
      return offset === 0
        ? 'Today'
        : offset === -1
          ? 'Yesterday'
          : date(w!.start, { weekday: 'short', month: 'short', day: 'numeric' });
    case 'week':
      return offset === 0
        ? 'This week'
        : offset === -1
          ? 'Last week'
          : `Week of ${date(w!.start, { month: 'short', day: 'numeric' })}`;
    case 'month':
      return offset === 0
        ? 'This month'
        : offset === -1
          ? 'Last month'
          : date(w!.start, { month: 'long', year: 'numeric' });
    case 'custom':
      return 'Custom';
    default:
      return 'All time';
  }
}

/**
 * How a heading names the period: "today", "this week", "in the last hour", "in the week of Oct 5", "over all time"…
 * ("Tasks completed this week, per day").
 */
export function periodPhrase(range: ActivityRange, now: number): string {
  const label = rangeLabel(range, now);
  const offset = Math.min(0, range.offset ?? 0);
  switch (range.preset) {
    case 'hour':
      return offset === 0 ? 'in the last hour' : `from ${label}`;
    case 'day':
      return offset >= -1 ? label.toLowerCase() : `on ${label}`;
    case 'week':
      return offset >= -1 ? label.toLowerCase() : `in the ${label.charAt(0).toLowerCase()}${label.slice(1)}`;
    case 'month':
      return offset >= -1 ? label.toLowerCase() : `in ${label}`;
    case 'custom':
      return 'in the chosen period';
    default:
      return 'over all time';
  }
}

/** Whether a bucket's period isn't over yet at `now`: the sparkline draws it lighter, as it is still filling up */
export const ongoing = (bucket: ActivityBucket, now: number) => bucket.end > now;

/**
 * The Activity sparkline's tooltip (docs/activity-popup-review.md §1): what the bars and the line are, and what the
 * current bucket has so far: "Tasks completed per day, all time: 236 · the line is the average of the last 3 days ·
 * this day so far: 12 · click for details".
 */
export function sparklineTitle(series: ActivitySeries, now: number): string {
  const unit = series.unitLabel;
  // "day" → "days", "30 days" → "periods of 30 days"
  const counted = /^\d/.test(unit);
  const parts = [
    `Tasks completed per ${unit}, all time: ${series.totalCompleted}`,
    `the line is the average of the last 3 ${counted ? `periods of ${unit}` : `${unit}s`}`,
  ];
  const last = series.buckets.at(-1);
  if (last && ongoing(last, now))
    parts.push(`${counted ? `the current ${unit}` : `this ${unit}`} so far: ${last.completed.length}`);
  return [...parts, 'click for details'].join(' · ');
}

/**
 * Activity bucketed by `bucketMs` (one of the sizes above; a preset's own, PRESET_BUCKET_MS) or else by the smallest
 * of a few natural lengths that fits the time span; null without completions. With `period`, only that period is
 * shown (completions, cycle times and type counts inside it).
 */
export function activitySeries(
  tasks: Task[],
  period?: { start: number; end: number } | null,
  now = Date.now(),
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  bucket?: number,
): ActivitySeries | null {
  const kind = (t: Task) => kindOf(vocab, t.status);
  const all = completions(tasks, vocab);
  if (all.length === 0) return null;
  const done = period ? all.filter((c) => c.time >= period.start && c.time < period.end) : all;
  const starts = tasks.map((t) => ms(t.started_at)).filter((t): t is number => t !== null);
  const first = period ? period.start : Math.min(all[0].time, ...starts);
  const last = period ? period.end - 1 : Math.max(all[all.length - 1].time, ...starts);

  const [bucketMs, unitLabel] =
    BUCKET_SIZES.find(([size]) => size === bucket) ??
    // Buckets under an hour only for a chosen period (e.g. the last hour); all time starts at hours
    BUCKET_SIZES.find(([size]) => (period || size >= HOUR) && (last - first) / size < MAX_ACTIVITY_BUCKETS) ??
    BUCKET_SIZES[BUCKET_SIZES.length - 1];
  // Align buckets to local time (midnight for days)
  const offset = new Date(first).getTimezoneOffset() * 60_000;
  const origin = Math.floor((first - offset) / bucketMs) * bucketMs + offset;
  const count = Math.floor((last - origin) / bucketMs) + 1;

  const typeOf = new Map(tasks.map((t) => [t.id, t.type]));
  // Work intervals per agent (work.ts): start to completion or review request, or to now while active
  const work = tasks.flatMap((t) => {
    const span = workSpan(t, now, vocab);
    return !span || span.end <= span.start ? [] : [{ agent: startedBy(t) ?? '', start: span.start, end: span.end }];
  });
  const buckets: ActivityBucket[] = [];
  for (let i = 0; i < count; i++) {
    const start = origin + i * bucketMs;
    const end = start + bucketMs;
    const completed = done.filter((c) => c.time >= start && c.time < end);
    const recent = [...buckets.slice(-2).map((b) => b.completed.length), completed.length];
    const byType: Record<string, number> = {};
    for (const c of completed) byType[typeOf.get(c.id) ?? '?'] = (byType[typeOf.get(c.id) ?? '?'] ?? 0) + 1;
    const agentBusy: Record<string, number> = {};
    for (const w of work) {
      const overlap = Math.min(end, w.end) - Math.max(start, w.start);
      if (overlap > 0) agentBusy[w.agent] = (agentBusy[w.agent] ?? 0) + overlap / bucketMs;
    }
    // An agent on two tasks at once is still one agent at work; unattributed work counts as one agent too (T-247)
    const byKind = { agents: 0, people: 0 };
    for (const a of Object.keys(agentBusy)) {
      agentBusy[a] = Math.min(1, agentBusy[a]);
      byKind[a && !isAgent(a) ? 'people' : 'agents'] += agentBusy[a];
    }
    buckets.push({
      start,
      end,
      agentBusy,
      byKind,
      completed,
      byType,
      average: recent.reduce((a, b) => a + b, 0) / recent.length,
    });
  }

  // Time per task is its work: until it asked for a review, if it did; the review isn't part of it
  const cycleTimes: CycleTime[] = tasks
    .filter((t) => kind(t) === 'done')
    .map((t) => ({ t, s: ms(t.started_at), c: ms(t.completed_at), w: ms(t.review_requested_at) ?? ms(t.completed_at) }))
    .filter(
      ({ s, c, w }) =>
        s !== null && c !== null && w !== null && w >= s && (!period || (c >= period.start && c < period.end)),
    )
    .map(({ t, s, c, w }) => ({
      id: t.id,
      name: t.name,
      type: t.type,
      time: c!,
      minutes: (w! - s!) / 60_000,
      withYou: !!t.labels?.includes(INTERACTIVE_LABEL),
    }))
    .sort((a, b) => a.time - b.time);

  const typeTotals = new Map<string, number>();
  for (const c of done) typeTotals.set(typeOf.get(c.id) ?? '?', (typeTotals.get(typeOf.get(c.id) ?? '?') ?? 0) + 1);
  return {
    types: [...typeTotals]
      .map(([type, count]) => ({ type, count }))
      .sort((a, b) => b.count - a.count || a.type.localeCompare(b.type)),
    agents: [...new Set(buckets.flatMap((b) => Object.keys(b.agentBusy)))].sort(
      (a, b) => Number(a === '') - Number(b === '') || a.localeCompare(b),
    ),
    ...idleIn(work, first, period ? Math.min(period.end, now) : Math.max(last, now)),
    unitLabel,
    bucketMs,
    buckets,
    cycleTimes,
    medianCycleMinutes: median(cycleTimes.map((c) => c.minutes)),
    totalCompleted: done.length,
  };
}
