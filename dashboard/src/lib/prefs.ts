// Per-browser dashboard preferences in localStorage. Storage can be missing, blocked (private mode, file://,
// sandboxed previews) or hold junk, so every access falls back to defaults instead of throwing.

export const PREFS_KEY = 'atm-dashboard';

export interface Prefs {
  autoRefresh: boolean;
  autoRefreshSeconds: number;
  /** Last dashboard version whose release notes were opened */
  lastSeenVersion: string;
  /** The server build whose update notice the user dismissed ("Not now"); only another build shows it again */
  dismissedBuild: string;
  /** The app version whose update the user put off ("Not now"); only a newer one is offered again (T-483) */
  dismissedAppUpdate: string;
  kanbanGroupByPhase: boolean;
  /** 'system', 'light' or 'dark' (see appearance.ts) */
  mode: string;
  /** Theme id (see appearance.ts) */
  theme: string;
  /** How the document viewer's page looks: a document style id (see appearance.ts DOC_STYLES) */
  docStyle: string;
  /** The overview shows only active projects (lib/projects.ts) */
  overviewActiveOnly: boolean;
  /** The overview's tab: 'projects' or 'board' (lib/routes.ts) */
  overviewTab: string;
  /** The All projects board shows a lane per project (lib/allboard.ts) */
  boardGroupByProject: boolean;
  /** The All projects board's search (T-261) */
  boardQuery: string;
  /** The All projects board shows the active sprints' tasks ('active', the default) or every task ('all'; T-340) */
  boardScope: string;
  /** Where the document viewer is and how big (lib/documents.ts ViewerGeometry), or null for its default */
  documentViewer: { x: number; y: number; w: number; h: number } | null;
}

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/** True only if values can actually be written and read back (not just if localStorage exists). */
export function storageAvailable(): boolean {
  try {
    const s = storage();
    if (!s) return false;
    const probe = PREFS_KEY + ':probe';
    s.setItem(probe, '1');
    const ok = s.getItem(probe) === '1';
    s.removeItem(probe);
    return ok;
  } catch {
    return false;
  }
}

/** The JSON object stored under `key`, or {} when it is missing, unreadable or not an object. */
export function readStored(key: string): Record<string, unknown> {
  try {
    const raw = storage()?.getItem(key);
    const value: unknown = raw ? JSON.parse(raw) : {};
    return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

/** Stores `value` as JSON under `key`, or removes the key for null; silently does nothing without storage. */
export function writeStored(key: string, value: object | null): void {
  try {
    if (value === null) storage()?.removeItem(key);
    else storage()?.setItem(key, JSON.stringify(value));
  } catch {
    // Quota exceeded or storage blocked: preferences are a convenience, not state we must keep.
  }
}

function readAll(): Partial<Prefs> {
  return readStored(PREFS_KEY) as Partial<Prefs>;
}

/** The stored value, or `fallback` when it is missing, unreadable or of the wrong type. */
export function getPref<K extends keyof Prefs>(key: K, fallback: Prefs[K]): Prefs[K] {
  const value = readAll()[key];
  return typeof value === typeof fallback ? (value as Prefs[K]) : fallback;
}

/** Stores the value; silently does nothing when storage is unavailable. */
export function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]): void {
  writeStored(PREFS_KEY, { ...readAll(), [key]: value });
}
