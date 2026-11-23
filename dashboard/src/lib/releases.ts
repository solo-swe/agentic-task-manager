// Releases (docs/sprints-and-releases-design.md SR-2.2, SR-7.5), as atm serves them with the tasks (ATM-720): each
// with its sprints, its tasks by stage and its notes, as `atm release notes` drafts them. The dashboard words them.
import type { ReleaseNotesTask, ServedRelease } from '$generated/types';

export type Release = ServedRelease;

const label = (t: ReleaseNotesTask) => t.key ?? t.id;

/** atm's notes for a release (§3.3) in Markdown, as `atm release notes` prints them: the done tasks by type, each with
 * its outcome; then what isn't done yet. Tasks by their keys, with task keys */
export function notesText(release: Release): string {
  const lines = [`# ${release.id}${release.name ? ` “${release.name}”` : ''}`];
  for (const group of release.notes.groups) {
    lines.push('', `## ${group.label}`, '');
    for (const t of group.tasks) lines.push(`- **${label(t)}** ${t.name}${t.outcome ? `: ${t.outcome}` : ''}`);
  }
  if (!release.notes.groups.length) lines.push('', 'Nothing done in it yet.');
  if (release.notes.open.length)
    lines.push('', '## Not done yet', '', ...release.notes.open.map((t) => `- ${label(t)} ${t.name} (${t.status})`));
  return lines.join('\n');
}

/**
 * The release view's two parts (T-323): the releases still open (planned), in the project's order, and the released
 * ones, newest first (by released_at; those without one after, the last listed first). Archived ones are in neither.
 */
export function splitReleases(releases: readonly Release[]): { open: Release[]; released: Release[] } {
  const at = (r: Release) => Date.parse(r.released_at ?? '') || -Infinity;
  const released = releases
    .map((r, i) => ({ r, i }))
    .filter(({ r }) => r.state === 'released')
    .sort((a, b) => at(b.r) - at(a.r) || b.i - a.i)
    .map(({ r }) => r);
  return { open: releases.filter((r) => r.state === 'planned'), released };
}
