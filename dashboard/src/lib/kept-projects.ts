// The workspaces' project lists as last loaded, kept for the app's next start (ATM-776): the page starts from them, so
// its first paint names each tab's project and shows the page its address names, instead of project IDs and a default
// view until atm has listed every project. The list is atm's answer as it was given, read back through the same rule
// as a fresh answer (load.ts projectsFrom); its tag goes with the first ask, so an unchanged list costs atm no
// summaries. A convenience: storage missing, blocked or full keeps nothing, and the page loads as without it.
import type { Validators } from './api/http';
import { projectsFrom, type ProjectEntry } from './load';
import { readStored, writeStored } from './prefs';

/** Where they are kept, by workspace */
export const KEPT_PROJECTS_KEY = 'atm-kept-projects';

export interface KeptProjects {
  projects: ProjectEntry[];
  validators: Validators;
}

const text = (v: unknown): string | undefined => (typeof v === 'string' && v ? v : undefined);

/** Workspace `workspace`'s list as last kept, or none (nothing kept, or nothing usable) */
export function keptProjects(workspace: string): KeptProjects | null {
  const kept = readStored(KEPT_PROJECTS_KEY)[workspace];
  if (!kept || typeof kept !== 'object') return null;
  const { listed, etag, lastModified } = kept as Record<string, unknown>;
  if (!Array.isArray(listed)) return null;
  const validators: Validators = {};
  if (text(etag)) validators.etag = text(etag);
  if (text(lastModified)) validators.lastModified = text(lastModified);
  return { projects: projectsFrom(listed), validators };
}

/** Keep workspace `workspace`'s list, as the answer gave it (`listed`), with its tag */
export function keepProjects(workspace: string, listed: unknown, validators: Validators) {
  const all = readStored(KEPT_PROJECTS_KEY);
  writeStored(KEPT_PROJECTS_KEY, { ...all, [workspace]: { listed, ...validators } });
}
