// The atm app's own updates (T-483): the app checks its release's update manifest and installs a signed update only on
// the user's click (app/src-tauri/src/updates.rs). The page asks through the app's two commands; the plugin's own API
// isn't the page's.
import { invoke } from '@tauri-apps/api/core';

/** An update the app can install: what `check_update` answers */
export interface AppUpdate {
  version: string;
  current_version: string;
  /** Its What's new, as plain text */
  notes: string | null;
}

/** How often the app looks for an update while it runs (on start, then at most this often, when its window is
 * focused) */
export const UPDATE_CHECK_MS = 6 * 60 * 60 * 1000;

/** A newer published version, or null (up to date, a build without updates, or the check failed: logged only) */
export async function checkAppUpdate(): Promise<AppUpdate | null> {
  try {
    return await invoke<AppUpdate | null>('check_update');
  } catch (e) {
    console.error('Checking for an update failed:', e);
    return null;
  }
}

/** Download, verify, install and restart: the app restarts on success, so this returns only on failure (its
 * message) or when there was nothing to install (null) */
export async function installAppUpdate(): Promise<string | null> {
  try {
    await invoke('install_update');
    return null;
  } catch (e) {
    const error = e as { message?: unknown };
    return typeof error?.message === 'string' ? error.message : String(e);
  }
}
