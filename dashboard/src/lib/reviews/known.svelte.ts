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
