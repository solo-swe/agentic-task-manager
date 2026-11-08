import changelog from 'virtual:atm-changelog';
import type { ChangelogEntry } from './changelog';

/** Release notes from WHATS-NEW.md, embedded at build time; newest first. */
export const releaseNotes: ChangelogEntry[] = changelog;

/** The version stamped into this dashboard at build time (see build/inject-version.ts). */
export function dashboardVersion(doc: Document = document): string | undefined {
  return doc.querySelector<HTMLMetaElement>('meta[name="atm-dashboard-version"]')?.content || undefined;
}

/** This page's build ID (<meta name="atm-dashboard-build">, stamped by the build); undefined in the dev server. */
export function dashboardBuild(doc: Document = document): string | undefined {
  return doc.querySelector<HTMLMetaElement>('meta[name="atm-dashboard-build"]')?.content || undefined;
}
