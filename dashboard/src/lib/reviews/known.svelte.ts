import type { ReviewStatus } from './types';

// The reviews the page has listed or shown, by project and reference, with their status (ATM-585): a task's link to
// its review shows the status when the page knows it, and never asks the server for it (a request per task would be
// one too many). The Reviews tab fills it as it lists and shows reviews.

const known = $state<Record<string, ReviewStatus>>({});

const key = (project: string, reference: string) => `${project}\u0000${reference}`;

/** What the page knows of `reference`'s status in `project`, if anything */
export function knownStatus(project: string, reference: string): ReviewStatus | null {
  return known[key(project, reference)] ?? null;
}

/** The reviews of `project` just listed or shown, with their statuses */
export function learnReviews(project: string, reviews: readonly { reference: string; status: ReviewStatus }[]): void {
  for (const r of reviews)
    if (known[key(project, r.reference)] !== r.status) known[key(project, r.reference)] = r.status;
}

// The reviews the Reviews tab listed as unreadable (ATM-617), by project and reference, with why: a task's link to one
// says it can't be read rather than show no status
const unreadable = $state<Record<string, string>>({});

/** Why `reference` in `project` can't be read, if the page listed it so */
export function knownUnreadable(project: string, reference: string): string | null {
  return unreadable[key(project, reference)] ?? null;
}

/** The reviews of `project` the list just said can't be read: every other one of the project's is readable again */
export function learnUnreadable(project: string, reviews: readonly { reference: string; problem: string }[]): void {
  const prefix = key(project, '');
  const now: Record<string, string> = Object.fromEntries(reviews.map((r) => [key(project, r.reference), r.problem]));
  for (const k of Object.keys(unreadable)) if (k.startsWith(prefix) && !(k in now)) delete unreadable[k];
  for (const [k, problem] of Object.entries(now)) if (unreadable[k] !== problem) unreadable[k] = problem;
}
