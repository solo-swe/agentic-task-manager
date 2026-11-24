// The overview of all projects (docs/multi-project-dashboard-proposal.md 3.5): which projects it shows, and in what
// order. The numbers come from the server's summaries (api/projects), so the browser never loads every task file.
import { formatMinutes } from './format';
import type { ProjectEntry, ProjectSummary } from './load';

/** A project without open work counts as active for this long after its last activity */
export const ACTIVE_DAYS = 14;
const DAY_MS = 24 * 3_600_000;

/** What needs the user: tasks in review, blocked on a question for them or to do together, and code reviews waiting
 * on them (ATM-606; a task in review through one of those is counted once, by the server) */
export function needsYou(p: ProjectEntry): number {
  const n = p.summary?.needs_you;
  return n ? n.review + n.blocked_on_user + (n.together ?? 0) + (n.code_reviews ?? 0) : 0;
}

/** Code reviews waiting on the user in a project (ATM-606): their decision or round, or answered since they wrote */
export function reviewsWaiting(p: ProjectEntry | undefined): number {
  return p?.summary?.needs_you.code_reviews ?? 0;
}

/** The items waiting in a project's inbox (ATM-761, ATM-762): the Inbox item's badge; none from an older server */
export function inboxOpen(p: ProjectEntry | undefined): number {
  return p?.summary?.inbox?.open ?? 0;
}

/** Tasks still to do: neither done nor skipped */
export function openTasks(p: ProjectEntry): number {
  return p.summary ? p.summary.total - p.summary.done - p.summary.skipped : 0;
}

function lastActivity(p: ProjectEntry): number {
  const t = Date.parse(p.summary?.last_activity ?? '');
  return Number.isNaN(t) ? -Infinity : t;
}

/**
 * Whether "Active only" shows the project: it has open work, or activity in the last `days` days. A project whose
 * folder is gone stays visible, so it can be removed.
 */
export function isActive(p: ProjectEntry, now: number, days = ACTIVE_DAYS): boolean {
  if (p.missing || !p.summary) return true;
  return openTasks(p) > 0 || lastActivity(p) >= now - days * DAY_MS;
}

/** A project's monogram, for a project without a logo: the initials of its first two words, else the first two
 * letters of its one word ("Agentic Task Manager" → "AT", "atm" → "AT"), upper case; "?" without letters. */
export function monogram(name: string): string {
  const [first = '', second] = name.match(/[\p{L}\p{N}]+/gu) ?? [];
  const letters = second ? first.slice(0, 1) + second.slice(0, 1) : first.slice(0, 2);
  return letters.toUpperCase() || '?';
}

/** A stable number for a project id, to pick its monogram's colour (the same project always gets the same one) */
export function idHash(id: string): number {
  let h = 0;
  for (const c of id) h = (h * 31 + (c.codePointAt(0) ?? 0)) >>> 0;
  return h;
}

/**
 * The card's stats line (docs/project-stats-proposal.md §1, T-255): "started 12 Aug · 236 tasks · 74 h agent time".
 * The year only when it isn't this one; agent time in whole hours from an hour on; no agent time, no part for it.
 */
export function statsLine(stats: NonNullable<ProjectSummary['stats']>, now: number): string {
  const parts: string[] = [];
  const started = stats.started ? new Date(stats.started) : null;
  if (started && !Number.isNaN(started.getTime())) {
    const year = started.getFullYear() === new Date(now).getFullYear() ? undefined : 'numeric';
    parts.push(`started ${started.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year })}`);
  }
  parts.push(`${stats.tasks} task${stats.tasks === 1 ? '' : 's'}`);
  if (stats.agent_minutes > 0) {
    const time =
      stats.agent_minutes >= 60 ? `${Math.round(stats.agent_minutes / 60)} h` : formatMinutes(stats.agent_minutes);
    parts.push(`${time} agent time`);
  }
  return parts.join(' · ');
}

/** Projects that need the user first, then the most recently active, then by name; gone ones last. */
export function orderProjects(projects: readonly ProjectEntry[]): ProjectEntry[] {
  return [...projects].sort(
    (a, b) =>
      Number(a.missing) - Number(b.missing) ||
      Number(needsYou(b) > 0) - Number(needsYou(a) > 0) ||
      lastActivity(b) - lastActivity(a) ||
      a.name.localeCompare(b.name),
  );
}
