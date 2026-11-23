// Human-readable durations and times for estimates.

/** "8 m", "1 h 10 m", "2 h"; "< 1 m" for very short durations. */
export function formatMinutes(minutes: number): string {
  const total = Math.round(minutes);
  if (total < 1) return '< 1 m';
  const h = Math.floor(total / 60);
  const m = total % 60;
  if (h === 0) return `${m} m`;
  return m === 0 ? `${h} h` : `${h} h ${m} m`;
}

/** "50 m – 1 h 40 m", or a single value when both ends round to the same text. */
export function formatMinutesRange(low: number, high: number): string {
  const a = formatMinutes(low);
  const b = formatMinutes(high);
  return a === b ? a : `${a} – ${b}`;
}

/** How long ago `iso` was, to the minute: "just now", "12 m ago", "1 h 10 m ago" (a card's claim and times). */
export function formatAgo(iso: string, now: number): string {
  const minutes = (now - Date.parse(iso)) / 60_000;
  return minutes < 1 ? 'just now' : `${formatMinutes(minutes)} ago`;
}

/** How long ago `iso` was, in its largest unit: "just now", "5 min ago", "3 h ago", "12 d ago"; its date after 30
 * days, and the text as it is when it isn't a time (a list's "updated"). */
export function formatAgoOrDate(iso: string, now: number): string {
  const seconds = (now - Date.parse(iso)) / 1000;
  if (!Number.isFinite(seconds)) return iso;
  if (seconds < 60) return 'just now';
  const units: [string, number][] = [
    ['min', 60],
    ['h', 3600],
    ['d', 86400],
  ];
  const limits = [3600, 86400, 86400 * 30];
  const at = limits.findIndex((limit) => seconds < limit);
  if (at < 0) return iso.slice(0, 10);
  const [unit, size] = units[at];
  return `${Math.floor(seconds / size)} ${unit} ago`;
}

/** A moment in full, date and time, in the viewer's locale (a note's, a claim's, a merge's); "" without one. */
export function formatDateTime(iso: string | undefined | null): string {
  return iso ? new Date(iso).toLocaleString() : '';
}

const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

/** Clock time in the viewer's locale, with the date only when it isn't the same day as `now`. */
export function formatWhen(ts: number, now: number): string {
  const d = new Date(ts);
  const time = d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  return sameDay(d, new Date(now))
    ? time
    : `${d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}, ${time}`;
}
